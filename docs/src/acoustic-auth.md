# Case study: acoustic authentication

The CRDT chapters showed the method on data types. This chapter shows it carries over to something with adversaries: a small security protocol, specified in `tla/AcousticAuth.tla`.

The setting is *acoustic authentication*. A verifier issues a single-use, time-bounded challenge token bound to its acoustic environment; a co-located prover captures the token and returns it to be accepted. We want three guarantees: a token works only once (no **replay**), only where it was issued (no cross-environment **relay**), and only while it is fresh (no **stale** acceptance), even when the verifier's clock is a little wrong.

## One guard to rule them all

The whole reason a single invariant can pin down each defense is that the spec funnels *every* acceptance — honest or adversarial — through one action, `Accept(t, v, c)`, where `v` is the verifier's environment and `c` is its (possibly skewed) clock reading:

```tla
Accept(t, v, c) ==
    /\ t \in issued
    /\ t \notin expired
    /\ env[t] = v               \* environment match
    /\ accepts[t] = 0           \* single use
    /\ WithinSkew(c)
    /\ c - issuedAt[t] < TTL    \* fresh by the verifier's OWN clock
    /\ ...
```

The legitimate prover (`VerifyToken`) and both attackers (`ReplayCaptured`, `RelayCaptured`) all call `Accept`. There is no second door. So each conjunct above is a defense, and each defense gets an invariant.

## Three defenses, each load-bearing

The adversary may *capture* the value of any issued token (`CaptureToken`) and then replay or relay it, any number of times, interleaved freely with honest actions. TLC explores that entire space. With both attackers and clock skew enabled (2 tokens, 2 envs, `MaxTime = 4`, `TTL = 2`, `MaxSkew = 1`), the attacker model is **16,853 distinct states** — model-checked (TLC, bounded), no error.

**Replay resistance.** The `accepts[t] = 0` conjunct admits a token only on its first acceptance. The invariant:

```tla
ReplayResistance == \A t \in Tokens : accepts[t] <= 1
```

A captured, already-accepted token is rejected.

**Relay impossibility.** An environment is an opaque fingerprint constant `env[t]`. The `env[t] = v` conjunct requires the verifier's environment to equal the token's, so a token captured in environment A and relayed to B is rejected:

```tla
RelayResistance == \A t \in Tokens : acceptedIn[t] \subseteq {env[t]}
```

**Freshness under bounded skew.** The verifier checks freshness against its *own* clock, which may be up to `MaxSkew` behind real time. The invariant tracks the token's *real* age:

```tla
Freshness ==
    \A t \in Tokens : accepts[t] >= 1 =>
        (acceptedAt[t] - issuedAt[t]) < TTL + MaxSkew
```

Each check is shown *load-bearing* the same way: delete the conjunct, and TLC finds the attack. Drop `accepts[t] = 0` and it finds a trace accepting a token twice. Drop `env[t] = v` and it finds a successful relay. Use `< TTL` instead of accounting for skew and a behind-clock verifier accepts a token of real age `TTL`. The invariants are not decoration; removing the code they guard produces a concrete counterexample.

## The one unbounded result

The freshness *arithmetic* is also machine-proved (TLAPS) in `tla/AcousticAuthProofs.tla` — the one deductive, unbounded result in this case study, discharged in **3 obligations** (`tlapm` 1.6.0-pre, Z3 backend). It proves, for *all* integers, that a clock at most `MaxSkew` behind real time accepting a token as fresh by its own clock (`c - issuedAt < TTL`) implies real age `< TTL + MaxSkew`. The proof is one line of algebra:

```tla
<1>1. now - issuedAt <= (c - issuedAt) + MaxSkew
  OBVIOUS
```

TLC checks the bound holds across the reachable protocol states; TLAPS shows the underlying inequality holds for every time, TTL, and skew, with no finite bound. (This and the G-Counter merge commutativity proof are the only two machine-proved results in Axiom.)

## The honest abstraction

This verifies protocol *logic*, not acoustics. An acoustic environment is modeled as an opaque constant `env[t]`; the model assumes a token's fingerprint cannot be reproduced in a different environment, and verifies the protocol rejects mismatches *given that assumption*. It does **not** justify the assumption. Time is a bounded integer counter and skew a bounded integer offset — no wall-clock physics. That an acoustic fingerprint is genuinely environment-bound is a physical claim outside the model. What Axiom establishes is the conditional: *if* environments are distinguishable and clocks are within `MaxSkew`, *then* replay, relay, and staleness are blocked by these three checks — and removing any one of them breaks it.
