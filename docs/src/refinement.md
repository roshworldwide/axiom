# The refinement mapping

A TLA+ spec and a Rust program are different artifacts written in different languages. Verifying the spec tells you nothing about the code unless something *connects* the two. In Axiom that connection is the **refinement mapping**: a small, explicit function from a Rust value to the abstract state its spec talks about.

Every CRDT in `crates/axiom-core` exposes a `tla_state()` method. It returns a plain data structure that mirrors the spec's state variable — no behavior, just the abstract value. For the G-Counter in `crates/axiom-core/src/gcounter.rs`, the spec state is one replica's knowledge vector `counts[r] ∈ [Replicas → Nat]`, and `tla_state()` returns exactly that:

```rust
pub fn tla_state(&self) -> TlaGCounterState {
    TlaGCounterState { counts: self.counts.clone() }
}
```

The claim a refinement mapping makes is: *if this mapping is faithful and the spec is verified, the implementation inherits the spec's properties.* The mapping is where the burden of proof actually lands — so Axiom exercises it two ways.

## Way 1: property tests phrased on `tla_state()`

The proptest cases compare abstract states, not raw Rust structs. That matters because Rust normalizes its representation (the G-Counter never stores a zero component), so comparing internal fields could spuriously disagree where the *spec* states agree. Phrasing assertions on `tla_state()` compares at the spec's altitude.

So the commutativity test mirrors the TLAPS-machine-proved `MergeCommutative` from `tla/GCounterProofs.tla` by checking lattice states are order-independent:

```rust
prop_assert_eq!(ab.tla_state(), ba.tla_state());
```

This is property-tested (proptest), randomized — not a proof. It checks that the Rust merge *agrees* with the property the spec proves; it does not re-derive that property.

## Way 2: trace replay

The second technique pins a concrete scenario on both sides and demands they agree. It works in two stages.

First, `tla/traces/GCounterTrace.tla` defines a *deterministic* behavior: a fixed `Script` of seven operations (increments and merges), walked by a program counter `pc`, reusing `GCounterBase.MergeVec` — the same merge operator model-checked in `tla/GCounter.tla` and machine-proved commutative in `tla/GCounterProofs.tla`. The spec also authors an `Expected` final state by hand, and asserts an invariant:

```tla
TraceMatches == Done => counts = Expected
```

Running TLC on this scenario and getting **no error** confirms that `Expected` is exactly what the spec computes for `Script`. This is a clean, bounded TLC run pinning a fixture — trace-validated, **not** an unbounded proof. TLC isn't searching a vast state space here; the behavior is a single deterministic line. It is "compile-checking" the `Expected` value against the spec's own semantics.

Second, `crates/axiom-core/tests/trace_replay.rs` replays the *same* `Script` on the Rust implementation — one `GCounter` per replica, the same seven ops in order — and asserts each replica reaches the same `Expected`, compared component-by-component through `tla_state()`:

```rust
let got = reps[&r].tla_state().counts;
let want = &trace.expected[&r.to_string()];
// ...compare each component, defaulting missing entries to 0
```

The crucial property is that **neither side derives the expected state from the other**. The TLA+ author wrote `Expected`; TLC pinned it; the Rust test reproduces it independently. They are two computations of the same operation-sequence-to-state mapping, and they match. That is what "refinement validated by trace replay" means.

## Scope

Trace replay covers the G-Counter (component counts compare directly), the OR-Set (per-replica **membership**, so the Uuid-vs-`<<replica,counter>>` tag encoding is irrelevant), and the RGA (the **visible id sequence** plus tombstone set, with the trace's `<<counter,replica>>` ids fed into the implementation via `insert_after_with_id` so the id tie-break matches the spec's). Each comparison is at the type's observable abstraction, never raw internal encoding. The OR-Set and RGA replays each have a negativity check: perturb the trace — drop the OR-Set's concurrent re-add, or the RGA's delete — and the match fails, proving those two positive tests are not vacuous. The G-Counter replay has no negativity check, so its non-vacuity is not independently demonstrated.
