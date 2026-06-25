# TLA+ trace fixtures

Deterministic, scripted TLA+ behaviors that **pin** the trace-replay fixtures
used by the Rust tests (`crates/axiom-core/tests/traces/`).

Each `<Crdt>Trace.tla` reuses the verified operators (e.g. `GCounterBase.MergeVec`,
the operator machine-proved commutative in `GCounterProofs.tla`) and walks a
fixed `Script` via a program counter. The invariant `TraceMatches` asserts
`Done => state = Expected`, so running TLC and getting **no error** confirms that
`Expected` is exactly what the spec computes for `Script`.

The Rust test (`tests/trace_replay.rs`) then replays the same `Script` on the
implementation and asserts it reaches the same `Expected`. So the refinement is
**validated by trace replay**: TLA+ (via TLC) and Rust independently agree on the
operation-sequence → state mapping. Neither side derives the expected state from
the other.

These specs are model-checked by the CI `tlc` job (it globs `traces/*.cfg` too).
Run locally from the `tla/` directory so `GCounterBase` resolves:

```sh
java -cp tla2tools.jar tlc2.TLC -config traces/GCounterTrace.cfg traces/GCounterTrace.tla
```

**Coverage:** `GCounter` (the canonical example — counts compare directly with
no tag/representation mismatch). The same mechanism extends to the other CRDTs
by comparing at their observable abstraction (counter value, set membership,
visible sequence) rather than raw internal state.
