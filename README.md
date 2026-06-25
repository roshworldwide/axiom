# Axiom

**A formally verified CRDT runtime: TLA+ specifications and a Rust
implementation connected by explicit refinement mappings.**

Axiom builds each Conflict-free Replicated Data Type (CRDT) twice — first as a
mathematical model in **TLA+**, then as production-quality **Rust** — and ties
the two together with a `tla_state()` *refinement mapping* so the implementation
can be checked against the verified spec.

> **Status:** Phases 1–3 complete — TLA+ specs, the Rust core, and the
> acoustic-auth security case study. Phase 4 (paper, Python bindings, book) is
> next. See [`CLAUDE.md`](CLAUDE.md) for the build plan and conventions.

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

## Verification

Axiom is built spec-first and keeps every assurance claim calibrated to *how* it
was established. Current results:

| Layer | Technique | Result |
|-------|-----------|--------|
| TLA+ specs | **model-checked (TLC, bounded)** | Counter, G/PN-Counter, OR-Set, RGA — ~45,000 distinct states across the suite, no violations (bounds in [`tla/README.md`](tla/README.md)) |
| G-Counter merge | **machine-proved (TLAPS)** | `MergeVec(u,v) = MergeVec(v,u)` for all vectors — all 11 obligations discharged ([`tla/GCounterProofs.tla`](tla/GCounterProofs.tla)) |
| Acoustic-auth freshness | **machine-proved (TLAPS)** | real age `< TTL + MaxSkew` under bounded clock skew — all 3 obligations ([`tla/AcousticAuthProofs.tla`](tla/AcousticAuthProofs.tla)) |
| Rust core | **property-tested (proptest)** | 31 properties at 256–500 cases each (~8,900 generated cases) + 20 concrete unit tests |
| Spec ↔ code | **trace-validated** | a TLC-pinned G-Counter operation trace, replayed on the Rust impl, reproduces the spec's state ([trace_replay.rs](crates/axiom-core/tests/trace_replay.rs)) |

What is **not** claimed: there is no unbounded *proof* of CRDT convergence — it is
model-checked within finite bounds and property-tested, not proved; the OR-Set
and RGA TLC models use small replica/op bounds; and TLAPS covers only two narrow
lemmas (G-Counter merge commutativity, acoustic-auth freshness arithmetic), not
whole protocols. We prefer understatement.

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

## Documentation

- **[The Axiom Book](docs/)** — long-form docs (mdBook): CRDTs from scratch, TLA+
  from scratch, and the refinement mapping; each data-type chapter links to the
  TLA+ invariant it refines. `mdbook serve docs` to preview; CI deploys it to
  GitHub Pages from `main`.
- **[Paper draft](paper/axiom.md)** — *"From Formal Specification to Verified
  Implementation of CRDTs."*
- **[CONTRIBUTING.md](CONTRIBUTING.md)** — the spec-first workflow and the claims
  discipline.

## Python

[`crates/axiom-py`](crates/axiom-py) exposes the CRDTs to Python via PyO3 +
maturin — for multi-agent / multi-process systems (LangChain, AutoGen, …) that
need coordinator-free shared state:

```python
from axiom import GCounter, ORSet, RGA
a = GCounter(1); b = GCounter(2)
a.increment(); b.increment()
b.merge(GCounter.from_bytes(a.to_bytes()))   # exchange + merge
```

See the [Python quickstart](crates/axiom-py/README.md) (`maturin develop` to
build).

## License

Dual-licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT)
at your option.
