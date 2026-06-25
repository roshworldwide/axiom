# CLAUDE.md — Axiom project context

> This file is permanent context for Claude Code. Read it at the start of every
> session. It encodes what Axiom is, how it's built, and — most importantly —
> the **claims policy** that keeps the project credible.

## What Axiom is

Axiom is a **formally verified CRDT runtime**: each Conflict-free Replicated
Data Type is built twice —

1. as a **TLA+ specification** (a mathematical model, model-checked with TLC and,
   where feasible, machine-proved with TLAPS), and
2. as a **production-quality Rust implementation**,

connected by an explicit **refinement mapping**: every public CRDT exposes a
`tla_state()` method whose result mirrors the abstract state of its TLA+ spec.
If that mapping is faithful and the spec is verified, the implementation
inherits the spec's checked properties.

The rare, valuable part is doing the **spec first** and keeping the
spec↔code connection explicit and machine-checkable. Do not invert this order.

## Repository layout

```
axiom/
├── Cargo.toml                 # workspace (resolver 2, shared [workspace.package])
├── rust-toolchain.toml        # pins stable + rustfmt + clippy
├── .github/workflows/ci.yml   # fmt, clippy, build, test + TLC on every *.tla
├── LICENSE-APACHE, LICENSE-MIT # dual licensed (Rust ecosystem norm)
├── README.md                  # public-facing; expanded over time
├── CLAUDE.md                  # this file
├── tla/                       # TLA+ specs + .cfg model files + SETUP.md
├── crates/
│   └── axiom-core/            # the CRDTs, vector clock, causal broadcast
├── crates/axiom-py/           # (Phase 4) PyO3 bindings
├── docs/                      # (Phase 4) "The Axiom Book" (mdBook)
└── paper/                     # (Phase 4) the paper
```

## Architecture & data flow

```
TLA+ specs ──refinement mapping──▶ Rust core ──▶ tests ──▶ client APIs
(tla/*.tla)   (tla_state())        (CRDTs +       (proptest +   (Rust + Python)
              + .cfg model         vector clock +  TLC trace
                checked by TLC)    causal broadcast) replay)
```

## Build plan (phase roadmap)

Build **in order**, one focused PR per week, green CI before moving on.

| Phase | Weeks | Deliverable |
|-------|-------|-------------|
| 0 | — | scaffold, CLAUDE.md, CI (this commit) |
| 1 | 1–4 | TLA+ specs: Counter → G/PN-Counter (+TLAPS) → OR-Set → RGA |
| 2 | 5–10 | Rust core: vector clock → counters → OR-Set → RGA+HLC → causal broadcast → verify+trace-replay (500+ proptests) |
| 3 | 11–14 | Acoustic Auth protocol spec + attacker models (replay/relay/freshness) |
| 4 | 15–20 | paper, PyO3 bindings, The Axiom Book, launch |

## Conventions — Rust

- Rust **stable**, **edition 2021**. Toolchain pinned via `rust-toolchain.toml`.
- **No `unsafe`** in the CRDT core (`crates/axiom-core` carries
  `#![forbid(unsafe_code)]`).
- **CI denies warnings** (`-D warnings`) and runs `cargo fmt --all --check` and
  `cargo clippy`. Run `cargo fmt` + `cargo clippy` locally before every commit.
- Every public CRDT type exposes `tla_state()` returning an abstract state that
  mirrors its `tla/<Type>.tla` spec. Keep the two in sync.
- Serialization uses **serde + rmp-serde (MessagePack)**; every CRDT has a
  serialize→bytes→deserialize round-trip test.
- Add dependencies **only when a phase needs them** (see reference table below).

## Conventions — TLA+

- Specs live in `tla/`. Each `Foo.tla` has a matching `Foo.cfg` model file.
- CI runs TLC on every spec that has a `.cfg`.
- When TLC's state space explodes: apply `SYMMETRY` over replicas, tighten op
  bounds, or abstract the data — and **document the chosen bounds** in
  `tla/README.md`.
