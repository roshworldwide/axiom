# Vector clocks & causal broadcast

The CRDTs in earlier chapters converge because their merges are commutative, associative, and idempotent. But that guarantee only kicks in once an operation actually arrives. This chapter covers the plumbing that decides *when* — three small pieces of supporting machinery: vector clocks, a hybrid logical clock, and a simulated causal-broadcast layer.

## Vector clocks: who-saw-what

A vector clock answers "did event A happen before event B, or are they concurrent?" It maps each replica to a count of the events that replica has produced, as known here. The implementation lives in `crates/axiom-core/src/vector_clock.rs`.

```rust
let mut a = VectorClock::new();
a.increment(ReplicaId(0)); // replica 0 did something
a.increment(ReplicaId(1)); // and we've seen one event from replica 1
```

The map is kept **normalized** — a component equal to `0` is never stored — so two clocks are structurally equal exactly when they are semantically equal.

The interesting part is the order. Vector clocks form a *partial* order, not a total one:

- `a` **happens-before** `b` (`a < b`) when every component of `a` is `<=` the matching component of `b`, and at least one is strictly less.
- `a` and `b` are **concurrent** when neither happens-before the other and they are not equal — each "got ahead" on a different replica.

`partial_cmp` walks both clocks once and returns `Some(Less | Equal | Greater)` or `None` for concurrent. The two helpers read directly off that:

```rust
a.happens_before(&b)   // Some(Less)
a.concurrent_with(&b)  // None
```

`merge` takes the component-wise maximum — the join in the vector-clock lattice. It is property-tested to be commutative, associative-via-idempotence, and an upper bound of both operands, and `happens_before` is property-tested to be irreflexive and transitive.

## Hybrid logical clocks: ids that order *and* timestamp

The RGA (`tla/RGA.tla`, `crates/axiom-core/src/rga.rs`) needs element ids that are globally unique *and* totally ordered, ideally close to real time so a human reading a log sees sensible ordering. A vector clock gives a partial order, which is the wrong shape. So RGA ids use a **hybrid logical clock**, in `crates/axiom-core/src/hlc.rs`.

An `Hlc` is a physical-time `wall` (millis since the epoch) plus a logical `counter` that breaks ties within the same millisecond:

```rust
pub struct Hlc { pub wall: u64, pub counter: u32 }
```

`HlcClock::tick` issues the next timestamp, always strictly greater than every one it has issued before — so ids are monotonic and unique even if the system clock stalls or runs backward. `observe` advances the clock past a timestamp learned from a peer, keeping it ahead of what it has seen. Pair an `Hlc` with a `ReplicaId` and you get the totally-ordered `ElementId` the RGA sorts by.

A nice detail: if a hostile peer ships a maxed-out `counter`, `incremented` rolls into the next logical millisecond instead of wrapping or panicking. Strictly-increasing ticks are property-tested across arbitrary (including backward) clock readings.

## Causal broadcast (CBCAST): deliver in the right order

Op-based CRDTs need each operation delivered *after* its causal predecessors. The simulated layer in `crates/axiom-core/src/causal_broadcast.rs` implements the Birman–Schiper–Stephenson / CBCAST rule. Every broadcast op carries the sender's vector clock; a receiver `p` delivers a message from sender `s` only when:

1. it is the **next** message from `s`: `msg.clock[s] == V_p[s] + 1`, and
2. every causally-prior message has already been delivered: `msg.clock[k] <= V_p[k]` for all `k != s`.

Messages that arrive early are **buffered** and reconsidered each time the clock advances, so delivery is always in causal order no matter how the transport reorders or delays things:

```rust
b.receive(m2);            // op2 arrives first -> buffered, nothing delivered
b.receive(m1);            // op1 arrives -> [op1, op2] delivered in order
```

This is a testing harness: an in-memory `Network` holds in-flight messages and hands them out in arbitrary order, modeling reordering and delay. (Real transport arrives in a later phase.)

The payoff is the headline property: op-based G-Counters and OR-Sets **converge under arbitrary network reordering** when ops flow through CBCAST. This is **property-tested** (proptest), not proved — the suite drives three replicas through randomized op/deliver actions, flushes the network, and asserts all replicas reach the same `tla_state` with no message left stuck in a buffer.
