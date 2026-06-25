# Contributing to Axiom

Thanks for your interest. Axiom is a formally-verified CRDT runtime, and its
value rests on **calibrated honesty** — so the contribution bar is about *claims*
as much as about code.

## Ground rules

- **No `unsafe`** in the CRDT core (`crates/axiom-core` carries
  `#![forbid(unsafe_code)]`). CI denies warnings.
- **Calibrated claims.** Use the precise terms and never conflate them:
  *model-checked* (TLC, bounded), *machine-proved* (TLAPS, unbounded),
  *property-tested* (proptest), *trace-validated*. Never write "proven" for a
  model-checked or tested result. See [`CLAUDE.md`](CLAUDE.md).
- **Spec first.** A new CRDT gets a TLA+ spec + a TLC config *before* Rust; the
  Rust type exposes `tla_state()` mirroring the spec, kept in sync with it.
- **Every change is green:** `cargo fmt --check`, `cargo clippy` (warnings
  denied), `cargo test`, and the relevant TLC / TLAPS checks pass. One focused PR
  per change.

## Running the checks

```sh
# Rust core
cargo test --workspace
cargo clippy --workspace --all-targets --all-features

# TLA+ model checking (see tla/SETUP.md for the toolchain)
java -cp tla2tools.jar tlc2.TLC -config tla/Foo.cfg tla/Foo.tla

# TLAPS proofs (Linux x86_64; see tla/README.md)
tlapm tla/FooProofs.tla

# Python bindings
cd crates/axiom-py && maturin develop && pytest

# The book
cd docs && mdbook build
```

CI runs the Rust gate, TLC on every `tla/**/*.cfg`, `tlapm` on every
`tla/*Proofs.tla`, the maturin + pytest job, and `mdbook build`.

## Good first contributions

- Larger TLC bounds for an existing spec (and document the chosen bounds).
- Extend **trace replay** to the OR-Set (set membership) or RGA (visible
  sequence).
- A new CRDT: spec + config + Rust impl + `tla_state()` + property tests.
- Tighten the docs or the paper.

## License

By contributing you agree your work is dual-licensed under
[Apache-2.0](LICENSE-APACHE) **or** [MIT](LICENSE-MIT), at the user's option.
