--------------------------- MODULE AcousticAuth ---------------------------
(***************************************************************************)
(* Axiom — Phase 3, Week 11.  An acoustic authentication protocol: a        *)
(* verifier issues single-use, time-bounded, environment-bound challenge    *)
(* tokens; a co-located prover captures and returns one to be accepted.     *)
(*                                                                          *)
(* This week models the HONEST protocol (the attacker is defined but        *)
(* disabled via the `Attacker` constant) and STATES the three safety        *)
(* invariants, which Weeks 12–14 then defend against an active adversary:   *)
(*   - ReplayResistance : a token is accepted at most once.                  *)
(*   - RelayResistance  : a token is accepted only in its own environment.   *)
(*   - Freshness        : a token is accepted only within its lifetime TTL.  *)
(*                                                                          *)
(* Every acceptance — honest or adversarial — funnels through the single    *)
(* `Accept(t, v)` guard, so these invariants pin down whether the protocol's *)
(* checks are sufficient: drop a check and TLC finds the attack.            *)
(***************************************************************************)
EXTENDS Naturals

CONSTANTS
    Tokens,    \* finite set of token identifiers
    Envs,      \* finite set of acoustic environments / locations
    MaxTime,   \* time bound (keeps the model finite)
    TTL,       \* token lifetime: a token is fresh for < TTL ticks
    Attacker   \* BOOLEAN: are the adversary actions enabled? (FALSE this week)

ASSUME TTL \in Nat \ {0}
ASSUME MaxTime \in Nat
ASSUME Attacker \in BOOLEAN

VARIABLES
    time,        \* current logical time, 0 .. MaxTime
    issued,      \* set of tokens that have been requested/issued
    issuedAt,    \* issuedAt[t] : the time t was issued
    env,         \* env[t] : the environment t was issued for
    expired,     \* set of tokens the verifier has expired (garbage-collected)
    accepts,     \* accepts[t] : how many times t has been accepted (0 or 1)
    acceptedIn,  \* acceptedIn[t] : the environments t was accepted in
    acceptedAt   \* acceptedAt[t] : the time t was accepted

vars == <<time, issued, issuedAt, env, expired, accepts, acceptedIn, acceptedAt>>

\* A token is fresh iff issued within the last TTL ticks.
Fresh(t) == (time - issuedAt[t]) < TTL

(***************************************************************************)
(* The verifier, located in environment `v`, accepts token `t`.  This is    *)
(* the protocol's entire security check; every acceptance goes through it.   *)
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
    /\ UNCHANGED <<time, issued, issuedAt, env, expired>>

\* ---- Honest protocol -------------------------------------------------

RequestToken ==
    \E t \in (Tokens \ issued), e \in Envs :
        /\ issued'   = issued \cup {t}
        /\ issuedAt' = [issuedAt EXCEPT ![t] = time]
        /\ env'      = [env EXCEPT ![t] = e]
        /\ UNCHANGED <<time, expired, accepts, acceptedIn, acceptedAt>>

\* The legitimate prover returns its token to a verifier co-located in the
\* token's own environment.
VerifyToken == \E t \in issued : Accept(t, env[t])

\* The verifier expires (garbage-collects) a stale challenge.
ExpireTokens ==
    \E t \in issued :
        /\ t \notin expired
        /\ ~Fresh(t)
        /\ expired' = expired \cup {t}
        /\ UNCHANGED <<time, issued, issuedAt, env, accepts, acceptedIn, acceptedAt>>

Tick ==
    /\ time < MaxTime
    /\ time' = time + 1
    /\ UNCHANGED <<issued, issuedAt, env, expired, accepts, acceptedIn, acceptedAt>>

\* ---- Adversary (defined now; enabled from Week 12) -------------------

\* Replay: re-present a token already accepted. `Accept` rejects it because
\* accepts[t] = 0 fails. (Week 12 develops the capture model.)
AttemptReplay == \E t \in Tokens : accepts[t] >= 1 /\ Accept(t, env[t])

\* Relay: present a token to a verifier in a DIFFERENT environment. `Accept`
\* rejects it because env[t] = v fails. (Week 13 develops the environment model.)
AttemptRelay == \E t \in issued, v \in Envs : v # env[t] /\ Accept(t, v)

\* ---- Spec ------------------------------------------------------------

HonestNext   == RequestToken \/ VerifyToken \/ ExpireTokens \/ Tick
AttackerNext == AttemptReplay \/ AttemptRelay

\* The `time = MaxTime` stutter gives the bounded model a terminal self-loop
\* (no deadlock) once the clock has run out.
Next == HonestNext \/ (Attacker /\ AttackerNext) \/ (time = MaxTime /\ UNCHANGED vars)

Init ==
    /\ time = 0
    /\ issued = {}
    /\ issuedAt = [t \in Tokens |-> 0]
    /\ env = [t \in Tokens |-> CHOOSE e \in Envs : TRUE]
    /\ expired = {}
    /\ accepts = [t \in Tokens |-> 0]
    /\ acceptedIn = [t \in Tokens |-> {}]
    /\ acceptedAt = [t \in Tokens |-> 0]

Spec == Init /\ [][Next]_vars

\* ---- Invariants (stated now; defended against the attacker in Wks 12–14) -

TypeOK ==
    /\ time \in 0 .. MaxTime
    /\ issued \subseteq Tokens
    /\ expired \subseteq Tokens
    /\ issuedAt \in [Tokens -> 0 .. MaxTime]
    /\ env \in [Tokens -> Envs]
    /\ accepts \in [Tokens -> Nat]
    /\ acceptedIn \in [Tokens -> SUBSET Envs]
    /\ acceptedAt \in [Tokens -> 0 .. MaxTime]

\* No replay succeeds: a token is accepted at most once.
ReplayResistance == \A t \in Tokens : accepts[t] <= 1

\* No relay succeeds: a token is only ever accepted in its own environment.
RelayResistance == \A t \in Tokens : acceptedIn[t] \subseteq {env[t]}

\* Tokens expire: an accepted token was accepted within its lifetime.
Freshness == \A t \in Tokens : accepts[t] >= 1 => (acceptedAt[t] - issuedAt[t]) < TTL
===========================================================================
