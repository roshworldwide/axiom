use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::hlc::{Hlc, HlcClock};
use crate::ReplicaId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ElementId {
    pub hlc: Hlc,
    pub replica: ReplicaId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Node<T> {
    content: T,
    predecessor: Option<ElementId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rga<T> {
    replica: ReplicaId,
    clock: HlcClock,
    elements: BTreeMap<ElementId, Node<T>>,
    tombstones: BTreeSet<ElementId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlaRgaState {
    pub elements: BTreeSet<(ElementId, Option<ElementId>)>,
    pub tombstones: BTreeSet<ElementId>,
}

impl<T> Rga<T> {
    pub fn new(replica: ReplicaId) -> Self {
        Self {
            replica,
            clock: HlcClock::new(),
            elements: BTreeMap::new(),
            tombstones: BTreeSet::new(),
        }
    }

    fn order(&self) -> Vec<ElementId> {
        let present: BTreeSet<ElementId> = self.elements.keys().copied().collect();
        let mut children: BTreeMap<Option<ElementId>, Vec<ElementId>> = BTreeMap::new();
        for (id, node) in &self.elements {
            let parent = match node.predecessor {
                Some(p) if !present.contains(&p) => None,
                other => other,
            };
            children.entry(parent).or_default().push(*id);
        }
        for kids in children.values_mut() {
            kids.sort_unstable();
        }
        let mut out = Vec::with_capacity(self.elements.len());
        let mut stack: Vec<ElementId> = children.get(&None).cloned().unwrap_or_default();
        while let Some(id) = stack.pop() {
            out.push(id);
            if let Some(kids) = children.get(&Some(id)) {
                stack.extend(kids.iter().copied());
            }
        }
        debug_assert_eq!(
            out.len(),
            self.elements.len(),
            "order() must emit every element exactly once"
        );
        out
    }

    pub fn ids(&self) -> Vec<ElementId> {
        self.order()
            .into_iter()
            .filter(|id| !self.tombstones.contains(id))
            .collect()
    }

    pub fn to_vec(&self) -> Vec<&T> {
        self.ids()
            .into_iter()
            .map(|id| &self.elements[&id].content)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.elements
            .keys()
            .filter(|id| !self.tombstones.contains(id))
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn insert_after(&mut self, after: Option<ElementId>, content: T) -> ElementId {
        debug_assert!(
            after.is_none_or(|p| self.elements.contains_key(&p)),
            "insert_after: `after` must be None or an existing element"
        );
        let id = ElementId {
            hlc: self.clock.tick(),
            replica: self.replica,
        };
        self.elements.insert(
            id,
            Node {
                content,
                predecessor: after,
            },
        );
        id
    }

    pub fn insert_after_with_id(&mut self, id: ElementId, after: Option<ElementId>, content: T) {
        debug_assert!(
            after.is_none_or(|p| self.elements.contains_key(&p)),
            "insert_after_with_id: `after` must be None or an existing element"
        );
        self.elements.insert(
            id,
            Node {
                content,
                predecessor: after,
            },
        );
    }

    pub fn insert(&mut self, index: usize, content: T) -> ElementId {
        let visible = self.ids();
        let after = match index {
            0 => None,
            i => visible
                .get(i - 1)
                .copied()
                .or_else(|| visible.last().copied()),
        };
        self.insert_after(after, content)
    }

    pub fn delete(&mut self, id: ElementId) {
        if self.elements.contains_key(&id) {
            self.tombstones.insert(id);
        }
    }

    pub fn tla_state(&self) -> TlaRgaState {
        TlaRgaState {
            elements: self
                .elements
                .iter()
                .map(|(id, node)| (*id, node.predecessor))
                .collect(),
            tombstones: self.tombstones.clone(),
        }
    }
}

impl<T: Clone> Rga<T> {
    pub fn merge(&mut self, other: &Rga<T>) {
        for (id, node) in &other.elements {
            self.elements.entry(*id).or_insert_with(|| node.clone());
        }
        self.tombstones.extend(other.tombstones.iter().copied());
        if let Some(max_hlc) = other.elements.keys().map(|id| id.hlc).max() {
            self.clock.observe(max_hlc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rid(n: u64) -> ReplicaId {
        ReplicaId(n)
    }

    fn seq(r: &Rga<char>) -> String {
        r.to_vec().into_iter().collect()
    }

    #[test]
    fn insert_after_same_reference_orders_newest_first() {
        let mut r = Rga::new(rid(1));
        r.insert(0, 'a');
        r.insert(1, 'b');
        r.insert(1, 'c');
        assert_eq!(seq(&r), "acb");
    }

    #[test]
    fn delete_tombstones_but_keeps_following_order() {
        let mut r = Rga::new(rid(1));
        r.insert(0, 'a');
        let b = r.insert(1, 'b');
        r.insert(2, 'c');
        r.delete(b);
        assert_eq!(seq(&r), "ac");
    }

    #[test]
    fn long_linear_chain_does_not_overflow_the_stack() {
        let mut r = Rga::new(rid(1));
        let mut prev = None;
        for _ in 0..100_000u32 {
            prev = Some(r.insert_after(prev, 'x'));
        }
        assert_eq!(r.to_vec().len(), 100_000);
        assert_eq!(r.len(), 100_000);
    }

    #[derive(Clone, Debug)]
    enum Op {
        Insert(usize, char),
        Delete(usize),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (
                0usize..6,
                prop::sample::select(vec!['a', 'b', 'c', 'd', 'e'])
            )
                .prop_map(|(i, c)| Op::Insert(i, c)),
            (0usize..6).prop_map(Op::Delete),
        ]
    }

    fn ops() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(op(), 0..12)
    }

    fn build(replica: u64, ops: &[Op]) -> Rga<char> {
        let mut r = Rga::new(rid(replica));
        for o in ops {
            match o {
                Op::Insert(i, c) => {
                    let idx = (*i).min(r.len());
                    r.insert(idx, *c);
                }
                Op::Delete(i) => {
                    let visible = r.ids();
                    if !visible.is_empty() {
                        let id = visible[*i % visible.len()];
                        r.delete(id);
                    }
                }
            }
        }
        r
    }

    fn owned(r: &Rga<char>) -> Vec<char> {
        r.to_vec().into_iter().copied().collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::proptest_cases()))]

        #[test]
        fn merge_is_order_independent(a_ops in ops(), b_ops in ops()) {
            let a = build(1, &a_ops);
            let b = build(2, &b_ops);
            let mut ab = a.clone();
            ab.merge(&b);
            let mut ba = b.clone();
            ba.merge(&a);
            prop_assert_eq!(owned(&ab), owned(&ba));
        }

        #[test]
        fn merge_is_idempotent(a_ops in ops()) {
            let a = build(1, &a_ops);
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(owned(&aa), owned(&a));
        }

        #[test]
        fn predecessors_are_present(a_ops in ops(), b_ops in ops()) {
            let a = build(1, &a_ops);
            let b = build(2, &b_ops);
            let mut m = a.clone();
            m.merge(&b);
            let st = m.tla_state();
            let ids: BTreeSet<ElementId> = st.elements.iter().map(|(id, _)| *id).collect();
            for (_, pred) in &st.elements {
                if let Some(p) = pred {
                    prop_assert!(ids.contains(p));
                }
            }
        }

        #[test]
        fn visible_sequence_is_a_permutation(a_ops in ops()) {
            let r = build(1, &a_ops);
            let ids = r.ids();
            let unique: BTreeSet<ElementId> = ids.iter().copied().collect();
            prop_assert_eq!(ids.len(), unique.len());
            prop_assert_eq!(ids.len(), r.len());
        }

        #[test]
        fn msgpack_roundtrip(a_ops in ops()) {
            let r = build(1, &a_ops);
            let bytes = rmp_serde::to_vec(&r).unwrap();
            let back: Rga<char> = rmp_serde::from_slice(&bytes).unwrap();
            prop_assert_eq!(r, back);
        }
    }
}
