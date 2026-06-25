--------------------------- MODULE AcousticAuth ---------------------------
(***************************************************************************)
(* Axiom — Phase 3.  An acoustic authentication protocol: a verifier issues *)
(* single-use, time-bounded, environment-bound challenge tokens; a          *)
(* co-located prover captures and returns one to be accepted.               *)
(*                                                                          *)
(* Week 11 honest protocol; Week 12 capture+replay adversary; Week 13       *)
(* cross-environment relay adversary; Week 14 (here) adds bounded CLOCK      *)
(* SKEW: a verifier's clock may differ from real time by up to MaxSkew, and  *)
(* it checks freshness with its OWN clock. Every acceptance funnels through  *)
(* the single Accept(t, v, c) guard (c = the verifier's clock reading), so   *)
(* the invariants pin down whether the protocol's checks suffice: drop the   *)
(* single-use check and TLC finds the replay; drop the environment check and *)
(* TLC finds the relay; ignore skew in the freshness bound and TLC finds a   *)
(* stale acceptance.                                                        *)
(*                                                                          *)
(* ABSTRACTION: an acoustic environment is an OPAQUE fingerprint constant    *)
(* (env[t]); we model neither acoustics nor real wall-clock physics. Time is *)
(* a bounded integer counter; skew is a bounded integer offset.             *)
(***************************************************************************)
EXTENDS Integers

CONSTANTS
    Tokens,    \* finite set of token identifiers
    Envs,      \* acoustic environments, each an opaque fingerprint constant
    MaxTime,   \* time bound (keeps the model finite)
    TTL,       \* token lifetime: a token is fresh for < TTL ticks
    MaxSkew,   \* a verifier's clock is within MaxSkew of the reference time
    Replay,    \* BOOLEAN: enable the capture + replay adversary?
    Relay      \* BOOLEAN: also enable the cross-environment relay adversary?

ASSUME TTL \in Nat \ {0}
ASSUME MaxTime \in Nat
ASSUME MaxSkew \in Nat
ASSUME Replay \in BOOLEAN /\ Relay \in BOOLEAN

VARIABLES
    time,        \* reference ("real") time, 0 .. MaxTime
    issued,      \* set of tokens that have been requested/issued
    issuedAt,    \* issuedAt[t] : the reference time t was issued
    env,         \* env[t] : the environment fingerprint t was issued for
    expired,     \* set of tokens the verifier has expired (garbage-collected)
    accepts,     \* accepts[t] : how many times t has been accepted (0 or 1)
    acceptedIn,  \* acceptedIn[t] : the environments t was accepted in
    acceptedAt,  \* acceptedAt[t] : the reference time t was accepted
    captured     \* set of tokens the adversary has eavesdropped

vars == <<time, issued, issuedAt, env, expired, accepts, acceptedIn,
          acceptedAt, captured>>

\* A verifier clock reading `c` is within MaxSkew of the reference time.
WithinSkew(c) == time <= c + MaxSkew /\ c <= time + MaxSkew

\* Possible verifier clock readings (a behind-clock can read below 0).
Clocks == (0 - MaxSkew) .. (MaxTime + MaxSkew)

(***************************************************************************)
(* The verifier, reading clock `c` in environment `v`, accepts token `t`.    *)
(* This is the protocol's entire security check; EVERY acceptance goes       *)
(* through it: single use (accepts = 0), environment match (env[t] = v), and *)
(* freshness BY THE VERIFIER'S OWN (possibly skewed) clock (c - issuedAt[t]   *)
(* < TTL).                                                                   *)
(***************************************************************************)
Accept(t, v, c) ==
    /\ t \in issued
    /\ t \notin expired
    /\ env[t] = v
    /\ accepts[t] = 0
    /\ WithinSkew(c)
    /\ c - issuedAt[t] < TTL
    /\ accepts'    = [accepts    EXCEPT ![t] = @ + 1]
    /\ acceptedIn' = [acceptedIn EXCEPT ![t] = @ \cup {v}]
    /\ acceptedAt' = [acceptedAt EXCEPT ![t] = time]
    /\ UNCHANGED <<time, issued, issuedAt, env, expired, captured>>

\* ---- Honest protocol -------------------------------------------------

RequestToken ==
    \E t \in (Tokens \ issued), e \in Envs :
        /\ issued'   = issued \cup {t}
        /\ issuedAt' = [issuedAt EXCEPT ![t] = time]
        /\ env'      = [env EXCEPT ![t] = e]
        /\ UNCHANGED <<time, expired, accepts, acceptedIn, acceptedAt, captured>>

\* The legitimate prover returns its token to a co-located verifier (whose
\* clock may itself be skewed).
VerifyToken == \E t \in issued, c \in Clocks : Accept(t, env[t], c)

\* The verifier expires a token that is stale even for the most behind clock
\* (real age >= TTL + MaxSkew), so a still-acceptable token is never removed.
ExpireTokens ==
    \E t \in issued :
        /\ t \notin expired
        /\ ~((time - issuedAt[t]) < TTL + MaxSkew)
        /\ expired' = expired \cup {t}
        /\ UNCHANGED <<time, issued, issuedAt, env, accepts, acceptedIn,
                       acceptedAt, captured>>

Tick ==
    /\ time < MaxTime
    /\ time' = time + 1
    /\ UNCHANGED <<issued, issuedAt, env, expired, accepts, acceptedIn,
                   acceptedAt, captured>>

\* ---- Adversary -------------------------------------------------------

CaptureToken ==
    \E t \in issued :
        /\ captured' = captured \cup {t}
        /\ UNCHANGED <<time, issued, issuedAt, env, expired, accepts,
                       acceptedIn, acceptedAt>>

ReplayCaptured == \E t \in captured, c \in Clocks : Accept(t, env[t], c)

RelayCaptured ==
    \E t \in captured, v \in Envs, c \in Clocks : v # env[t] /\ Accept(t, v, c)

\* ---- Spec ------------------------------------------------------------

HonestNext == RequestToken \/ VerifyToken \/ ExpireTokens \/ Tick

AnyAttacker == Replay \/ Relay
AttackerNext ==
    \/ (AnyAttacker /\ CaptureToken)
    \/ (Replay /\ ReplayCaptured)
    \/ (Relay /\ RelayCaptured)

Next == HonestNext \/ AttackerNext \/ (time = MaxTime /\ UNCHANGED vars)

Init ==
    /\ time = 0
    /\ issued = {}
    /\ issuedAt = [t \in Tokens |-> 0]
    /\ env = [t \in Tokens |-> CHOOSE e \in Envs : TRUE]
    /\ expired = {}
    /\ accepts = [t \in Tokens |-> 0]
    /\ acceptedIn = [t \in Tokens |-> {}]
    /\ acceptedAt = [t \in Tokens |-> 0]
    /\ captured = {}

Spec == Init /\ [][Next]_vars

\* ---- Invariants ------------------------------------------------------

TypeOK ==
    /\ time \in 0 .. MaxTime
    /\ issued \subseteq Tokens
    /\ expired \subseteq Tokens
    /\ captured \subseteq Tokens
    /\ issuedAt \in [Tokens -> 0 .. MaxTime]
    /\ env \in [Tokens -> Envs]
    /\ accepts \in [Tokens -> Nat]
    /\ acceptedIn \in [Tokens -> SUBSET Envs]
    /\ acceptedAt \in [Tokens -> 0 .. MaxTime]

\* No replay succeeds: a token is accepted at most once.
ReplayResistance == \A t \in Tokens : accepts[t] <= 1

\* No relay succeeds: a token is only ever accepted in its own environment.
RelayResistance == \A t \in Tokens : acceptedIn[t] \subseteq {env[t]}

\* Freshness under skew: despite a verifier clock up to MaxSkew behind real
\* time, an accepted token's REAL age is < TTL + MaxSkew (tokens still expire).
Freshness ==
    \A t \in Tokens : accepts[t] >= 1 => (acceptedAt[t] - issuedAt[t]) < TTL + MaxSkew
===========================================================================
