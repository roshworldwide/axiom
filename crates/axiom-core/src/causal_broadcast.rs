use serde::{Deserialize, Serialize};

use crate::{ReplicaId, VectorClock};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message<Op> {
    pub from: ReplicaId,
    pub clock: VectorClock,
    pub op: Op,
}

#[derive(Clone, Debug)]
pub struct CausalProcess<Op> {
    id: ReplicaId,
    clock: VectorClock,
    pending: Vec<Message<Op>>,
}

impl<Op: Clone> CausalProcess<Op> {
    pub fn new(id: ReplicaId) -> Self {
        Self {
            id,
            clock: VectorClock::new(),
            pending: Vec::new(),
        }
    }

    pub fn id(&self) -> ReplicaId {
        self.id
    }

    pub fn clock(&self) -> &VectorClock {
        &self.clock
    }

    pub fn broadcast(&mut self, op: Op) -> Message<Op> {
        self.clock.increment(self.id);
        Message {
            from: self.id,
            clock: self.clock.clone(),
            op,
        }
    }

    pub fn receive(&mut self, msg: Message<Op>) -> Vec<Op> {
        self.pending.push(msg);
        let mut delivered = Vec::new();
        while let Some(idx) = self.pending.iter().position(|m| self.deliverable(m)) {
            let m = self.pending.remove(idx);
            self.clock.increment(m.from);
            delivered.push(m.op);
        }
        delivered
    }

    pub fn buffered(&self) -> usize {
        self.pending.len()
    }

    fn deliverable(&self, msg: &Message<Op>) -> bool {
        msg.clock.get(msg.from) == self.clock.get(msg.from) + 1
            && msg
                .clock
                .iter()
                .all(|(k, c)| k == msg.from || c <= self.clock.get(k))
    }
}

#[derive(Clone, Debug)]
pub struct Network<Op> {
    inflight: Vec<(ReplicaId, Message<Op>)>,
}

impl<Op> Default for Network<Op> {
    fn default() -> Self {
        Self {
            inflight: Vec::new(),
        }
    }
}

impl<Op: Clone> Network<Op> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.inflight.is_empty()
    }

    pub fn len(&self) -> usize {
        self.inflight.len()
    }

    pub fn broadcast(&mut self, targets: impl IntoIterator<Item = ReplicaId>, msg: Message<Op>) {
        for t in targets {
            self.inflight.push((t, msg.clone()));
        }
    }

    pub fn take(&mut self, index: usize) -> Option<(ReplicaId, Message<Op>)> {
        if self.inflight.is_empty() {
            None
        } else {
            Some(self.inflight.remove(index % self.inflight.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GCounter, ORSet};
    use proptest::prelude::*;

    fn rid(n: u64) -> ReplicaId {
        ReplicaId(n)
    }

    #[test]
    fn buffers_until_causal_dependency_arrives() {
        let mut a = CausalProcess::new(rid(0));
        let m1 = a.broadcast("op1");
        let m2 = a.broadcast("op2");

        let mut b = CausalProcess::new(rid(1));
        assert!(b.receive(m2).is_empty());
        assert_eq!(b.buffered(), 1);

        assert_eq!(b.receive(m1), vec!["op1", "op2"]);
        assert_eq!(b.buffered(), 0);
    }

    #[test]
    fn first_message_from_a_replica_delivers_immediately() {
        let mut a = CausalProcess::new(rid(0));
        let m = a.broadcast("hello");
        let mut b = CausalProcess::new(rid(1));
        assert_eq!(b.receive(m), vec!["hello"]);
    }

    #[derive(Clone, Debug)]
    enum Action {
        Op(u8),
        Deliver(u16),
    }

    fn actions() -> impl Strategy<Value = Vec<Action>> {
        prop::collection::vec(
            prop_oneof![
                (0u8..3).prop_map(Action::Op),
                (0u16..50).prop_map(Action::Deliver),
            ],
            0..40,
        )
    }

    fn simulate<C, Op: Clone>(
        n: usize,
        actions: &[Action],
        new_crdt: impl Fn(ReplicaId) -> C,
        local_op: impl Fn(&mut C) -> Op,
        apply: impl Fn(&mut C, &Op),
    ) -> Vec<C> {
        let mut procs: Vec<CausalProcess<Op>> = (0..n)
            .map(|i| CausalProcess::new(ReplicaId(i as u64)))
            .collect();
        let mut crdts: Vec<C> = (0..n).map(|i| new_crdt(ReplicaId(i as u64))).collect();
        let mut net: Network<Op> = Network::new();

        for a in actions {
            match a {
                Action::Op(i) => {
                    let i = (*i as usize) % n;
                    let op = local_op(&mut crdts[i]);
                    let msg = procs[i].broadcast(op);
                    let targets = (0..n).filter(|&j| j != i).map(|j| ReplicaId(j as u64));
                    net.broadcast(targets, msg);
                }
                Action::Deliver(k) => {
                    if let Some((t, msg)) = net.take(*k as usize) {
                        let ti = t.0 as usize;
                        for op in procs[ti].receive(msg) {
                            apply(&mut crdts[ti], &op);
                        }
                    }
                }
            }
        }
        while let Some((t, msg)) = net.take(0) {
            let ti = t.0 as usize;
            for op in procs[ti].receive(msg) {
                apply(&mut crdts[ti], &op);
            }
        }
        for p in &procs {
            assert_eq!(p.buffered(), 0, "a message was left undelivered");
        }
        crdts
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::proptest_cases()))]

        #[test]
        fn gcounter_converges_under_reordering(acts in actions()) {
            let crdts = simulate(
                3, &acts,
                GCounter::new,
                |c| c.increment(),
                |c, op| c.apply(op),
            );
            let s0 = crdts[0].tla_state();
            for c in &crdts {
                prop_assert_eq!(c.tla_state(), s0.clone());
            }
        }

        #[test]
        fn orset_converges_under_reordering(acts in actions()) {
            let crdts = simulate(
                3, &acts,
                |_id| ORSet::<u8>::new(),
                |c| c.add(0u8),
                |c, op| c.apply(op),
            );
            let s0 = crdts[0].tla_state();
            for c in &crdts {
                prop_assert_eq!(c.tla_state(), s0.clone());
            }
        }
    }
}
