# METRICS — canonical source of truth

Every figure Axiom quotes (README, paper, book, launch copy) is derived from
source by the command/file cited here. **Regenerate with these commands before
changing a number anywhere.** Each figure carries its claim-class per the
[claims policy](CLAUDE.md#claims-policy-read-this-before-writing-any-result-down):
*model-checked (TLC, bounded)*, *machine-proved (TLAPS, unbounded)*,
*property-tested (proptest)*, *trace-validated*. A CI check
(`scripts/check_metrics.py`) fails if the README headline numbers drift from the
block at the bottom of this file.

## TLA+ modules

`ls tla/*.tla tla/traces/*.tla` → **12 modules**, classified:

| Module | Role | Has `.cfg`? |
|--------|------|-------------|
| `Counter.tla` | model-checked spec (warm-up) | yes |
| `GCounter.tla` | model-checked spec (CRDT) | yes |
| `PNCounter.tla` | model-checked spec (CRDT) | yes |
| `ORSet.tla` | model-checked spec (CRDT) | yes |
| `RGA.tla` | model-checked spec (CRDT) | yes |
| `AcousticAuth.tla` | model-checked spec (protocol) | yes |
| `GCounterTrace.tla` | trace scenario (model-checked via `TraceMatches`) | yes (`traces/`) |
| `ORSetTrace.tla` | trace scenario (model-checked) | yes (`traces/`) |
| `RGATrace.tla` | trace scenario (model-checked) | yes (`traces/`) |
| `GCounterProofs.tla` | TLAPS proof | no |
| `AcousticAuthProofs.tla` | TLAPS proof | no |
| `GCounterBase.tla` | shared helper (merge math) | no |

- **6 model-checked spec modules** (each has a `.cfg`): Counter, GCounter,
  PNCounter, ORSet, RGA, AcousticAuth.
- **4 CRDTs**: G-Counter, PN-Counter, OR-Set, RGA. (`Counter` is a warm-up;
  `AcousticAuth` is a protocol, not a CRDT.)
- **2 TLAPS proof modules**; **1 shared helper**; **3 trace scenarios**.

## Model-checked (TLC, bounded) — distinct states

Command (per spec, from `tla/`):
`java -cp tla2tools.jar tlc2.TLC -workers auto -config <X>.cfg <X>.tla`
(tla2tools **v1.7.4**). Each run reports "No error has been found."

| Spec | Distinct states |
|------|----------------:|
| `Counter` | 6 |
| `GCounter` | 480 |
| `PNCounter` | 2,020 |
| `ORSet` | 7,239 |
| `RGA` | 35,441 |
| `AcousticAuth` | 16,853 |
| **Total across the 6 spec models** | **62,039** |
| `GCounterTrace` scenario | 8 |
| `ORSetTrace` scenario | 11 |
| `RGATrace` scenario | 9 |
| **Total TLC states (incl. scenarios)** | **62,067** |

This is the **bounded subset checked on every commit** (kept fast). A separate,
documented **large-scale run** explores far more — see the next section.

## Large-scale TLC run (coverage; not in per-commit CI)

Command: `evaluation/large_run.sh` (also runnable via the manual / nightly
`.github/workflows/large-tlc.yml`). Widened models in `evaluation/configs/`;
full raw TLC output in `evaluation/large_run.md`. Reference run on **Apple M3,
8 cores, 16 GiB RAM**, TLC **2.19** (tla2tools v1.7.4), JVM 17 `-Xmx10g`,
`-workers auto`. Each model is explored to completion ("No error has been found").

| Model | Bounds (symmetry over replicas) | Distinct states |
|-------|---------------------------------|----------------:|
| `GCounter` | 3 replicas, MaxIncrements=13 | 142,934,260 |
| `PNCounter` | 3 replicas, MaxOps=3 | 48,201,700 |
| `ORSet` | 3 replicas, 2 elements, MaxAdds=2 | 19,902,877 |
| **Cumulative** | | **211,038,837** |

- **≥ 1e8 distinct states explored in a documented large-scale run on Apple M3 /
  16 GiB** — both as a **single model** (GCounter, **1.43 × 10⁸**) and
  **cumulatively** (**2.11 × 10⁸**). Per-commit CI checks only the bounded
  ~62K subset above.
- **Claim class:** *model-checked (TLC, bounded)* — larger bounds, still bounded.
  **State count is a coverage proxy, not proof strength**: a bigger count widens
  the search for counterexamples but does NOT upgrade "model-checked" to
  "proved." The unbounded guarantees come from the TLAPS lemmas; the Rust↔spec
  link from trace-replay.

## Machine-proved (TLAPS, unbounded) — obligations

Command: `tlapm tla/<M>Proofs.tla` (Linux x86_64; locally via `linux/amd64`
Docker — there is no arm64 `tlapm`). Tool: tlapm **1.6.0-pre**, Z3 backend.

| Theorem | Module | Obligations |
|---------|--------|------------:|
| G-Counter merge commutativity | `GCounterProofs.tla` | 11 |
| Freshness under bounded clock skew | `AcousticAuthProofs.tla` | 3 |
| **Total** | **2 proofs** | **14** |

## Property-tested (proptest)

Command: `cargo test --workspace` (rustc stable; `-D warnings`).

- **55 test functions**, all passing: 50 in the `axiom-core` lib unit tests +
  5 integration tests (`tests/trace_replay.rs`). (`cargo test` output.)
- Of the 55: **31 are proptest properties**; **24 are concrete unit/integration
  tests**. (Count of `#[test]` inside `proptest! { }` blocks vs outside.)
- proptest **cases**: 27 properties at the default 256 cases + 4 properties at
  500 cases (`axiom_verify.rs`, `ProptestConfig::with_cases(500)`):
  `27 × 256 + 4 × 500 = 6,912 + 2,000 = 8,912` **generated cases**.
- Python: **7 pytest** smoke tests (`cd crates/axiom-py && pytest`), separate
  from `cargo test`.

## Trace-validated

`crates/axiom-core/tests/trace_replay.rs` replays TLC-pinned op traces — each
`tla/traces/<Crdt>Trace.tla` asserts `Done => observable = Expected`, so a clean
TLC run pins the expected state:
- **G-Counter** — per-replica component counts (`GCounterTrace.tla`).
- **OR-Set** — per-replica membership / live elements (`ORSetTrace.tla`); tag
  encoding (Uuid vs `<<replica,counter>>`) is abstracted away.
- **RGA** — the visible id sequence + tombstone set (`RGATrace.tla`); the trace's
  `<<counter,replica>>` ids are fed into the impl (`insert_after_with_id`) so the
  id tie-break matches the spec's.

Each has a **negativity check** (perturb the trace → the match fails), confirming
the positive tests are not vacuous.

## Code size

- `axiom-core`: **2,002 lines** of Rust — 1,927 (`src/`, includes inline
  `#[cfg(test)]` tests) + 75 (`tests/`).
  `find crates/axiom-core/{src,tests} -name '*.rs' | xargs wc -l`
- `axiom-py`: **251 lines** (`wc -l crates/axiom-py/src/lib.rs`).
- TLA+: **740 lines** (`cat tla/*.tla tla/traces/*.tla | wc -l`).

## Docs & CI

- mdBook chapters: **11** (`ls docs/src/*.md` minus `SUMMARY.md`).
- CI jobs: **6** in `ci.yml` (rust, tlc, tlaps, python, book, metrics) + **2**
  in `pages.yml` (build, deploy) + **1** in `large-tlc.yml` (large-scale TLC,
  **manual / nightly only — not per-commit**).

---

The README must contain each line below verbatim; `scripts/check_metrics.py`
enforces it so the headline numbers can't silently diverge.

<!-- HEADLINE-START -->
- 62,039 distinct states
- 6 model-checked specs
- 4 CRDTs
- 2 TLAPS proofs
- 14 obligations
- 55 test functions
- 31 property tests
- 8,912 generated cases
<!-- HEADLINE-END -->
