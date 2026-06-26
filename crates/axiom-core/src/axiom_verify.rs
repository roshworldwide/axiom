//! Cross-CRDT verification harness.
//!
//! Generic convergence checking that complements the per-CRDT proptests: build
//! several replicas, merge them in different orders, and assert they all reach
//! the same state. Because every CRDT here is a state-based CvRDT (its merge is
//! a join — commutative, associative, idempotent), the joined state must be
//! independent of merge order. This is the executable counterpart of the
//! `Convergent` / `Monotonic` properties model-checked in the TLA+ specs.

/// Fold-merge the replicas selected by `order` (indices into `replicas`) into a
/// single joined value, using `merge` as the in-place join. `order` must be
/// non-empty.
pub fn join_in_order<C: Clone>(replicas: &[C], order: &[usize], merge: impl Fn(&mut C, &C)) -> C {
    let mut acc = replicas[order[0]].clone();
    for &i in order.iter().skip(1) {
        merge(&mut acc, &replicas[i]);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcounter::{GCounter, GCounterOp};
    use crate::orset::ORSet;
    use crate::pncounter::{PNCounter, PNCounterOp};
    use crate::rga::Rga;
    use crate::ReplicaId;
    use proptest::prelude::*;

    const N: usize = 4; // replicas

    fn rid(n: usize) -> ReplicaId {
        ReplicaId(n as u64)
    }

    /// Two independent permutations of `0..N`, for merging in different orders.
    fn perm() -> impl Strategy<Value = Vec<usize>> {
        Just((0..N).collect::<Vec<_>>()).prop_shuffle()
    }

    /// `N` lists of random ops, one list per replica.
    fn op_lists<T: std::fmt::Debug + Clone>(
        each: impl Strategy<Value = T>,
        max_len: usize,
    ) -> impl Strategy<Value = Vec<Vec<T>>> {
        prop::collection::vec(prop::collection::vec(each, 0..max_len), N..=N)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::proptest_cases()))]

        /// G-Counters merge to the same state regardless of merge order.
        #[test]
        fn gcounter_merges_converge(
            specs in op_lists((0usize..N, 1u64..6), 6),
            a in perm(),
            b in perm(),
        ) {
            let replicas: Vec<GCounter> = specs.iter().enumerate().map(|(i, ops)| {
                let mut g = GCounter::new(rid(i));
                for &(r, c) in ops {
                    g.apply(&GCounterOp { replica: rid(r), count: c });
                }
                g
            }).collect();
            let ja = join_in_order(&replicas, &a, |x, y| x.merge(y));
            let jb = join_in_order(&replicas, &b, |x, y| x.merge(y));
            prop_assert_eq!(ja.tla_state(), jb.tla_state());
        }

        /// PN-Counters merge to the same state regardless of merge order.
        #[test]
        fn pncounter_merges_converge(
            specs in op_lists((any::<bool>(), 0usize..N, 1u64..6), 6),
            a in perm(),
            b in perm(),
        ) {
            let replicas: Vec<PNCounter> = specs.iter().enumerate().map(|(i, ops)| {
                let mut c = PNCounter::new(rid(i));
                for &(inc, r, ct) in ops {
                    let o = GCounterOp { replica: rid(r), count: ct };
                    c.apply(&if inc { PNCounterOp::Inc(o) } else { PNCounterOp::Dec(o) });
                }
                c
            }).collect();
            let ja = join_in_order(&replicas, &a, |x, y| x.merge(y));
            let jb = join_in_order(&replicas, &b, |x, y| x.merge(y));
            prop_assert_eq!(ja.tla_state(), jb.tla_state());
        }

        /// OR-Sets merge to the same state regardless of merge order.
        #[test]
        fn orset_merges_converge(
            specs in op_lists((any::<bool>(), 0u8..4), 6),
            a in perm(),
            b in perm(),
        ) {
            let replicas: Vec<ORSet<u8>> = specs.iter().map(|ops| {
                let mut s = ORSet::new();
                for &(add, x) in ops {
                    if add { s.add(x); } else { s.remove(&x); }
                }
                s
            }).collect();
            let ja = join_in_order(&replicas, &a, |x, y| x.merge(y));
            let jb = join_in_order(&replicas, &b, |x, y| x.merge(y));
            prop_assert_eq!(ja.tla_state(), jb.tla_state());
        }

        /// RGAs merge to the same visible sequence regardless of merge order.
        #[test]
        fn rga_merges_converge(
            specs in op_lists((any::<bool>(), 0u8..6), 6),
            a in perm(),
            b in perm(),
        ) {
            let replicas: Vec<Rga<u8>> = specs.iter().enumerate().map(|(i, ops)| {
                let mut r = Rga::new(rid(i));
                for &(insert, v) in ops {
                    if insert {
                        let idx = (v as usize) % (r.len() + 1);
                        r.insert(idx, v);
                    } else if !r.is_empty() {
                        let id = r.ids()[(v as usize) % r.len()];
                        r.delete(id);
                    }
                }
                r
            }).collect();
            let ja = join_in_order(&replicas, &a, |x, y| x.merge(y));
            let jb = join_in_order(&replicas, &b, |x, y| x.merge(y));
            let sa: Vec<u8> = ja.to_vec().into_iter().copied().collect();
            let sb: Vec<u8> = jb.to_vec().into_iter().copied().collect();
            prop_assert_eq!(sa, sb);
        }
    }
}
