# Axiom TLA+ specifications

This directory holds the formal models that the Rust implementation refines.

## Layout

- Each specification `Foo.tla` has a matching model file `Foo.cfg`.
- CI runs the TLC model checker on every `*.tla` that has a `*.cfg`.
- `SETUP.md` (added in Phase 1, Week 1) explains how to obtain the tooling and
  the difference between TLC and TLAPS.

## Specs (added phase by phase)

| Spec | Phase | What it models | Status |
|------|-------|----------------|--------|
| `Counter.tla` | 1 · Wk 1 | toolchain warm-up | ✅ model-checked (TLC) |
| `GCounter.tla` | 1 · Wk 2 | grow-only counter | _pending_ |
| `PNCounter.tla` | 1 · Wk 2 | inc/dec counter | _pending_ |
| `ORSet.tla` | 1 · Wk 3 | observed-remove set | _pending_ |
| `RGA.tla` | 1 · Wk 4 | replicated growable array | _pending_ |
| `AcousticAuth.tla` | 3 | acoustic auth protocol | _pending_ |

## Claims policy (read before writing any result down)

When recording what a spec establishes, use precise language:

- **model-checked (TLC, bounded)** — exhaustive exploration up to stated finite
  bounds. Always state the bounds (replicas, ops, constants).
- **machine-proved (TLAPS)** — a deductive, unbounded machine-checked proof.
- Never write "proven" for a TLC result. Prefer understatement.

## Model bounds

When TLC's state space explodes, we apply symmetry reduction, tighten operation
bounds, or abstract the data — and document the chosen bounds here per spec.

| Spec | Model bounds | TLC result |
|------|--------------|------------|
| `Counter.tla` | `CONSTRAINT counter <= 5` | 6 distinct states (13 generated), depth 6, no error |
