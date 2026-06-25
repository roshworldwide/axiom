------------------------ MODULE AcousticAuthProofs ------------------------
(***************************************************************************)
(* Axiom — Phase 3, Week 14.  The arithmetic core of AcousticAuth's          *)
(* Freshness invariant, machine-checked with TLAPS.                         *)
(*                                                                          *)
(* Freshness is maintained under bounded clock skew: if a verifier whose     *)
(* clock `c` is at most MaxSkew behind the real time `now` accepts a token   *)
(* as fresh by its OWN clock (c - issuedAt < TTL), then the token's REAL age  *)
(* (now - issuedAt) is strictly less than TTL + MaxSkew. Unlike the TLC       *)
(* result (bounded), this holds for ALL integer times, TTLs, and skews.      *)
(*                                                                          *)
(* SCOPE: this is the time-arithmetic lemma underlying the Freshness          *)
(* invariant; it assumes the verifier's clock is within MaxSkew of real time *)
(* (the bounded-skew / bounded-delay assumption). It does not model the      *)
(* protocol's other actions — those are model-checked in AcousticAuth.tla.   *)
(***************************************************************************)
EXTENDS Integers

THEOREM FreshnessUnderSkew ==
    ASSUME NEW issuedAt \in Int,
           NEW c \in Int,
           NEW now \in Int,
           NEW TTL \in Int,
           NEW MaxSkew \in Int,
           c - issuedAt < TTL,      \* the verifier's freshness check passed
           now <= c + MaxSkew        \* the verifier clock is <= MaxSkew behind now
    PROVE  now - issuedAt < TTL + MaxSkew
PROOF
  <1>1. now - issuedAt <= (c - issuedAt) + MaxSkew
    OBVIOUS
  <1> QED
    BY <1>1
===========================================================================
