# Axiom

**A formally verified CRDT runtime: TLA+ specifications and a Rust
implementation connected by explicit refinement mappings.**

Axiom builds each Conflict-free Replicated Data Type (CRDT) twice — first as a
mathematical model in **TLA+**, then as production-quality **Rust** — and ties
the two together with a `tla_state()` *refinement mapping* so the implementation
can be checked against the verified spec.

> **Status:** early scaffolding. Specs and implementation land phase by phase.
> See [`CLAUDE.md`](CLAUDE.md) for the build plan and conventions.

## Why this is unusual

Most "verified" data-structure projects verify a model *or* test an
implementation. Axiom keeps both and makes the connection explicit and
machine-checkable, so a claim about the spec can be traced down to the code that
implements it.

## Architecture

```
TLA+ specs ──refinement mapping──▶ Rust core ──▶ tests ──▶ client APIs
(tla/*.tla)   (tla_state())        (CRDTs +       (proptest +   (Rust + Python)
                                    vector clock +  TLC trace
                                    causal broadcast) replay)
```

## Claims policy

Credibility depends on never overstating assurance. Throughout this repo we use
precise, distinct terms:

| Term | Meaning |
|------|---------|
| **model-checked (TLC, bounded)** | exhaustive state exploration up to stated finite bounds |
| **machine-proved (TLAPS)** | a deductive, unbounded machine-checked proof |
| **property-tested (proptest)** | validated on randomized inputs (N cases) |
| **trace-validated** | Rust replays a TLC execution trace and matches the expected state |

We never write "proven correct" without a TLAPS proof behind it. We prefer
understatement. A full "Verification" section is added once implementation
lands (Phase 2, Week 10).

## License

Dual-licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT)
at your option.