- Tooling (verified June 2026):
  - **tla2tools.jar v1.7.4** ("Xenophanes", 2024-08-05) — the latest *stable*
    release. Pinned download:
    `https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar`.
    Run TLC headless: `java -cp tla2tools.jar tlc2.TLC -config Foo.cfg Foo.tla`.
    TLC exits non-zero on any violation, so CI fails automatically.
  - **Java 11+** required (use `actions/setup-java@v4`, temurin, JDK 17).
  - **TLAPS (tlapm) 1.6.0-pre** rolling prebuilt tarball is the most reliable
    CI binary; from-source builds are brittle. tlapm's exit-code semantics on
    unproved obligations are less crisp than TLC's — also scan output. Treat
    TLAPS-in-CI as *medium confidence*; gate it behind its own job.

## Dependency reference (versions verified June 2026)

Add these per phase, with the noted features. Caret ranges shown.

| Crate | Version | Notes |
|-------|---------|-------|
| `serde` | `1.0.228` | feature `derive` |
| `rmp-serde` | `1.3.1` | MessagePack codec |
| `uuid` | `1.23.4` | features `v4`, `serde` (OR-Set tags) |
| `proptest` | `1.11.0` | **dev-dependency** |
| `pyo3` | `0.29.0` | Phase 4; features `extension-module`, `abi3-py39`. MSRV Rust 1.83 |
| `maturin` | `1.14.1` | Phase 4 build tool (`pip install maturin`); `[build-system] requires = ["maturin>=1.0,<2.0"]` |
| `tokio` | `1.52.3` | only if/when real networking is added |
| `quinn` | `0.11.11` | features `runtime-tokio`, `rustls-ring`; only if real QUIC transport is added |
| `mdbook` | `0.5.3` | Phase 4 docs (`cargo install --version 0.5.3 mdbook`) |

> Note: the Week-9 causal broadcast is a **simulated** in-memory network for
> testing — it does **not** require tokio/quinn. Add those only if real
> networking is ever built.

## Refinement mapping requirement

For each CRDT, define an abstract `Tla<Type>State` type that mirrors the spec's
state, and `pub fn tla_state(&self) -> Tla<Type>State`. Document it with the
claims-policy wording: *"If this mapping is faithful and the TLA+ spec is
verified, the implementation inherits the spec's properties."* Strengthen the
connection with TLC **trace-replay** fixtures (Week 10), and keep the README
claim at **"trace-validated,"** not "proved."

## CLAIMS POLICY (read this before writing any result down)

In code comments, docs, README, and the paper, use precise, distinct language:

- **"model-checked with TLC up to N replicas / M ops"** — for TLC results
  (always **bounded**; state the bounds).
- **"machine-checked proof via TLAPS"** — ONLY where a TLAPS proof actually
  exists.
- **"property-tested with proptest (K cases)"** — for proptest.
- **"refinement mapping validated by trace replay"** — for the Rust↔TLA+ trace
  tests.

Never write **"proven correct"** (unbounded) unless a TLAPS proof backs it.
**Prefer understatement.** Calibrated honesty is the whole point — a smaller,
honest result beats a big overclaim.

## Definition of Done (every phase)

- `cargo build` and `cargo test` are green.
- `cargo clippy` is clean (warnings denied) and `cargo fmt --check` passes.
- The relevant TLC config(s) pass (and any TLAPS proof checks).
- Docs updated (CLAUDE.md / README / `tla/README.md` bounds tables as needed).
- One focused commit with the suggested message; CI green before moving on.

## House rules

- No `unsafe` in CRDT core. CI denies warnings. `cargo fmt` + `clippy` before
  every commit.
- Every CRDT exposes `tla_state()`; keep it in sync with its `tla/` spec.
- Distinguish **model-checked (TLC, bounded)** vs **machine-proved (TLAPS)** vs
  **property-tested (proptest)** vs **trace-validated** — never conflate them.
- Each week = one focused PR with green CI before moving on.
- When TLC explodes: reduce to 2 replicas, cut ops, add `SYMMETRY`, add a state
  constraint — and document the bounds.
- When proptest finds a counterexample, it shrinks to a minimal case — that's
  gold. Fix the impl or the spec; don't silence the test.

## Local toolchain setup (current machine state)

As of scaffolding, this machine has **no Rust toolchain and no Java** installed.
To run the Definition-of-Done checks locally you need:

- **Rust** (rustup): `curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh`
  — `rust-toolchain.toml` then provisions stable + rustfmt + clippy.
- **Java 11+** (for TLC): e.g. `brew install temurin` (macOS), plus
  `tla2tools.jar` v1.7.4 (see `tla/SETUP.md`, added in Phase 1).

CI installs both; local installation is only needed to reproduce checks offline.
