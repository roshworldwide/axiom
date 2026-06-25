---
title: "Axiom: From Formal Specification to Verified Implementation of CRDTs"
author: "RoSh"
date: 2026
abstract: |
  Conflict-free Replicated Data Types (CRDTs) are widely deployed, yet the gap
  between a CRDT's published correctness argument and a given production
  implementation is rarely closed: papers verify a *model*, code is *tested*, and
  nothing connects the two. Axiom is a small CRDT runtime built spec-first: each
  data type is specified in TLA+, model-checked with TLC (and, for two flagship
  lemmas, machine-proved with TLAPS), then implemented in Rust with an explicit
  `tla_state()` *refinement mapping* back to its specification. The mapping is
  exercised two ways — property-based tests phrased against the abstract state,
  and *trace replay*, in which an operation trace whose final state is pinned by
  TLC is replayed on the implementation and checked to match. We report the
  artifact (4 CRDTs, a causal-broadcast layer, ~2k lines of Rust, ~740 lines of
  TLA+) and an acoustic-authentication case study showing the method extends from
  data structures to a security protocol. Throughout we are deliberate about the
  *strength* of each claim: ~62,000 model-checked states are bounded evidence,
  not proof; only two narrow lemmas are proved unbounded. Axiom's contribution is
  less a new algorithm than a reproducible discipline for keeping a verified spec
  and a real implementation honestly connected.
---

# 1. Introduction

Conflict-free Replicated Data Types (CRDTs) let replicas of shared state accept
writes locally and converge without coordination. Their correctness — *strong
eventual consistency*: replicas that have observed the same updates are in the
same state — is subtle, depending on the algebraic properties of a merge
operation and, for operation-based variants, on causal delivery.

The literature verifies *models* of these algorithms; production systems *test*
implementations. The two artifacts are developed separately and connected only by
the implementer's confidence. This paper asks a narrower, engineering question:
**can a verified specification and a real implementation be kept explicitly
connected, cheaply, and checkably?**

Axiom answers in the affirmative for a small but non-trivial runtime. Its method
is:

1. **Specify first.** Each CRDT is written in TLA+ and model-checked with TLC;
   two flagship lemmas are machine-proved with TLAPS.
2. **Implement against the spec.** Each Rust type exposes a `tla_state()`
   *refinement mapping* returning an abstract value mirroring the spec's state.
3. **Exercise the mapping.** Properties the spec checks are re-checked on the
   implementation via property-based testing phrased on `tla_state()`; and a
   *trace whose final state is computed by TLC* is replayed on the implementation
   and required to match.

We make a point of **calibrated claims**: a model-checked result is bounded
evidence, a property test is randomized evidence, a trace-replay is a single
validated execution, and only a TLAPS proof is unbounded. Conflating these is the
usual way "verified" software overstates itself; we try not to.

**Contributions.** (i) A reproducible spec-to-implementation discipline for CRDTs
built around an explicit refinement mapping and TLC-pinned trace replay. (ii) An
open artifact: four CRDTs (G-Counter, PN-Counter, OR-Set, RGA), a hybrid logical
clock, and a causal-broadcast layer, in ~2,000 lines of Rust refining ~740 lines
of TLA+. (iii) An acoustic-authentication case study showing the method extends
to a security protocol, with replay/relay/freshness defenses each shown
load-bearing. (iv) A frank limitations section delineating exactly what is and is
not proved.

# 2. Background

**CRDTs.** A *state-based* (convergent) CRDT equips its state with a join-
semilattice; replicas merge by the least upper bound, which is commutative,
associative, and idempotent, so any order of merges converges. An *operation-
based* (commutative) CRDT broadcasts operations that, delivered in causal order,
commute. We implement both forms.

**TLA+, TLC, TLAPS.** TLA+ is a specification language for concurrent and
distributed systems. *TLC* is an explicit-state model checker: given a finite
instance it enumerates all reachable states and checks invariants — exhaustive
but **bounded**. *TLAPS* is a proof system: a successful check is a deductive,
**unbounded** proof. Distinguishing the two is central to this paper's claims.

# 3. Specifications

All specifications and model configurations live in `tla/`. A warm-up `Counter`
spec exercises the toolchain. The four CRDTs are:

