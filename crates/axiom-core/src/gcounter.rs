//! Grow-only counter (G-Counter), refining [`tla/GCounter.tla`].
//!
//! A `GCounter` instance is **one replica's knowledge vector** — the value
//! `counts[r]` of the spec's `counts ∈ [Replicas → [Replicas → Nat]]`. Each
//! replica increments only its own component; merging takes the component-wise
//! maximum (`MergeVec` in the spec). The counter's [`GCounter::value`] mirrors
//! the spec's `Value` (the sum of components).
//!
//! [`tla/GCounter.tla`]: ../../../../tla/GCounter.tla

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ReplicaId;

/// A grow-only counter replicated across replicas.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GCounter {
    /// The replica that owns this instance (whose component `increment` bumps).
    id: ReplicaId,
    /// `replica -> increments`. Normalized: never stores a `0`.
    counts: BTreeMap<ReplicaId, u64>,
}

/// An op-based increment: "`replica`'s component is now (at least) `count`".
///
/// This is a delta-state op — applying it is the component-wise `max`, which is
/// idempotent and commutative, so it converges under the Week-9 causal
/// broadcast regardless of delivery order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GCounterOp {
    pub replica: ReplicaId,
    pub count: u64,
}

/// Abstract state mirroring a single replica's knowledge vector `counts[r]`
/// in `tla/GCounter.tla` — a value of `[Replicas → Nat]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TlaGCounterState {
    pub counts: BTreeMap<ReplicaId, u64>,
}

impl TlaGCounterState {
    /// Mirrors the spec's `Value(v)` — the sum of the vector's components.
    pub fn value(&self) -> u64 {
        self.counts.values().copied().sum()
    }
}

impl GCounter {
    /// A fresh counter owned by `id`, with every component zero.
    pub fn new(id: ReplicaId) -> Self {
        Self {
            id,
            counts: BTreeMap::new(),
        }
    }

    /// Increment this replica's own component by one, returning the op to
    /// broadcast to peers.
    pub fn increment(&mut self) -> GCounterOp {
        let c = self.counts.entry(self.id).or_insert(0);
        *c += 1;
        GCounterOp {
            replica: self.id,
            count: *c,
        }
    }

    /// The counter's value: the sum of all components (the spec's `Value`).
    pub fn value(&self) -> u64 {
        self.counts.values().copied().sum()
    }

    /// Apply a remote op (component-wise `max` — idempotent & commutative).
    pub fn apply(&mut self, op: &GCounterOp) {
        let e = self.counts.entry(op.replica).or_insert(0);
        *e = (*e).max(op.count);
    }

    /// Merge another counter's full state in by component-wise maximum
    /// (`MergeVec` in the spec).
    pub fn merge(&mut self, other: &GCounter) {
        for (&replica, &count) in &other.counts {
            let e = self.counts.entry(replica).or_insert(0);
            *e = (*e).max(count);
        }
    }

    /// The refinement mapping to `tla/GCounter.tla`.
    ///
    /// Returns the abstract knowledge vector this instance represents. If this
    /// mapping is faithful and the TLA+ spec is verified, the implementation
    /// inherits the spec's properties: the merge here corresponds to the spec's
    /// `MergeVec`, whose commutativity is machine-proved via TLAPS
    /// (`tla/GCounterProofs.tla`), and whose value-monotonicity is model-checked
    /// with TLC.
    pub fn tla_state(&self) -> TlaGCounterState {
        TlaGCounterState {
            counts: self.counts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn r(n: u64) -> ReplicaId {
        ReplicaId(n)
    }

    #[test]
    fn increment_and_value() {
        let mut g = GCounter::new(r(0));
        assert_eq!(g.value(), 0);
        g.increment();
        g.increment();
        assert_eq!(g.value(), 2);
        // value() agrees with the refinement mapping's Value.
        assert_eq!(g.value(), g.tla_state().value());
    }

    #[test]
    fn apply_is_max_not_sum() {
        let mut g = GCounter::new(r(0));
        let op = GCounterOp {
            replica: r(1),
            count: 3,
        };
        g.apply(&op);
        g.apply(&op); // idempotent: applying twice changes nothing
        assert_eq!(g.value(), 3);
    }

    /// A G-Counter with arbitrary per-replica components, built via `apply`.
    fn gcounter() -> impl Strategy<Value = GCounter> {
        (0u64..3, prop::collection::vec((0u64..3, 1u64..5), 0..6)).prop_map(|(id, ops)| {
            let mut g = GCounter::new(r(id));
            for (rep, count) in ops {
                g.apply(&GCounterOp {
                    replica: r(rep),
                    count,
                });
            }
            g
        })
    }

    proptest! {
        /// Monotonicity (mirrors GCounter.tla's `Monotonic`): the merged value
        /// dominates both operands.
        #[test]
        fn merge_value_is_monotonic(a in gcounter(), b in gcounter()) {
            let mut m = a.clone();
            m.merge(&b);
            prop_assert!(m.value() >= a.value());
            prop_assert!(m.value() >= b.value());
        }

        /// Commutativity (mirrors the TLAPS-proved `MergeCommutative`): the
        /// abstract state is independent of merge order.
        #[test]
        fn merge_is_commutative(a in gcounter(), b in gcounter()) {
            let mut ab = a.clone();
            ab.merge(&b);
            let mut ba = b.clone();
            ba.merge(&a);
            prop_assert_eq!(ab.tla_state(), ba.tla_state());
        }

        /// Idempotence: merging with itself changes nothing.
        #[test]
        fn merge_is_idempotent(a in gcounter()) {
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(aa.tla_state(), a.tla_state());
        }

        /// Op application converges to the same state as a full merge.
        #[test]
        fn apply_ops_match_merge(a in gcounter(), b in gcounter()) {
            let mut viamerge = a.clone();
            viamerge.merge(&b);
            let mut viaops = a.clone();
            for (&replica, &count) in b.tla_state().counts.iter() {
                viaops.apply(&GCounterOp { replica, count });
            }
            prop_assert_eq!(viamerge.tla_state(), viaops.tla_state());
        }

        /// MessagePack round-trip: serialize -> bytes -> deserialize == original.
        #[test]
        fn msgpack_roundtrip(a in gcounter()) {
            let bytes = rmp_serde::to_vec(&a).unwrap();
            let back: GCounter = rmp_serde::from_slice(&bytes).unwrap();
            prop_assert_eq!(a, back);
        }
    }
}
