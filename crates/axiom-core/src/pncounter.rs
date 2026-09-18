use serde::{Deserialize, Serialize};

use crate::gcounter::{GCounter, GCounterOp, TlaGCounterState};
use crate::ReplicaId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PNCounter {
    p: GCounter,
    n: GCounter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PNCounterOp {
    Inc(GCounterOp),
    Dec(GCounterOp),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TlaPNCounterState {
    pub p: TlaGCounterState,
    pub n: TlaGCounterState,
}

impl TlaPNCounterState {
    pub fn value(&self) -> i64 {
        self.p.value() as i64 - self.n.value() as i64
    }
}

impl PNCounter {
    pub fn new(id: ReplicaId) -> Self {
        Self {
            p: GCounter::new(id),
            n: GCounter::new(id),
        }
    }

    pub fn increment(&mut self) -> PNCounterOp {
        PNCounterOp::Inc(self.p.increment())
    }

    pub fn decrement(&mut self) -> PNCounterOp {
        PNCounterOp::Dec(self.n.increment())
    }

    pub fn value(&self) -> i64 {
        self.p.value() as i64 - self.n.value() as i64
    }

    pub fn apply(&mut self, op: &PNCounterOp) {
        match op {
            PNCounterOp::Inc(o) => self.p.apply(o),
            PNCounterOp::Dec(o) => self.n.apply(o),
        }
    }

    pub fn merge(&mut self, other: &PNCounter) {
        self.p.merge(&other.p);
        self.n.merge(&other.n);
    }

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
        assert_eq!(c.value(), -1);
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
        #![proptest_config(ProptestConfig::with_cases(crate::proptest_cases()))]

        #[test]
        fn merge_is_commutative(a in pncounter(), b in pncounter()) {
            let mut ab = a.clone();
            ab.merge(&b);
            let mut ba = b.clone();
            ba.merge(&a);
            prop_assert_eq!(ab.tla_state(), ba.tla_state());
        }

        #[test]
        fn merge_is_idempotent(a in pncounter()) {
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(aa.tla_state(), a.tla_state());
        }

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

        #[test]
        fn msgpack_roundtrip(a in pncounter()) {
            let bytes = rmp_serde::to_vec(&a).unwrap();
            let back: PNCounter = rmp_serde::from_slice(&bytes).unwrap();
            prop_assert_eq!(a, back);
        }
    }
}
