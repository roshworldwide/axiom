//! Vector clocks for tracking causality between replicas.
//!
//! A [`VectorClock`] maps each [`ReplicaId`] to the number of events that
//! replica has produced (as known here). It induces a *partial* order:
//! `a < b` means every component of `a` is `<=` the matching component of `b`
//! and at least one is strictly less — i.e. `a` *happens-before* `b`. Clocks
//! that are neither `<=` nor `>=` each other are *concurrent*.
//!
//! The map is kept **normalized** — components equal to `0` are never stored —
//! so the derived structural [`PartialEq`] coincides with semantic equality
//! (and with `partial_cmp(..) == Some(Equal)`).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Identifier of a replica. A transparent newtype over `u64` so it serializes
/// as a bare integer while staying type-distinct from ordinary counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReplicaId(pub u64);

/// A vector clock: `replica -> event count`, with missing entries meaning `0`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Invariant: never contains an entry whose value is `0`.
    entries: BTreeMap<ReplicaId, u64>,
}

impl VectorClock {
    /// The empty clock (every component is `0`).
    pub fn new() -> Self {
        Self::default()
    }

    /// The count this clock records for `replica` (`0` if unknown).
    pub fn get(&self, replica: ReplicaId) -> u64 {
        self.entries.get(&replica).copied().unwrap_or(0)
    }

    /// Record one new event at `replica`, advancing its component by one.
    pub fn increment(&mut self, replica: ReplicaId) {
        *self.entries.entry(replica).or_insert(0) += 1;
    }

    /// Merge `other` into `self` by taking the component-wise maximum (the join
    /// in the vector-clock lattice).
    pub fn merge(&mut self, other: &VectorClock) {
        for (&replica, &count) in &other.entries {
            if count == 0 {
                continue;
            }
            let entry = self.entries.entry(replica).or_insert(0);
            *entry = (*entry).max(count);
        }
    }

    /// `true` iff `self` strictly happens-before `other` (`self < other`).
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Less))
    }

    /// `true` iff `self` and `other` are concurrent — neither happens-before
    /// the other, and they are not equal.
    pub fn concurrent_with(&self, other: &VectorClock) -> bool {
        self.partial_cmp(other).is_none()
    }

    /// Iterate the non-zero `(replica, count)` entries.
    pub fn iter(&self) -> impl Iterator<Item = (ReplicaId, u64)> + '_ {
        self.entries.iter().map(|(&r, &c)| (r, c))
    }

    /// Number of replicas this clock has a (non-zero) entry for.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` iff this clock has no events recorded (it is the lattice bottom).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl PartialOrd for VectorClock {
    /// The vector-clock partial order. Returns `None` when the clocks are
    /// concurrent.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_greater = false;
        let mut other_greater = false;
        // Visiting a key from both maps twice is harmless: the comparison is
        // idempotent.
        for &replica in self.entries.keys().chain(other.entries.keys()) {
            match self.get(replica).cmp(&other.get(replica)) {
                Ordering::Greater => self_greater = true,
                Ordering::Less => other_greater = true,
                Ordering::Equal => {}
            }
            if self_greater && other_greater {
                return None; // concurrent
            }
        }
        match (self_greater, other_greater) {
            (false, false) => Some(Ordering::Equal),
            (true, false) => Some(Ordering::Greater),
            (false, true) => Some(Ordering::Less),
            (true, true) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ---- concrete unit tests ------------------------------------------

    fn clock_of(pairs: &[(u64, u64)]) -> VectorClock {
        let mut vc = VectorClock::new();
        for &(r, n) in pairs {
            for _ in 0..n {
                vc.increment(ReplicaId(r));
            }
        }
        vc
    }

    #[test]
    fn increment_advances_and_happens_after() {
        let a = clock_of(&[(0, 1), (1, 2)]);
        let mut b = a.clone();
        b.increment(ReplicaId(1));
        assert!(a.happens_before(&b));
        assert!(!b.happens_before(&a));
        assert!(!a.concurrent_with(&b));
    }

    #[test]
    fn concurrent_clocks_are_detected() {
        let a = clock_of(&[(0, 1)]); // r0 ahead
        let b = clock_of(&[(1, 1)]); // r1 ahead
        assert!(a.concurrent_with(&b));
        assert!(b.concurrent_with(&a));
        assert!(!a.happens_before(&b));
        assert!(!b.happens_before(&a));
    }

    #[test]
    fn equal_clocks_are_not_happens_before_nor_concurrent() {
        let a = clock_of(&[(0, 2), (1, 1)]);
        let b = clock_of(&[(1, 1), (0, 2)]);
        assert_eq!(a, b);
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Equal));
        assert!(!a.happens_before(&b));
        assert!(!a.concurrent_with(&b));
    }

    #[test]
    fn merge_is_componentwise_join() {
        let mut a = clock_of(&[(0, 3), (1, 1)]);
        let b = clock_of(&[(0, 1), (1, 4), (2, 2)]);
        a.merge(&b);
        assert_eq!(a.get(ReplicaId(0)), 3);
        assert_eq!(a.get(ReplicaId(1)), 4);
        assert_eq!(a.get(ReplicaId(2)), 2);
    }

    // ---- property-based tests -----------------------------------------

    /// A clock built from a random sequence of increments over replicas 0..4.
    fn clock() -> impl Strategy<Value = VectorClock> {
        prop::collection::vec(0u64..4, 0..8).prop_map(|incs| {
            let mut vc = VectorClock::new();
            for r in incs {
                vc.increment(ReplicaId(r));
            }
            vc
        })
    }

    /// A non-empty list of increments (guarantees a strict happens-before step).
    fn incs() -> impl Strategy<Value = Vec<ReplicaId>> {
        prop::collection::vec(0u64..4, 1..5).prop_map(|v| v.into_iter().map(ReplicaId).collect())
    }

    proptest! {
        /// happens-before is irreflexive: a clock never happens-before itself.
        #[test]
        fn happens_before_is_irreflexive(a in clock()) {
            prop_assert!(!a.happens_before(&a));
        }

        /// happens-before is transitive: a -> b and b -> c implies a -> c.
        #[test]
        fn happens_before_is_transitive(a in clock(), incs_b in incs(), incs_c in incs()) {
            let mut b = a.clone();
            for r in &incs_b { b.increment(*r); }
            let mut c = b.clone();
            for r in &incs_c { c.increment(*r); }
            // Non-empty increment lists make each step strict.
            prop_assert!(a.happens_before(&b));
            prop_assert!(b.happens_before(&c));
            prop_assert!(a.happens_before(&c));
        }

        /// Merge is commutative.
        #[test]
        fn merge_is_commutative(a in clock(), b in clock()) {
            let mut ab = a.clone();
            ab.merge(&b);
            let mut ba = b.clone();
            ba.merge(&a);
            prop_assert_eq!(ab, ba);
        }

        /// Merge is idempotent: merging a clock with itself changes nothing.
        #[test]
        fn merge_is_idempotent(a in clock()) {
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(aa, a);
        }

        /// Merge yields an upper bound of both operands (the join dominates).
        #[test]
        fn merge_dominates_both(a in clock(), b in clock()) {
            let mut j = a.clone();
            j.merge(&b);
            prop_assert!(a <= j);
            prop_assert!(b <= j);
        }
    }
}
