# G-Counter

The G-Counter (grow-only counter) is Axiom's flagship CRDT. It is the one component that carries the full verification stack: a model-checked safety property, a *machine-proved* (TLAPS, unbounded) merge law, property tests, and a TLC-pinned trace replayed against the Rust code. If you read only one CRDT chapter to understand how Axiom links a spec to an implementation, read this one.

## The idea

A G-Counter only ever counts up. Each replica owns one slot in a knowledge vector and increments only its own slot. The counter's *value* is the sum of all slots. To reconcile two replicas you take the component-wise maximum of their vectors — never a sum, which would double-count. That max-merge is what makes the type a CRDT: it is idempotent, commutative, and associative, so replicas converge no matter what order updates arrive in.

In `tla/GCounterBase.tla` this merge is one operator, shared by both the model and the proof:

```tla
MergeVec(u, v) == [t \in Replicas |-> Max(u[t], v[t])]
```

## The Rust API

A `GCounter` instance is **one replica's knowledge vector** — the value `counts[r]` of the spec's `counts ∈ [Replicas → [Replicas → Nat]]`. The map is normalized to never store a zero. See `crates/axiom-core/src/gcounter.rs`.

```rust
// Create a counter owned by a replica id.
pub fn new(id: ReplicaId) -> Self

// Bump this replica's own component by one; returns the op to broadcast.
pub fn increment(&mut self) -> GCounterOp

// The value: the sum of all components (the spec's `Value`).
pub fn value(&self) -> u64

// Merge another replica's full state by component-wise max (`MergeVec`).
pub fn merge(&mut self, other: &GCounter)

// The refinement mapping into tla/GCounter.tla.
pub fn tla_state(&self) -> TlaGCounterState
```

There is also `apply(&mut self, op: &GCounterOp)`, the op-based path: applying an op is a single component-wise `max`, which is idempotent and commutative, so out-of-order delivery still converges.

### Usage

```rust
use axiom_core::{gcounter::GCounter, ReplicaId};

let mut a = GCounter::new(ReplicaId(0));
let mut b = GCounter::new(ReplicaId(1));

a.increment();
a.increment();           // a's value == 2
b.increment();           // b's value == 1

a.merge(&b);             // component-wise max of the two vectors
assert_eq!(a.value(), 3);

// Merge is order-independent; merging back is a no-op (idempotent).
b.merge(&a);
assert_eq!(b.value(), 3);
```

## The refinement mapping

`tla_state()` returns the abstract knowledge vector this instance stands for, a `[Replicas → Nat]`. The bridge is deliberate: if the mapping is faithful and the spec holds, the Rust type inherits the spec's guarantees. Rust `merge` corresponds to the spec's `MergeVec`; Rust `value()` corresponds to the spec's `Value` (a sum over components).

## What's verified, and how

**Monotonic — model-checked (TLC, bounded).** `tla/GCounter.tla` states that each replica's value never decreases across any step (`Increment` adds one, `Merge` takes a max, stutter leaves it equal):

```tla
Monotonic ==
    [][ \A r \in Replicas : Value(counts'[r]) >= Value(counts[r]) ]_counts
```

TLC explored this exhaustively up to the bounds in the model — **480 distinct states** — with no violation. Bounded, not a proof for all inputs, but exhaustive within those bounds.

**MergeCommutative — machine-proved (TLAPS, unbounded).** `tla/GCounterProofs.tla` proves `MergeVec(u, v) = MergeVec(v, u)` for *all* knowledge vectors, deductively, via tlapm with the Z3 backend — **11 proof obligations**, all discharged. This is one of only two machine-proved results in Axiom. Unlike the TLC numbers, it holds without any finite bound.

**Property tests (proptest).** `gcounter.rs` cross-checks the implementation against these laws on randomized inputs: `merge_value_is_monotonic` mirrors `Monotonic`, `merge_is_commutative` mirrors the TLAPS-proved `MergeCommutative`, plus idempotence, op/merge agreement, and a MessagePack round-trip.

**Trace replay.** The G-Counter is currently the only CRDT with trace validation: a TLC-pinned trace is replayed on the Rust implementation and checked to match step-for-step — closing the loop from spec back to code.
