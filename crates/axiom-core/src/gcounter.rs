use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ReplicaId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GCounter {
    id: ReplicaId,
    counts: BTreeMap<ReplicaId, u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GCounterOp {
    pub replica: ReplicaId,
    pub count: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TlaGCounterState {
    pub counts: BTreeMap<ReplicaId, u64>,
}

impl TlaGCounterState {
    pub fn value(&self) -> u64 {
        self.counts.values().copied().sum()
    }
}

impl GCounter {
    pub fn new(id: ReplicaId) -> Self {
        Self {
            id,
            counts: BTreeMap::new(),
        }
    }

    pub fn increment(&mut self) -> GCounterOp {
        let c = self.counts.entry(self.id).or_insert(0);
        *c += 1;
        GCounterOp {
            replica: self.id,
            count: *c,
        }
    }

    pub fn value(&self) -> u64 {
        self.counts.values().copied().sum()
    }

    pub fn apply(&mut self, op: &GCounterOp) {
        let e = self.counts.entry(op.replica).or_insert(0);
        *e = (*e).max(op.count);
    }

    pub fn merge(&mut self, other: &GCounter) {
        for (&replica, &count) in &other.counts {
            let e = self.counts.entry(replica).or_insert(0);
            *e = (*e).max(count);
        }
    }

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
        g.apply(&op);
        assert_eq!(g.value(), 3);
    }

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
        #![proptest_config(ProptestConfig::with_cases(crate::proptest_cases()))]

        #[test]
        fn merge_value_is_monotonic(a in gcounter(), b in gcounter()) {
            let mut m = a.clone();
            m.merge(&b);
            prop_assert!(m.value() >= a.value());
            prop_assert!(m.value() >= b.value());
        }

        #[test]
        fn merge_is_commutative(a in gcounter(), b in gcounter()) {
            let mut ab = a.clone();
            ab.merge(&b);
            let mut ba = b.clone();
            ba.merge(&a);
            prop_assert_eq!(ab.tla_state(), ba.tla_state());
        }

        #[test]
        fn merge_is_idempotent(a in gcounter()) {
            let mut aa = a.clone();
            aa.merge(&a);
            prop_assert_eq!(aa.tla_state(), a.tla_state());
        }

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

        #[test]
        fn msgpack_roundtrip(a in gcounter()) {
            let bytes = rmp_serde::to_vec(&a).unwrap();
            let back: GCounter = rmp_serde::from_slice(&bytes).unwrap();
            prop_assert_eq!(a, back);
        }
    }
}
