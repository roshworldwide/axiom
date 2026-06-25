--------------------------- MODULE AcousticAuth ---------------------------
(***************************************************************************)
(* Axiom — Phase 3.  An acoustic authentication protocol: a verifier issues *)
(* single-use, time-bounded, environment-bound challenge tokens; a          *)
(* co-located prover captures and returns one to be accepted.               *)
(*                                                                          *)
(* Week 11 modeled the honest protocol. Week 12 (here) adds an ACTIVE        *)
(* adversary that can eavesdrop ("capture") any issued token and re-present  *)
(* it — a replay attack — and verifies the protocol resists it. Every        *)
(* acceptance, honest or adversarial, funnels through the single Accept(t,v) *)
(* guard, so the invariants pin down whether the protocol's checks suffice:  *)
(* drop the single-use check and TLC finds the replay.                       *)
(*                                                                          *)
(*   Replay : enable capture + same-environment re-presentation (Week 12).   *)
(*   Relay  : also enable cross-environment re-presentation     (Week 13).   *)
(***************************************************************************)
EXTENDS Naturals

CONSTANTS
    Tokens,    \* finite set of token identifiers
    Envs,      \* finite set of acoustic environments / locations
    MaxTime,   \* time bound (keeps the model finite)
    TTL,       \* token lifetime: a token is fresh for < TTL ticks
    Replay,    \* BOOLEAN: enable the capture + replay adversary?
    Relay      \* BOOLEAN: also enable the cross-environment relay adversary?

ASSUME TTL \in Nat \ {0}
ASSUME MaxTime \in Nat
ASSUME Replay \in BOOLEAN /\ Relay \in BOOLEAN

VARIABLES
    time,        \* current logical time, 0 .. MaxTime
    issued,      \* set of tokens that have been requested/issued
    issuedAt,    \* issuedAt[t] : the time t was issued
    env,         \* env[t] : the environment t was issued for
    expired,     \* set of tokens the verifier has expired (garbage-collected)
    accepts,     \* accepts[t] : how many times t has been accepted (0 or 1)
    acceptedIn,  \* acceptedIn[t] : the environments t was accepted in
    acceptedAt,  \* acceptedAt[t] : the time t was accepted
    captured     \* set of tokens the adversary has eavesdropped

vars == <<time, issued, issuedAt, env, expired, accepts, acceptedIn,
          acceptedAt, captured>>

\* A token is fresh iff issued within the last TTL ticks.
Fresh(t) == (time - issuedAt[t]) < TTL

(***************************************************************************)
(* The verifier, located in environment `v`, accepts token `t`. This is the *)
(* protocol's entire security check; EVERY acceptance goes through it. The   *)
(* `accepts[t] = 0` conjunct is single-use enforcement — the replay defense. *)
(***************************************************************************)
Accept(t, v) ==
    /\ t \in issued
    /\ t \notin expired
    /\ Fresh(t)
    /\ env[t] = v
    /\ accepts[t] = 0
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

\* The legitimate prover returns its token to a co-located verifier.
VerifyToken == \E t \in issued : Accept(t, env[t])

ExpireTokens ==
    \E t \in issued :
        /\ t \notin expired
        /\ ~Fresh(t)
        /\ expired' = expired \cup {t}
        /\ UNCHANGED <<time, issued, issuedAt, env, accepts, acceptedIn,
                       acceptedAt, captured>>

Tick ==
    /\ time < MaxTime
    /\ time' = time + 1
    /\ UNCHANGED <<issued, issuedAt, env, expired, accepts, acceptedIn,
                   acceptedAt, captured>>

\* ---- Adversary -------------------------------------------------------

\* Eavesdrop: the adversary captures the value of any issued token.
CaptureToken ==
    \E t \in issued :
        /\ captured' = captured \cup {t}
        /\ UNCHANGED <<time, issued, issuedAt, env, expired, accepts,
                       acceptedIn, acceptedAt>>

\* Replay: re-present a captured token to a verifier in its own environment.
\* Accept rejects an already-accepted token (accepts[t] = 0 fails).
ReplayCaptured == \E t \in captured : Accept(t, env[t])

\* Relay (Week 13): present a captured token to a verifier in a DIFFERENT
\* environment. Accept rejects it because env[t] = v fails.
RelayCaptured == \E t \in captured, v \in Envs : v # env[t] /\ Accept(t, v)

\* ---- Spec ------------------------------------------------------------

HonestNext == RequestToken \/ VerifyToken \/ ExpireTokens \/ Tick

AnyAttacker == Replay \/ Relay
AttackerNext ==
    \/ (AnyAttacker /\ CaptureToken)
    \/ (Replay /\ ReplayCaptured)
    \/ (Relay /\ RelayCaptured)

\* The `time = MaxTime` stutter gives the bounded model a terminal self-loop.
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

\* No replay succeeds: a token is accepted at most once — even though the
\* adversary may have captured it and re-presented it any number of times.
ReplayResistance == \A t \in Tokens : accepts[t] <= 1

\* No relay succeeds: a token is only ever accepted in its own environment.
RelayResistance == \A t \in Tokens : acceptedIn[t] \subseteq {env[t]}

\* Tokens expire: an accepted token was accepted within its lifetime.
Freshness == \A t \in Tokens : accepts[t] >= 1 => (acceptedAt[t] - issuedAt[t]) < TTL
===========================================================================