- **G-Counter** (`GCounter.tla`). Per-replica vectors; increment bumps one's own
  component; merge is component-wise maximum (`MergeVec`). We check `TypeOK` and
  a box-action `Monotonic` property (each replica's value is non-decreasing).
- **PN-Counter** (`PNCounter.tla`). Two G-Counters (`P`, `N`); value is
  `sum(P) − sum(N)`, which may *decrease*. We check `NoFabrication` (a merge
  never invents an operation).
- **OR-Set** (`ORSet.tla`). Observed-remove set with *tombstones*: an `added` set
  of `(element, tag)` pairs and a `removed` tag set, both union-merged. We check
  `TombstonesObserved` (the add-wins mechanism) and `Monotonic` (convergence).
  We deliberately model the tombstone variant: the one-set "drop pairs + union
  merge" formulation lets removed pairs resurrect on the next merge and cannot
  satisfy remove-wins-when-observed.
- **RGA** (`RGA.tla`). Replicated growable array: a tree of elements rooted at an
  origin, siblings ordered by id; delete tombstones. We check `Wellformed`,
  `LinearizationOK`, and the KEY `Convergent` property (any two replicas agree on
  the relative order of every element they both hold). RGA's id tie-break makes
  replicas distinguishable, so we do **not** use symmetry reduction.

# 4. Implementation

The Rust core (`crates/axiom-core`, ~2,000 lines, `#![forbid(unsafe_code)]`)
mirrors the specifications module-for-module: a vector clock; the four CRDTs; a
hybrid logical clock (HLC) supplying RGA's globally-unique, totally-ordered ids;
and a simulated causal-broadcast (CBCAST) layer that delivers an operation only
after its causal predecessors. CRDTs serialize via `serde`/MessagePack.

The implementation is engineered, not toy: e.g., RGA's traversal is iterative
(an explicit heap stack) so that a long linear predecessor chain — the normal
shape of a text buffer — cannot overflow the call stack, a defect a multi-agent
adversarial review of the module surfaced and we fixed (and regression-tested at
100,000 elements). The HLC uses overflow-safe counter arithmetic so a maximal
(hostile-peer) timestamp cannot break monotonicity.

# 5. The Refinement Mapping

Each CRDT exposes `tla_state()` returning an abstract value that mirrors its
spec's state (e.g., a G-Counter returns its knowledge vector; an OR-Set its
`(added, removed)` sets). We connect implementation to specification two ways:

**Property tests on the abstract state.** Convergence properties are asserted on
`tla_state()`, not on the concrete struct — so that, for instance, `merge(a,b)`
and `merge(b,a)` are compared as lattice states (they may legitimately differ
only in an owner-id field). These properties echo what TLC checks: G-Counter
merge commutativity (also TLAPS-proved), value monotonicity, OR-Set add-wins, RGA
order-independent convergence.

**Trace replay.** A deterministic, scripted TLA+ behavior (`GCounterTrace.tla`)
reuses the verified `MergeVec` operator and asserts `Done ⟹ counts = Expected`.
A clean TLC run therefore *pins* `Expected` to the spec's semantics. The Rust
test replays the same operation script and requires the implementation's
`tla_state()` to reproduce `Expected`. Neither side derives the expected state
from the other; TLC and Rust independently agree on the operation→state mapping.
We call this connection **trace-validated**, not proved.

# 6. Evaluation

All numbers are produced by the repository's CI (`cargo test`; TLC on each
`.cfg`; `tlapm` on each `*Proofs.tla`).

**Model checking (TLC, bounded).** Distinct reachable states, no invariant
violations:

| Spec | Bounds | Distinct states | Depth |
|------|--------|----------------:|------:|
| `Counter` | `counter ≤ 5` | 6 | 6 |
| `GCounter` | 3 replicas, `MaxIncrements=2`, symmetry | 480 | 13 |
| `PNCounter` | 3 replicas, `MaxOps=1`, symmetry | 2,020 | 13 |
| `ORSet` | 3 replicas, 2 elements, `MaxAdds=1`, symmetry | 7,239 | 14 |
| `RGA` | 2 replicas, `MaxInserts=2` (no symmetry) | 35,441 | 13 |
| `AcousticAuth` | 2 tokens, 2 envs, `MaxTime=4`, `TTL=2`, `MaxSkew=1`, full attacker | 16,853 | 13 |
| **Total** | | **≈ 62,000** | |

**Deductive proof (TLAPS, unbounded).** Two lemmas, machine-checked by `tlapm`
1.6.0-pre with the Z3 backend:

| Theorem | Module | Obligations |
|---------|--------|------------:|
| G-Counter merge commutativity | `GCounterProofs.tla` | 11 |
| Freshness under bounded clock skew | `AcousticAuthProofs.tla` | 3 |

**Property testing (proptest).** 31 property tests at 256–500 cases each
(≈ 8,900 generated cases), plus 20 concrete unit/integration tests — 51 test
functions total, all passing under `-D warnings`. Properties include partial-
order laws of the vector clock, the four CRDTs' convergence/commutativity/
idempotence, op-based convergence under arbitrary network reordering through
CBCAST, and MessagePack round-trips.

**Trace replay (trace-validated).** One TLC-pinned G-Counter trace replays on the
implementation and matches the spec-computed final state.

**Non-vacuity.** For OR-Set, RGA, and each acoustic-auth defense, we ran
deliberately-false invariants and confirmed TLC produces a concrete
counterexample — establishing that the passing invariants constrain a *working*
system, and that each protocol check is load-bearing rather than vacuous.

