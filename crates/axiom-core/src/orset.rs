use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: Serialize + Ord",
    deserialize = "T: Deserialize<'de> + Ord"
))]
pub struct ORSet<T> {
    added: BTreeSet<(T, Uuid)>,
    removed: BTreeSet<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ORSetOp<T> {
    Add { element: T, tag: Uuid },
    Remove { tags: BTreeSet<Uuid> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlaORSetState<T> {
    pub added: BTreeSet<(T, Uuid)>,
    pub removed: BTreeSet<Uuid>,
}

impl<T: Ord> Default for ORSet<T> {
    fn default() -> Self {
        Self {
            added: BTreeSet::new(),
            removed: BTreeSet::new(),
        }
    }
}

impl<T: Ord> ORSet<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn contains(&self, x: &T) -> bool {
        self.added
            .iter()
            .any(|(e, tag)| e == x && !self.removed.contains(tag))
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        let mut present: BTreeSet<&T> = BTreeSet::new();
        for (e, tag) in &self.added {
            if !self.removed.contains(tag) {
                present.insert(e);
            }
        }
        present.into_iter()
    }

    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    pub fn remove(&mut self, x: &T) -> ORSetOp<T> {
        let tags: BTreeSet<Uuid> = self
            .added
            .iter()
            .filter(|(e, _)| e == x)
            .map(|(_, tag)| *tag)
            .collect();
        self.removed.extend(tags.iter().copied());
        ORSetOp::Remove { tags }
    }
}

impl<T: Ord + Clone> ORSet<T> {
    pub fn add(&mut self, x: T) -> ORSetOp<T> {
        let tag = Uuid::new_v4();
        self.added.insert((x.clone(), tag));
        ORSetOp::Add { element: x, tag }
    }

    pub fn apply(&mut self, op: &ORSetOp<T>) {
        match op {
            ORSetOp::Add { element, tag } => {
                self.added.insert((element.clone(), *tag));
            }
            ORSetOp::Remove { tags } => {
                self.removed.extend(tags.iter().copied());
            }
        }
    }

    pub fn merge(&mut self, other: &ORSet<T>) {
        self.added.extend(other.added.iter().cloned());
        self.removed.extend(other.removed.iter().copied());
    }

    pub fn tla_state(&self) -> TlaORSetState<T> {
        TlaORSetState {
            added: self.added.clone(),
            removed: self.removed.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn add_then_contains() {
        let mut s = ORSet::new();
        assert!(!s.contains(&7u8));
        s.add(7u8);
        assert!(s.contains(&7u8));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn concurrent_add_beats_remove() {
        let mut a = ORSet::new();
        a.add(7u8);
        let mut b = a.clone();
        a.add(7u8);
        b.remove(&7u8);
        let mut m = a.clone();
        m.merge(&b);
        assert!(m.contains(&7u8));
    }

    #[test]
    fn remove_wins_when_it_observed_the_add() {
        let mut a = ORSet::new();
        a.add(7u8);
        let mut b = a.clone();
        b.remove(&7u8);
        let mut m = a.clone();
        m.merge(&b);
        assert!(!m.contains(&7u8));
    }

    #[derive(Clone, Debug)]
    enum Op {
        Add(u8),
        Remove(u8),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![(0u8..4).prop_map(Op::Add), (0u8..4).prop_map(Op::Remove)]
    }

    fn ops() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(op(), 0..10)
    }

    fn build(ops: &[Op]) -> ORSet<u8> {
        let mut s = ORSet::new();
        for o in ops {
            match o {
                Op::Add(x) => {
                    s.add(*x);
                }
                Op::Remove(x) => {
                    s.remove(x);
                }
            }
        }
        s
    }

    fn added_tags(s: &ORSet<u8>) -> BTreeSet<Uuid> {
        s.tla_state().added.iter().map(|(_, t)| *t).collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::proptest_cases()))]

        #[test]
        fn merge_is_convergent(a_ops in ops(), b_ops in ops()) {
            let a = build(&a_ops);
            let b = build(&b_ops);
            let mut ma = a.clone();
            ma.merge(&b);
            let mut mb = b.clone();
            mb.merge(&a);
            prop_assert_eq!(ma.tla_state(), mb.tla_state());
            for x in 0u8..4 {
                prop_assert_eq!(ma.contains(&x), mb.contains(&x));
            }
        }

        #[test]
        fn merge_is_idempotent(a_ops in ops()) {
            let a = build(&a_ops);
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(aa.tla_state(), a.tla_state());
        }

        #[test]
        fn concurrent_add_always_wins(base in ops(), x in 0u8..4) {
            let mut a = build(&base);
            let mut b = a.clone();
            a.add(x);
            b.remove(&x);
            let mut m = a.clone();
            m.merge(&b);
            prop_assert!(m.contains(&x));
        }

        #[test]
        fn tombstones_are_observed(a_ops in ops(), b_ops in ops()) {
            let a = build(&a_ops);
            let b = build(&b_ops);
            let mut m = a.clone();
            m.merge(&b);
            let tags = added_tags(&m);
            prop_assert!(m.tla_state().removed.iter().all(|t| tags.contains(t)));
        }

        #[test]
        fn msgpack_roundtrip(a_ops in ops()) {
            let s = build(&a_ops);
            let bytes = rmp_serde::to_vec(&s).unwrap();
            let back: ORSet<u8> = rmp_serde::from_slice(&bytes).unwrap();
            prop_assert_eq!(s, back);
        }
    }
}
