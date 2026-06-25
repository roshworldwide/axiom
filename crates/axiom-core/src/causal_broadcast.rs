//! Simulated causal broadcast (CBCAST).
//!
//! Implements the Birman–Schiper–Stephenson / CBCAST delivery rule using vector
//! clocks: an operation broadcast by replica `s` carries `s`'s vector clock, and
//! a receiver `p` delivers it only once
//!
//! 1. it is the *next* message from `s` (`C[s] == V_p[s] + 1`), and
//! 2. every message that causally precedes it has already been delivered
//!    (`C[k] <= V_p[k]` for all `k != s`).
//!
//! Messages that arrive early are **buffered** until they become deliverable, so
//! operations are always delivered in causal order no matter how the network
//! reorders or delays them. This is a TESTING harness — an in-memory
//! [`Network`] stands in for real transport (that arrives in Phase 4).

use serde::{Deserialize, Serialize};

use crate::{ReplicaId, VectorClock};

/// A broadcast operation tagged with the sender and its vector clock at send
/// time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message<Op> {
    pub from: ReplicaId,
    pub clock: VectorClock,
    pub op: Op,
}

/// One replica's view of the causal-broadcast protocol: its delivered-message
/// vector clock plus a buffer of received-but-not-yet-deliverable messages.
#[derive(Clone, Debug)]
pub struct CausalProcess<Op> {
    id: ReplicaId,
    clock: VectorClock,
    pending: Vec<Message<Op>>,
}

impl<Op: Clone> CausalProcess<Op> {
    /// A process for replica `id` that has delivered nothing yet.
    pub fn new(id: ReplicaId) -> Self {
        Self {
            id,
            clock: VectorClock::new(),
            pending: Vec::new(),
        }
    }

    /// This process's replica id.
    pub fn id(&self) -> ReplicaId {
        self.id
    }

    /// This process's current delivered-message clock.
    pub fn clock(&self) -> &VectorClock {
        &self.clock
    }

    /// Broadcast `op`: advance this process's own component and stamp `op` with
    /// the resulting clock. The returned [`Message`] is shipped to peers; the
    /// caller has already applied `op` to its own local state.
    pub fn broadcast(&mut self, op: Op) -> Message<Op> {
        self.clock.increment(self.id);
        Message {
            from: self.id,
            clock: self.clock.clone(),
            op,
        }
    }

    /// Receive a message, delivering it and any now-unblocked buffered messages
    /// in causal order. Returns the ops to apply, in delivery order.
    pub fn receive(&mut self, msg: Message<Op>) -> Vec<Op> {
        self.pending.push(msg);
        let mut delivered = Vec::new();
        while let Some(idx) = self.pending.iter().position(|m| self.deliverable(m)) {
            let m = self.pending.remove(idx);
            // Precondition guarantees m.clock[m.from] == self.clock[m.from] + 1.
            self.clock.increment(m.from);
            delivered.push(m.op);
        }
        delivered
    }

    /// Number of messages currently buffered awaiting their causal predecessors.
    pub fn buffered(&self) -> usize {
        self.pending.len()
    }

    /// The CBCAST delivery condition.
    fn deliverable(&self, msg: &Message<Op>) -> bool {
        msg.clock.get(msg.from) == self.clock.get(msg.from) + 1
            && msg
                .clock
                .iter()
                .all(|(k, c)| k == msg.from || c <= self.clock.get(k))
    }
}

/// An in-memory network that holds in-flight messages and can deliver them in
/// any order (modeling reordering and delay). Causal delivery is still
/// guaranteed because each [`CausalProcess`] buffers early arrivals.
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
    /// An empty network.
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` iff no messages are in flight.
    pub fn is_empty(&self) -> bool {
        self.inflight.is_empty()
    }

    /// Number of in-flight messages.
    pub fn len(&self) -> usize {
        self.inflight.len()
    }

    /// Queue `msg` for delivery to each target replica.
    pub fn broadcast(&mut self, targets: impl IntoIterator<Item = ReplicaId>, msg: Message<Op>) {
        for t in targets {
            self.inflight.push((t, msg.clone()));
        }
    }

    /// Remove and return the in-flight message at `index` (wrapping). Choosing
    /// arbitrary indices is how a caller models reordering/delay. Returns `None`
    /// when the network is empty.
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
        // Replica 0 broadcasts op1 then op2 (op2 causally after op1).
        let mut a = CausalProcess::new(rid(0));
        let m1 = a.broadcast("op1");
        let m2 = a.broadcast("op2");

        // Replica 1 receives op2 FIRST, out of causal order.
        let mut b = CausalProcess::new(rid(1));
        assert!(b.receive(m2).is_empty()); // buffered, not delivered
        assert_eq!(b.buffered(), 1);

        // When the dependency op1 arrives, both deliver in causal order.
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

    /// Drive `n` replicas through CBCAST + a reordering [`Network`] per the
    /// action list, then flush the network, returning the final CRDT states.
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
                    let op = local_op(&mut crdts[i]); // sender applies locally
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
        // Flush every remaining message to every target.
        while let Some((t, msg)) = net.take(0) {
            let ti = t.0 as usize;
            for op in procs[ti].receive(msg) {
                apply(&mut crdts[ti], &op);
            }
        }
        // No message may be left stuck in a buffer once the network drains.
        for p in &procs {
            assert_eq!(p.buffered(), 0, "a message was left undelivered");
        }
        crdts
    }

    proptest! {
        /// Op-based G-Counters converge across replicas no matter how the
        /// network reorders delivery (causal broadcast + commutative ops).
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

        /// Op-based OR-Sets converge under arbitrary reordering too.
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
