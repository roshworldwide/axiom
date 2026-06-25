//! Replicated Growable Array (RGA), refining [`tla/RGA.tla`].
//!
//! A sequence CRDT. Each element has a globally-unique [`ElementId`]
//! (`(Hlc, ReplicaId)`) and a `predecessor` (the element it was inserted after,
//! or `None` for the list head/origin). Deleting tombstones an id; the element
//! keeps its position so later elements stay correctly placed. Merge unions the
//! element and tombstone sets.
//!
//! The visible sequence is the pre-order traversal from the origin — children
//! of a node ordered DESCENDING by id (newest-after-the-reference first) — with
//! tombstoned elements filtered out. This is exactly the `Walk`/`SortDesc`
//! ordering of `tla/RGA.tla`, whose convergence is model-checked with TLC.
//!
//! [`tla/RGA.tla`]: ../../../../tla/RGA.tla

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::hlc::{Hlc, HlcClock};
use crate::ReplicaId;

/// A globally-unique, totally-ordered element identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ElementId {
    pub hlc: Hlc,
    pub replica: ReplicaId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Node<T> {
    content: T,
    /// The element this was inserted after; `None` means after the origin.
    predecessor: Option<ElementId>,
}

/// A replicated growable array of elements of type `T`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rga<T> {
    replica: ReplicaId,
    clock: HlcClock,
    elements: BTreeMap<ElementId, Node<T>>,
    tombstones: BTreeSet<ElementId>,
}

/// Abstract state mirroring `tla/RGA.tla` — the set of `(id, predecessor)`
/// elements and the tombstone set. Content is abstracted away (it does not
/// affect ordering or convergence), exactly as in the spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlaRgaState {
    pub elements: BTreeSet<(ElementId, Option<ElementId>)>,
    pub tombstones: BTreeSet<ElementId>,
}

impl<T> Rga<T> {
    /// A fresh, empty array owned by `replica`.
    pub fn new(replica: ReplicaId) -> Self {
        Self {
            replica,
            clock: HlcClock::new(),
            elements: BTreeMap::new(),
            tombstones: BTreeSet::new(),
        }
    }

    /// The full traversal order (including tombstoned elements, which hold their
    /// positions): the pre-order walk from the origin, siblings in DESCENDING id
    /// order (`Walk`/`SortDesc` in the spec).
    ///
    /// Iterative (an explicit heap stack) so a deep predecessor chain — the
    /// normal shape of a text buffer built by appending — cannot overflow the
    /// call stack. Any element whose predecessor is absent (which the spec's
    /// `Wellformed` invariant forbids, but a hostile or corrupt deserialized
    /// input could contain) is deterministically re-rooted at the origin, so it
    /// is never silently dropped and `len()` stays consistent with `ids()`.
    fn order(&self) -> Vec<ElementId> {
        let present: BTreeSet<ElementId> = self.elements.keys().copied().collect();
        let mut children: BTreeMap<Option<ElementId>, Vec<ElementId>> = BTreeMap::new();
        for (id, node) in &self.elements {
            let parent = match node.predecessor {
                Some(p) if !present.contains(&p) => None, // re-root orphan
                other => other,
            };
            children.entry(parent).or_default().push(*id);
        }
        // Sort siblings ASCENDING so that, popped from a LIFO stack, the largest
        // id is visited first (descending order) and its whole subtree precedes
        // the next sibling (pre-order).
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

    /// The ids of the visible (non-tombstoned) elements, in sequence order.
    pub fn ids(&self) -> Vec<ElementId> {
        self.order()
            .into_iter()
            .filter(|id| !self.tombstones.contains(id))
            .collect()
    }

    /// The visible sequence of element contents.
    pub fn to_vec(&self) -> Vec<&T> {
        self.ids()
            .into_iter()
            .map(|id| &self.elements[&id].content)
            .collect()
    }

    /// Number of visible (non-tombstoned) elements.
    pub fn len(&self) -> usize {
        self.elements
            .keys()
            .filter(|id| !self.tombstones.contains(id))
            .count()
    }

    /// `true` iff the array has no visible elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert `content` immediately after `after` (or at the front if `None`).
    /// Returns the new element's id.
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

    /// Insert `content` at visible position `index` (clamped to the end).
    /// Returns the new element's id.
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

    /// Tombstone the element `id` (if present). The element keeps its position.
    pub fn delete(&mut self, id: ElementId) {
        if self.elements.contains_key(&id) {
            self.tombstones.insert(id);
        }
    }

    /// The refinement mapping to `tla/RGA.tla`.
    ///
    /// Returns the abstract `(id, predecessor)` element set and tombstone set.
    /// If this mapping is faithful and the spec is verified, the implementation
    /// inherits the spec's properties — `Wellformed`, `LinearizationOK`, and the
    /// KEY `Convergent` property — all model-checked with TLC.
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
    /// Merge another replica's state in: union the element and tombstone sets,
    /// and advance this replica's clock past the merged ids.
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
        // Insert 'a', then 'b' after 'a', then 'c' after 'a'. RGA places the
        // newest insert (c) immediately after the reference (a), before b.
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
        // A buffer built by appending at the end is a linear predecessor chain
        // whose depth equals its length. The iterative traversal must handle
        // depths that would blow a recursive call stack (test threads default to
        // a small stack, where the old recursive walk aborted well below this).
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
        /// Order-independent convergence (the KEY property): merging two
        /// replicas in either order yields the same visible sequence.
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

        /// Idempotence: merging with itself changes nothing visible.
        #[test]
        fn merge_is_idempotent(a_ops in ops()) {
            let a = build(1, &a_ops);
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(owned(&aa), owned(&a));
        }

        /// Wellformed (mirrors the spec): every predecessor is present.
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

        /// LinearizationOK (mirrors the spec): the visible sequence lists each
        /// visible element exactly once.
        #[test]
        fn visible_sequence_is_a_permutation(a_ops in ops()) {
            let r = build(1, &a_ops);
            let ids = r.ids();
            let unique: BTreeSet<ElementId> = ids.iter().copied().collect();
            prop_assert_eq!(ids.len(), unique.len()); // no duplicates
            prop_assert_eq!(ids.len(), r.len());
        }

        /// MessagePack round-trip.
        #[test]
        fn msgpack_roundtrip(a_ops in ops()) {
            let r = build(1, &a_ops);
            let bytes = rmp_serde::to_vec(&r).unwrap();
            let back: Rga<char> = rmp_serde::from_slice(&bytes).unwrap();
            prop_assert_eq!(r, back); // full-struct fidelity (clock, tombstones, links)
        }
    }
}
