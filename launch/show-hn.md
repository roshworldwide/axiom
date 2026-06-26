# Show HN: Axiom — CRDTs specified in TLA+, implemented in Rust, connected by refinement

Axiom is a small CRDT runtime built spec-first. Each data type — G-Counter,
PN-Counter, OR-Set, RGA (a sequence) — is written in TLA+ and model-checked with
TLC, then implemented in Rust with an explicit `tla_state()` *refinement mapping*
back to the spec. The mapping is exercised two ways: property tests phrased on
that abstract state, and **trace replay** — a trace whose final state is pinned
by TLC, replayed on the Rust implementation and checked to match.

The thing I cared most about is **not overclaiming**. So, precisely:

- **model-checked (TLC, bounded):** 62,039 distinct states across 6 specs, no
  invariant violations. This is exhaustive *up to finite bounds*, not a proof.
- **machine-proved (TLAPS, unbounded):** exactly two narrow lemmas — G-Counter
  merge commutativity (11 obligations) and acoustic-auth freshness (3) — not
  whole data types.
- **property-tested:** 31 properties, 62,000 generated cases on the full nightly
  run (per-commit CI runs a faster 7,936-case subset).
- **trace-validated:** G-Counter, OR-Set, and RGA (each with a negativity check).

There's also a security case study — an acoustic-authentication protocol — showing
the same method carries from data structures to a protocol. Each defense (replay,
relay, freshness-under-clock-skew) is shown *load-bearing*: delete the check and
TLC produces the attack. And there are PyO3 Python bindings for multi-agent /
multi-process shared state.

What it is **not**: there is no unbounded proof of CRDT convergence — convergence
is model-checked within finite bounds and property-tested, not proved — and the
refinement mapping is *validated*, not proved. The README, the paper, and the
mdBook ("The Axiom Book") all state exactly what is and isn't established, in
those words.

I'd genuinely like feedback on (1) the refinement-mapping + trace-replay approach
as a lightweight way to keep a verified spec and real code from drifting apart,
and (2) where the line should sit between honest reporting and overclaiming in
"verified" software.

Repo: https://github.com/roshworldwide/axiom · The Axiom Book:
https://roshworldwide.github.io/axiom · Paper draft: `paper/axiom.md`