# 7. Case Study: Acoustic Authentication

To test whether the method generalizes beyond data structures, we model an
acoustic authentication protocol (`AcousticAuth.tla`): a verifier issues single-
use, time-bounded, environment-bound challenge tokens; a co-located prover
returns one to be accepted. Every acceptance — honest or adversarial — funnels
through one `Accept(t, v, c)` guard, so a single invariant pins each defense:

- **Replay resistance.** An adversary may capture any issued token and re-present
  it arbitrarily; `Accept`'s single-use check (`accepts[t]=0`) rejects an already-
  accepted token. TLC verifies `accepts[t] ≤ 1` across 15,957 states.
- **Relay impossibility.** Modeling an environment as an opaque fingerprint
  constant, `Accept` requires the verifier's environment to match the token's; a
  token captured in environment A is never accepted in B.
- **Freshness under skew.** A verifier's clock may run up to `MaxSkew` behind real
  time; despite this, an accepted token's real age is `< TTL + MaxSkew`. TLC
  checks this on 16,853 states; the underlying arithmetic is TLAPS-proved for all
  integers. The bound is tight: a skew-ignoring bound (`< TTL`) is refuted by a
  behind-clock acceptance.

Each defense was confirmed load-bearing by deleting its check and observing TLC
find the corresponding attack. We are explicit that environments are modeled as
opaque constants: TLC verifies the protocol *logic* given that a fingerprint is
environment-bound; it does not model acoustics or justify that physical
assumption.

# 8. Related Work

Shapiro et al. introduced and surveyed CRDTs and strong eventual consistency;
Roh et al. proposed the RGA; Birman et al.'s CBCAST is the causal-broadcast
algorithm we simulate. On connecting specs to code, IronFleet (Hawblitzel et al.)
and Verdi (Wilcox et al.) *prove* distributed implementations correct via
refinement — a far stronger guarantee than Axiom's, at far greater proof cost.
Industrial TLA+ practice (Newcombe et al., "How Amazon Web Services Uses Formal
Methods") model-checks designs but does not connect them to code. Axiom occupies
a deliberately modest middle ground: a *lightweight, mostly-automated* connection
(refinement mapping + trace replay + property tests) that an individual engineer
can maintain, trading the strength of full refinement proof for low cost and
reproducibility.

# 9. Limitations

We state precisely what is **not** established:

- **No unbounded convergence proof.** CRDT convergence is model-checked within
  small finite bounds (e.g., RGA at 2 replicas, ≤ 2 inserts) and property-tested;
  it is **not** proved for all parameters. A bug appearing only at larger scale
  would be invisible to both.
- **TLAPS covers two narrow lemmas only** — G-Counter merge commutativity and the
  freshness arithmetic — not whole data types or the protocol.
- **The refinement mapping is validated, not verified.** That `tla_state()`
  faithfully abstracts the implementation is supported by property tests and one
  trace, not proved; trace replay currently covers the G-Counter.
- **The security case study abstracts physics.** Acoustic environments are opaque
  constants; the relay result is conditional on an unmodeled physical assumption.
- **Bounded model sizes.** State counts (~62k) are evidence within the stated
  bounds, not exhaustive over the real parameter space.

These are the honest boundaries of the artifact; closing any of them (larger
bounds, a refinement proof, set/sequence trace replay) is future work.

# 10. Conclusion

Axiom shows that a verified specification and a real implementation can be kept
explicitly and checkably connected at modest cost: an explicit refinement
mapping, property tests phrased on the abstract state, and trace replay against a
TLC-pinned final state, with two flagship lemmas proved deductively. The same
discipline carried, unchanged, from CRDTs to a security protocol. The result is
not a stronger correctness theorem than prior verified-systems work, but a more
reproducible and lower-effort way to stop a verified model and its code from
drifting apart — and a worked example of reporting assurance claims at exactly
their true strength.

# References

1. M. Shapiro, N. Preguiça, C. Baquero, M. Zawirski. *Conflict-free Replicated
   Data Types.* SSS 2011.
2. H.-G. Roh, M. Jeon, J.-S. Kim, J. Lee. *Replicated Abstract Data Types:
   Building Blocks for Collaborative Applications.* JPDC 2011 (RGA).
3. K. Birman, A. Schiper, P. Stephenson. *Lightweight Causal and Atomic Group
   Multicast.* ACM TOCS 1991 (CBCAST).
4. L. Lamport. *Specifying Systems: The TLA+ Language and Tools.* 2002.
5. C. Newcombe et al. *How Amazon Web Services Uses Formal Methods.* CACM 2015.
6. C. Hawblitzel et al. *IronFleet: Proving Practical Distributed Systems
   Correct.* SOSP 2015.
7. J. R. Wilcox et al. *Verdi: A Framework for Implementing and Formally
   Verifying Distributed Systems.* PLDI 2015.
