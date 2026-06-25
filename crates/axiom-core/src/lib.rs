//! # Axiom Core
//!
//! Formally specified CRDTs (Conflict-free Replicated Data Types) whose Rust
//! implementations are connected to TLA+ specifications by explicit
//! *refinement mappings*.
//!
//! Each public CRDT type will expose a `tla_state()` method returning an
//! abstract state value that mirrors the corresponding `tla/<Type>.tla`
//! specification. If that mapping is faithful and the spec is verified, the
//! implementation inherits the spec's checked properties.
//!
//! ## Claims policy
//!
//! This crate is deliberate about levels of assurance (see `CLAUDE.md` and the
//! repository `README.md` for the full policy):
//!
//! - **model-checked (TLC, bounded)** — verified by exhaustive state
//!   exploration up to stated finite bounds.
//! - **machine-proved (TLAPS)** — a deductive, unbounded machine-checked proof.
//! - **property-tested (proptest)** — validated on randomized inputs.
//! - **trace-validated** — Rust execution replays a TLC trace and matches.
//!
//! Nothing here is described as "proven correct" without a TLAPS proof to back
//! it. We prefer understatement.
//!
//! ## Module map (introduced phase by phase — see `CLAUDE.md`)
//!
//! - `vector_clock` — causal ordering (Phase 2, Week 5)
//! - `gcounter`, `pncounter` — counter CRDTs (Week 6)
//! - `orset` — observed-remove set (Week 7)
//! - `hlc`, `rga` — hybrid logical clock + replicated growable array (Week 8)
//! - `causal_broadcast` — CBCAST causal delivery (Week 9)
//! - `axiom_verify` — convergence harness + TLC trace replay (Week 10)
//!
//! The crate intentionally exports nothing yet; the scaffold compiles clean as
//! an empty library.
#![forbid(unsafe_code)]

pub mod vector_clock;

pub use vector_clock::{ReplicaId, VectorClock};
