# Large-scale TLC run — reference results

This is a **documented large-scale model-checking run**, kept separate from the
fast bounded TLC that runs on every commit (~62K states; see `METRICS.md`). It
exists to put a real, reproducible number behind the claim that Axiom's specs
have been explored at scale.

Reproduce with [`evaluation/large_run.sh`](large_run.sh) (or the manual /
nightly [`large-tlc.yml`](../.github/workflows/large-tlc.yml) workflow). The
widened models live in [`evaluation/configs/`](configs/).

## Environment

| | |
|---|---|
| Machine | Apple **M3**, 8 cores, **16 GiB** RAM |
| OS / arch | macOS (Darwin), arm64 |
| TLC | **TLC2 Version 2.19** (08 Aug 2024), from `tla2tools.jar` **v1.7.4** |
| JVM | OpenJDK **17.0.19**, `-Xmx10g`, `-workers auto` (8) |
| Date | 2026-06-26 (UTC) |

Each model is widened with larger CONSTANTS and explored **to completion**
(queue drained to 0, "No error has been found"). Symmetry reduction over
replicas keeps it sound for the invariants checked.

## Results

| Model | Bounds (symmetry over replicas) | Invariants | Distinct states | Generated | Depth | Wall time |
|-------|----------------------------------|-----------|----------------:|----------:|------:|----------:|
| **GCounter** | 3 replicas, MaxIncrements=13 | `TypeOK` | **142,934,260** | 1,633,060,647 | 46 | 19m 28s |
| **PNCounter** | 3 replicas, MaxOps=3 | `TypeOK`, `NoFabrication` | **48,201,700** | 575,325,421 | 25 | 24m 24s |
| **OR-Set** | 3 replicas, 2 elements, MaxAdds=2 | `TypeOK`, `TombstonesObserved` | **19,902,877** | 455,652,310 | 22 | 18m 24s |
| **Cumulative** | | | **211,038,837** | 2,664,038,378 | | ~62 min |

- **Single model ≥ 1e8:** GCounter explored **142,934,260 distinct states**
  (1.43 × 10⁸) on its own.
- **Cumulative across the sweep:** **211,038,837 distinct states** (2.11 × 10⁸).

> The large run checks each model's `TypeOK` (and `NoFabrication` /
> `TombstonesObserved`) invariant. The per-commit CI model of each spec
> additionally checks the temporal `Monotonic` property on its small bounded
> instance; that is intentionally left off here so TLC spends the budget on
> state coverage.

## Raw TLC output (tails)

### GCounter — 3 replicas, MaxIncrements=13

```
Model checking completed. No error has been found.
1633060647 states generated, 142934260 distinct states found, 0 states left on queue.
The depth of the complete state graph search is 46.
Finished in 19min 28s at (2026-06-26 05:13:54)
```

### PNCounter — 3 replicas, MaxOps=3

```
Model checking completed. No error has been found.
575325421 states generated, 48201700 distinct states found, 0 states left on queue.
The depth of the complete state graph search is 25.
Finished in 24min 24s at (2026-06-25 22:51:06)
```

### OR-Set — 3 replicas, 2 elements, MaxAdds=2

```
Model checking completed. No error has been found.
455652310 states generated, 19902877 distinct states found, 0 states left on queue.
The depth of the complete state graph search is 22.
Finished in 18min 24s at (2026-06-25 23:16:07)
```

## What this does and does not mean

**State count is a coverage proxy, not proof strength.** Exploring 2.11 × 10⁸
distinct states means TLC checked the invariants on that many reachable
configurations of these widened, *bounded* models — it is still bounded model
checking, not an unbounded proof. The actual guarantees come from:

- the **invariants and the `Monotonic` property** checked by TLC (bounded),
- the **TLAPS lemmas** (`GCounterProofs`, `AcousticAuthProofs`) which *are*
  unbounded machine-checked proofs, and
- the **refinement / trace-replay** tying the Rust implementation to the specs.

A bigger state count buys more confidence that no small-scale counterexample was
missed; it does not, by itself, upgrade "model-checked (bounded)" to "proved."
See the [claims policy](../CLAUDE.md#claims-policy-read-this-before-writing-any-result-down).

## Notes on bounds (why these numbers)

CRDT state spaces grow in large discrete jumps, so landing *near* a target is
not always possible by tuning one knob:

- **GCounter** is grow-only (a single matrix, no decrement/tombstones), so its
  states are cheap and `MaxIncrements` is a fine-grained knob — MaxIncrements=13
  lands at 1.43 × 10⁸ and finishes in ~20 min. This is the headline model.
- **PNCounter** MaxOps=3 (48.2M) is the largest that finishes quickly; MaxOps=4
  jumps to ~10⁹ and does not finish in reasonable time on this machine.
- **OR-Set** MaxAdds=2 (19.9M) likewise; MaxAdds=3, a 4th replica, or a 3rd
  element overshoot to ~10⁹+.

These three completing runs already clear 1e8 both as a single model (GCounter)
and cumulatively, so no state-constraint tuning was needed.
