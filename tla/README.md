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
| `GCounter.tla` | 1 · Wk 2 | grow-only counter | ✅ model-checked (TLC) · ✅ machine-proved (TLAPS) |
| `PNCounter.tla` | 1 · Wk 2 | inc/dec counter | ✅ model-checked (TLC) |
| `ORSet.tla` | 1 · Wk 3 | observed-remove set | ✅ model-checked (TLC) |
| `RGA.tla` | 1 · Wk 4 | replicated growable array | ✅ model-checked (TLC) |
| `AcousticAuth.tla` | 3 · Wk 11–14 | acoustic auth protocol | ✅ model-checked (TLC, attacker + skew) · ✅ TLAPS freshness lemma |

Helper / proof modules (no `.cfg`, so not directly model-checked):
`GCounterBase.tla` (shared merge math), `GCounterProofs.tla` and
`AcousticAuthProofs.tla` (TLAPS proofs).

## Machine-checked proofs (TLAPS)

Beyond bounded model checking, results proved deductively hold for **all**
inputs (no finite bound):

| Theorem | Module | Tool | Result |
|---------|--------|------|--------|
| G-Counter merge is commutative — `MergeVec(u, v) = MergeVec(v, u)` for all `u, v ∈ [Replicas → Nat]` | `GCounterProofs.tla` | `tlapm` 1.6.0-pre (Z3 4.8.9) | ✅ **all 11 obligations proved** |
| Freshness under clock skew — a clock `≤ MaxSkew` behind accepting a token as fresh ⟹ real age `< TTL + MaxSkew` | `AcousticAuthProofs.tla` | `tlapm` 1.6.0-pre (Z3 4.8.9) | ✅ **all 3 obligations proved** |

The merge operator (`MergeVec`) is defined once in `GCounterBase.tla`, which
both `GCounter.tla` (model checking) and `GCounterProofs.tla` (the proof) extend
— so the theorem is about the *exact* operator TLC checks, not a copy of it.
(`RECURSIVE` and the `TLC` module are kept out of the base/proof modules because
`tlapm` cannot elaborate them.)

**How it's verified.** The `tlaps` CI job runs `tlapm` on every `*Proofs.tla`
(Linux x86_64, native). Locally on Apple Silicon — where no arm64 `tlapm` build
exists — it was verified by running the Linux `tlapm` under `linux/amd64`
emulation in Docker; `tlapm GCounterProofs.tla` reports *"All 11 obligations
proved."*

## Acoustic Auth (security case study)

Phase 3 applies the same methodology to a security protocol
(`AcousticAuth.tla`). Every acceptance — honest or adversarial — passes through
one `Accept(t, v)` guard, so a single invariant pins down each defense.

**Replay resistance (Week 12).** The adversary may *capture* the value of any
issued token and re-present it any number of times, in any order, interleaved
with honest requests and verifications — the model explores that entire space.
`Accept` admits a token only when `accepts[t] = 0` (single use), so a captured,
already-accepted token is rejected. TLC verifies `ReplayResistance`
(`accepts[t] <= 1`) holds across all **15,957 reachable states** (2 tokens,
2 envs, `MaxTime = 4`, `TTL = 2`). This is **bounded**: it shows no replay
succeeds *within these finite bounds*, not a proof for all parameters. That the
single-use check is load-bearing was confirmed by removing the `accepts[t] = 0`
conjunct — TLC then finds a short trace accepting a token twice.

**Relay resistance (Week 13).** An acoustic environment is modeled as an
opaque *fingerprint* constant `env[t]`; `Accept` requires the verifier's
environment to equal the token's. So a token captured in environment A and
relayed to a verifier in environment B is rejected. TLC confirms
`RelayResistance` (`acceptedIn[t] ⊆ {env[t]}`) holds with both adversaries
enabled — a relayed token is never accepted in a foreign environment. Removing
the `env[t] = v` conjunct makes TLC find a relay that succeeds, confirming the
check is load-bearing.

**Freshness under skew (Week 14).** A verifier's clock may run up to `MaxSkew`
behind real time, and it checks freshness with its own clock. TLC confirms
`Freshness` — an accepted token's *real* age is always `< TTL + MaxSkew` — across
the full attacker + skew model (16,853 states). The bound is tight: a check
ignoring skew (`< TTL`) is refuted by a behind-clock verifier accepting a token
of real age `TTL`. Beyond the bounded TLC result, the arithmetic core is
**machine-proved with TLAPS** (`AcousticAuthProofs.tla`, all 3 obligations) for
*all* integer times, TTLs, and skews — the one unbounded result in the case
study.

**Honest abstraction.** This is a result about the protocol's *logic*, not about
acoustics. We assume a token's fingerprint cannot be reproduced in a different
environment (modeled by distinct opaque constants); TLC verifies the protocol
rejects mismatches *given that assumption*. It does **not** justify the
assumption — that an acoustic fingerprint is genuinely environment-bound is a
physical claim outside the model.

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
| `GCounter.tla` | 3 replicas, `MaxIncrements = 2`, `SYMMETRY` | 480 distinct (4,849 generated), depth 13, no error |
| `PNCounter.tla` | 3 replicas, `MaxOps = 1`, `SYMMETRY` | 2,020 distinct (20,893 generated), depth 13, no error |
| `ORSet.tla` | 3 replicas, 2 elements, `MaxAdds = 1`, `SYMMETRY` | 7,239 distinct (115,296 generated), depth 14, no error |
| `RGA.tla` | **2 replicas, no symmetry**, `MaxInserts = 2` | 35,441 distinct (278,273 generated), depth 13, no error |
| `AcousticAuth.tla` (honest) | 2 tokens, 2 envs, `MaxTime = 4`, `TTL = 2`, no attacker | 4,109 distinct (8,350 generated), depth 11, no error |
| `AcousticAuth.tla` (attacker + skew) | 2 tokens, 2 envs, `MaxTime = 4`, `TTL = 2`, `MaxSkew = 1`, `Replay = Relay = TRUE` | 16,853 distinct (76,194 generated), depth 13, no error |

`RGA.tla` deliberately does **not** use symmetry: its tie-break is a total order
on ids (hence on replica identifiers), which makes replicas distinguishable, so
symmetry reduction would be unsound. It is bounded tightly instead (2 replicas).

The OR-Set `TombstonesObserved` invariant is non-vacuous: a throwaway negative
check (assert a tombstoned element is always absent) yields a 5-step
counterexample where two replicas add the same element concurrently, one removes
it, and after merge the other replica's concurrently-added tag survives — i.e.
the **Add wins** over the concurrent Remove.
