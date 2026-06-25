# Introduction

Most projects that touch formal methods make a quiet choice: they either verify a *model* or they test *code*. A team writes a TLA+ specification, model-checks it, publishes a paper — and the running system is a separate artifact that nobody proved anything about. Or a team writes excellent property tests against real code, and the "design" lives only in someone's head. Either way, there is a gap between the thing that was checked and the thing that runs.

Axiom is unusual because it refuses that choice. It is a formally-verified CRDT runtime that keeps **both** sides — a mathematical model and a production Rust implementation — and connects them with an explicit, machine-checkable link.

## Building each CRDT twice

A CRDT (Conflict-free Replicated Data Type) is a data structure that several replicas can edit independently and merge without coordination, always converging to the same value. They are subtle: the guarantees depend on algebraic properties that are easy to state and easy to get wrong.

So Axiom builds each CRDT twice.

1. First as a **TLA+ specification** — a mathematical model of the data type and its operations, living in `tla/` (`tla/GCounter.tla`, `tla/ORSet.tla`, `tla/RGA.tla`, and so on).
2. Then as **Rust** in `crates/axiom-core` (e.g. `crates/axiom-core/src/gcounter.rs`).

The two are tied together by a **refinement mapping**: every public CRDT exposes a `tla_state()` method whose result mirrors the abstract state of its TLA+ spec. The discipline is: *if that mapping is faithful and the spec is verified, the implementation inherits the spec's checked properties.* To pin the mapping down beyond a method signature, Axiom replays a TLC-generated execution trace on the Rust code and checks that the states match — **trace replay**, currently covering the G-Counter.

```
TLA+ specs ──refinement mapping──▶ Rust core ──▶ tests ──▶ client APIs
(tla/*.tla)   (tla_state())        (CRDTs +       (proptest +   (Rust + Python)
                                    vector clock +  TLC trace
                                    causal broadcast) replay)
```

## Spec-first discipline

The order matters, and Axiom does not invert it:

1. **Specify** the CRDT in TLA+.
2. **Model-check** it with TLC (and, where a small lemma is tractable, **machine-prove** it with TLAPS).
3. **Implement** it in Rust.
4. **Refine**: connect the implementation to the spec via `tla_state()`, then validate that link with trace replay.

Writing the model first means the design is pinned down — and checked — before a single line of the data structure is written. The code then has something concrete to be faithful to.

## What this book covers

The chapters ahead walk through Axiom from the ground up: a short primer on CRDTs and on TLA+; each specification (`tla/Counter.tla` through `tla/RGA.tla`) and what TLC explored in it; the two TLAPS proofs; the Rust core and its property tests; the refinement mapping and trace replay that join the two halves; the acoustic-auth security case study (`tla/AcousticAuth.tla`); and the Python bindings. The toolchain throughout is tla2tools v1.7.4 for TLC, tlapm 1.6.0-pre (Z3 backend) for TLAPS, Rust stable, and PyO3 for the Python layer.

## Calibrated honesty

One rule shapes everything in this book: **never overstate what was established, and be precise about *how*.** These four terms are kept strictly distinct and are never used interchangeably:

- **model-checked (TLC, bounded)** — exhaustive state exploration up to finite bounds. Across the suite that is roughly 62,000 distinct states (Counter 6, GCounter 480, PNCounter 2,020, ORSet 7,239, RGA 35,441, AcousticAuth 16,853).
- **machine-proved (TLAPS)** — a deductive, *unbounded* proof. Exactly two results have this: G-Counter merge commutativity (11 obligations, `tla/GCounterProofs.tla`) and acoustic-auth freshness arithmetic (3 obligations).
- **property-tested (proptest)** — randomized testing: 31 property tests, about 8,900 generated cases.
- **trace-validated** — a TLC-pinned trace replayed on the Rust impl, matching the spec's state.

We never write "proven" or "proved" for a TLC or proptest result; those are reserved for the two TLAPS proofs above. There is no unbounded proof of full CRDT convergence — convergence is model-checked within finite bounds and property-tested, not proved. Throughout, we prefer understatement. A smaller honest claim is worth more than a big one you cannot back up — and that is the whole point of Axiom.
