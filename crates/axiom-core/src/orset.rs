//! Observed-Remove Set (OR-Set), refining [`tla/ORSet.tla`].
//!
//! Each add of an element is stamped with a unique [`Uuid`] tag. State is the
//! set of observed-added `(element, tag)` pairs plus a set of tombstoned tags;
//! an element is present iff it has an observed-added tag that has not been
//! tombstoned. Merge is the union of both sets.
//!
//! This mirrors the **tombstone** design verified in `tla/ORSet.tla` (the
//! one-set "drop pairs + union merge" variant resurrects removed pairs on the
//! next merge and cannot satisfy add-wins). The spec uses `<<replica, counter>>`
//! tags; here tags are `Uuid` v4 — both are unique opaque tokens, and the spec's
//! reasoning is agnostic to the tag representation.
//!
//! [`tla/ORSet.tla`]: ../../../../tla/ORSet.tla

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An observed-remove set of elements of type `T`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: Serialize + Ord",
    deserialize = "T: Deserialize<'de> + Ord"
))]
pub struct ORSet<T> {
    /// Observed `(element, tag)` adds.
    added: BTreeSet<(T, Uuid)>,
    /// Tombstoned tags (observed removals).
    removed: BTreeSet<Uuid>,
}

/// An op-based OR-Set update (for the Week-9 causal broadcast).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ORSetOp<T> {
    /// Add `element` under the fresh unique `tag`.
    Add { element: T, tag: Uuid },
    /// Tombstone the observed `tags`.
    Remove { tags: BTreeSet<Uuid> },
}

/// Abstract state mirroring `ORSet.tla`'s per-replica `added` and `removed`.
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
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` iff `x` is currently present: it has an observed-added tag that is
    /// not tombstoned.
    pub fn contains(&self, x: &T) -> bool {
        self.added
            .iter()
            .any(|(e, tag)| e == x && !self.removed.contains(tag))
    }

    /// Iterate the distinct present elements.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        let mut present: BTreeSet<&T> = BTreeSet::new();
        for (e, tag) in &self.added {
            if !self.removed.contains(tag) {
                present.insert(e);
            }
        }
        present.into_iter()
    }

    /// Number of distinct present elements.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// `true` iff no element is present.
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Remove `x`: tombstone exactly the tags of `x` this replica has OBSERVED.
    /// Returns the op to broadcast.
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
    /// Add `x` under a fresh unique tag. Returns the op to broadcast.
    pub fn add(&mut self, x: T) -> ORSetOp<T> {
        let tag = Uuid::new_v4();
        self.added.insert((x.clone(), tag));
        ORSetOp::Add { element: x, tag }
    }

    /// Apply a remote op.
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

    /// Merge another set's state in by union of both the added and tombstone
    /// sets.
    pub fn merge(&mut self, other: &ORSet<T>) {
        self.added.extend(other.added.iter().cloned());
        self.removed.extend(other.removed.iter().copied());
    }

    /// The refinement mapping to `tla/ORSet.tla`.
    ///
    /// If this mapping is faithful and the spec is verified, the implementation
    /// inherits the spec's properties: `TombstonesObserved` (a tag is tombstoned
    /// only if observed — the add-wins mechanism) and `Monotonic` (the state
    /// grows in the join-semilattice — convergence), both model-checked with
    /// TLC.
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
        a.add(7u8); // tag t1
        let mut b = a.clone(); // b observes t1
        a.add(7u8); // tag t2, only at a (concurrent with b's remove)
        b.remove(&7u8); // tombstones t1 (observed), NOT t2
        let mut m = a.clone();
        m.merge(&b);
        assert!(m.contains(&7u8)); // add wins: t2 survives
    }

    #[test]
    fn remove_wins_when_it_observed_the_add() {
        let mut a = ORSet::new();
        a.add(7u8); // t1
        let mut b = a.clone(); // observes t1
        b.remove(&7u8); // tombstones t1; no concurrent add
        let mut m = a.clone();
        m.merge(&b);
        assert!(!m.contains(&7u8)); // remove wins
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
        /// Convergence (SEC): merging in either direction yields the same state,
        /// and the same `contains` answers — for any two-replica interleaving.
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

        /// Idempotence: merging with itself changes nothing.
        #[test]
        fn merge_is_idempotent(a_ops in ops()) {
            let a = build(&a_ops);
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(aa.tla_state(), a.tla_state());
        }

        /// Add-wins: a fresh add concurrent with a remove ALWAYS survives the
        /// merge, regardless of the prior history.
        #[test]
        fn concurrent_add_always_wins(base in ops(), x in 0u8..4) {
            let mut a = build(&base);
            let mut b = a.clone();
            a.add(x);       // fresh tag at a, unobserved by b
            b.remove(&x);   // tombstones only what b observed
            let mut m = a.clone();
            m.merge(&b);
            prop_assert!(m.contains(&x));
        }

        /// TombstonesObserved (the spec invariant): every tombstoned tag is one
        /// the replica has actually observed — never a fabricated removal.
        #[test]
        fn tombstones_are_observed(a_ops in ops(), b_ops in ops()) {
            let a = build(&a_ops);
            let b = build(&b_ops);
            let mut m = a.clone();
            m.merge(&b);
            let tags = added_tags(&m);
            prop_assert!(m.tla_state().removed.iter().all(|t| tags.contains(t)));
        }

        /// MessagePack round-trip.
        #[test]
        fn msgpack_roundtrip(a_ops in ops()) {
            let s = build(&a_ops);
            let bytes = rmp_serde::to_vec(&s).unwrap();
            let back: ORSet<u8> = rmp_serde::from_slice(&bytes).unwrap();
            prop_assert_eq!(s, back);
        }
    }
}
