//! Positive-Negative counter (PN-Counter), refining [`tla/PNCounter.tla`].
//!
//! Two [`GCounter`]s: `p` accumulates increments, `n` accumulates decrements.
//! The value is `p.value() - n.value()` as an `i64` and so **may decrease** —
//! that is the whole point of PN over G. A merge never fabricates an op (each
//! side is a grow-only counter merged by component-wise max).
//!
//! [`tla/PNCounter.tla`]: ../../../../tla/PNCounter.tla

use serde::{Deserialize, Serialize};

use crate::gcounter::{GCounter, GCounterOp, TlaGCounterState};
use crate::ReplicaId;

/// A counter supporting both increment and decrement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PNCounter {
    p: GCounter,
    n: GCounter,
}

/// An op-based PN-Counter update: an increment or a decrement delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PNCounterOp {
    Inc(GCounterOp),
    Dec(GCounterOp),
}

/// Abstract state mirroring `tla/PNCounter.tla` — the pair of knowledge vectors
/// `P[r]` and `N[r]` for this replica.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TlaPNCounterState {
    pub p: TlaGCounterState,
    pub n: TlaGCounterState,
}

impl TlaPNCounterState {
    /// The net value, mirroring the spec's `Value(r) = sum(P[r]) - sum(N[r])`.
    pub fn value(&self) -> i64 {
        self.p.value() as i64 - self.n.value() as i64
    }
}

impl PNCounter {
    /// A fresh counter owned by `id`, with value zero.
    pub fn new(id: ReplicaId) -> Self {
        Self {
            p: GCounter::new(id),
            n: GCounter::new(id),
        }
    }

    /// Increment by one, returning the op to broadcast.
    pub fn increment(&mut self) -> PNCounterOp {
        PNCounterOp::Inc(self.p.increment())
    }

    /// Decrement by one, returning the op to broadcast.
    pub fn decrement(&mut self) -> PNCounterOp {
        PNCounterOp::Dec(self.n.increment())
    }

    /// The net value `P - N` (may be negative).
    pub fn value(&self) -> i64 {
        self.p.value() as i64 - self.n.value() as i64
    }

    /// Apply a remote op to the appropriate side.
    pub fn apply(&mut self, op: &PNCounterOp) {
        match op {
            PNCounterOp::Inc(o) => self.p.apply(o),
            PNCounterOp::Dec(o) => self.n.apply(o),
        }
    }

    /// Merge another counter's state in (component-wise max on both sides).
    pub fn merge(&mut self, other: &PNCounter) {
        self.p.merge(&other.p);
        self.n.merge(&other.n);
    }

    /// The refinement mapping to `tla/PNCounter.tla`.
    ///
    /// If this mapping is faithful and the spec is verified, the implementation
    /// inherits the spec's properties — in particular `NoFabrication` (a merge
    /// never invents an op), model-checked with TLC. Note the *value* is
    /// intentionally **not** monotonic (decrements lower it); only the
    /// underlying `P` and `N` knowledge vectors grow.
    pub fn tla_state(&self) -> TlaPNCounterState {
        TlaPNCounterState {
            p: self.p.tla_state(),
            n: self.n.tla_state(),
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
    fn value_can_decrease() {
        let mut c = PNCounter::new(r(0));
        c.increment();
        assert_eq!(c.value(), 1);
        c.decrement();
        c.decrement();
        assert_eq!(c.value(), -1); // decrements lower the value: not monotonic
        assert_eq!(c.value(), c.tla_state().value());
    }

    fn pnop() -> impl Strategy<Value = PNCounterOp> {
        (any::<bool>(), 0u64..3, 1u64..5).prop_map(|(inc, rep, count)| {
            let o = GCounterOp {
                replica: r(rep),
                count,
            };
            if inc {
                PNCounterOp::Inc(o)
            } else {
                PNCounterOp::Dec(o)
            }
        })
    }

    fn pncounter() -> impl Strategy<Value = PNCounter> {
        (0u64..3, prop::collection::vec(pnop(), 0..8)).prop_map(|(id, ops)| {
            let mut c = PNCounter::new(r(id));
            for op in &ops {
                c.apply(op);
            }
            c
        })
    }

    proptest! {
        /// Commutativity: the abstract state is independent of merge order.
        #[test]
        fn merge_is_commutative(a in pncounter(), b in pncounter()) {
            let mut ab = a.clone();
            ab.merge(&b);
            let mut ba = b.clone();
            ba.merge(&a);
            prop_assert_eq!(ab.tla_state(), ba.tla_state());
        }

        /// Idempotence: merging with itself changes nothing.
        #[test]
        fn merge_is_idempotent(a in pncounter()) {
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(aa.tla_state(), a.tla_state());
        }

        /// The underlying P and N knowledge vectors are monotone under merge
        /// (mirrors PNCounter.tla), even though the value is not.
        #[test]
        fn merge_grows_both_sides(a in pncounter(), b in pncounter()) {
            let mut m = a.clone();
            m.merge(&b);
            let ms = m.tla_state();
            prop_assert!(ms.p.value() >= a.tla_state().p.value());
            prop_assert!(ms.p.value() >= b.tla_state().p.value());
            prop_assert!(ms.n.value() >= a.tla_state().n.value());
            prop_assert!(ms.n.value() >= b.tla_state().n.value());
        }

        /// MessagePack round-trip.
        #[test]
        fn msgpack_roundtrip(a in pncounter()) {
            let bytes = rmp_serde::to_vec(&a).unwrap();
            let back: PNCounter = rmp_serde::from_slice(&bytes).unwrap();
            prop_assert_eq!(a, back);
        }
    }
}
