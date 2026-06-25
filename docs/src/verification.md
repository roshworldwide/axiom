# Verification & honest claims

This is the chapter where the claims get cashed out. Axiom calls itself "formally verified," and that phrase is doing a lot of work in this industry — usually too much. So here is the whole truth, organized by *how* each result was established, and then an equally explicit account of what is **not** claimed.

The discipline behind this chapter is simple: every assurance term means exactly one thing, and we never borrow the strength of one term to describe a result that earned a weaker one.

## The four assurance levels

| Level | What it means | What Axiom has |
|-------|---------------|----------------|
| **model-checked (TLC, bounded)** | Exhaustive state exploration up to stated finite bounds — every reachable state checked, no violation found | All five CRDT specs plus the acoustic-auth model: ~62,000 distinct states across the suite |
| **machine-proved (TLAPS)** | A deductive, *unbounded* proof — holds for all inputs, no finite bound | Two narrow lemmas, 14 proof obligations total |
| **property-tested (proptest)** | Randomized inputs checked against a property | 31 properties, ~8,900 generated cases (plus 51 test functions overall) |
| **trace-validated** | A TLC-pinned execution trace replayed on the Rust impl, matching the spec's state | The G-Counter |

### Model-checked (TLC, bounded)

TLC walks the entire reachable state space within finite bounds and reports any invariant violation. Each spec carries its own bounds, documented in `tla/README.md`:

| Spec | Distinct states |
|------|----------------:|
| `Counter.tla` | 6 |
| `GCounter.tla` | 480 |
| `PNCounter.tla` | 2,020 |
| `ORSet.tla` | 7,239 |
| `RGA.tla` | 35,441 |
| `AcousticAuth.tla` (attacker + skew) | 16,853 |

That is roughly 62,000 distinct states, every one explored, no violation found — within the stated bounds. Toolchain: `tla2tools` v1.7.4.

### Machine-proved (TLAPS)

Exactly two results are deductive, unbounded proofs — and no more:

- **G-Counter merge commutativity** (`tla/GCounterProofs.tla`): `MergeVec(u, v) = MergeVec(v, u)` for *all* vectors — 11 obligations discharged.
- **Acoustic-auth freshness arithmetic** (`tla/AcousticAuthProofs.tla`): an accepted token's real age is `< TTL + MaxSkew` for *all* integer times, TTLs, and skews — 3 obligations discharged.

Fourteen obligations, both checked by `tlapm` 1.6.0-pre with the Z3 backend. Because `MergeVec` is defined once in `GCounterBase.tla` and shared by both the model and the proof, the theorem is about the *exact* operator TLC checks.

### Property-tested (proptest)

The Rust core in `crates/axiom-core` is exercised by 31 property tests over ~8,900 randomly generated cases — commutativity, associativity, idempotence, convergence on random op interleavings. Randomized testing finds bugs; it does not prove their absence.

### Trace-validated

`crates/axiom-core/tests/trace_replay.rs` takes a TLC-pinned G-Counter trace and replays it through the Rust implementation, confirming the code reproduces the spec's state via the `tla_state()` refinement mapping. This currently covers the G-Counter only.

## What is NOT claimed

Read this section as carefully as the table above.

- **No unbounded proof of CRDT convergence.** Convergence is model-checked within finite bounds and property-tested — not proved. There is no TLAPS theorem saying "all replicas converge for all schedules."
- **TLAPS covers only two narrow lemmas.** Merge commutativity and freshness arithmetic. It does not cover whole protocols, the OR-Set, the RGA, or end-to-end safety.
- **Refinement is validated, not verified.** The link between spec and Rust rests on trace replay (G-Counter only) and the `tla_state()` mapping — evidence the code tracks the spec, not a machine-checked refinement proof.
- **The security study abstracts the physics.** Acoustic-auth models a fingerprint as an opaque constant. TLC shows the protocol *logic* rejects replays and relays *given* that fingerprints are environment-bound. It does not justify that physical assumption.
- **Model sizes are bounded and small.** RGA uses 2 replicas and no symmetry (its total-order tie-break makes replicas distinguishable, so symmetry would be unsound). A bounded result shows no violation *within those bounds* — nothing beyond them.

## Why calibrated honesty is the point

The temptation in formal methods is to let "we ran TLC" quietly become "we proved it correct." That single slide is how "verified" loses its meaning across a whole field.

Axiom's bet is the opposite. A reader should be able to take any claim here, trace it to the technique that backs it, and find the technique strong enough to support exactly that claim and no more. Two genuine proofs, presented as two genuine proofs, are worth more than a hundred dressed-up test runs — precisely because you can trust the next claim we make. Calibrated honesty is not modesty. It is what makes the strong claims believable.
