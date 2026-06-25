------------------------ MODULE GCounterProofs ------------------------
(***************************************************************************)
(* Axiom — Phase 1, Week 2.  TLAPS proofs about the G-Counter merge.       *)
(*                                                                          *)
(* Separated from GCounter so that tlapm need not process TLC-only or       *)
(* RECURSIVE constructs.  It proves over the SAME `MergeVec` operator that   *)
(* GCounter model-checks, because both extend GCounterBase.                  *)
(*                                                                          *)
(* Checked with tlapm (the TLA+ Proof System).  Unlike the TLC results,      *)
(* which hold only up to the finite bounds in GCounter.cfg, this is an       *)
(* UNBOUNDED, machine-checked proof: it holds for ALL knowledge vectors.     *)
(***************************************************************************)
EXTENDS GCounterBase

\* The merge (component-wise max) is COMMUTATIVE.
THEOREM MergeCommutative ==
    ASSUME NEW u \in [Replicas -> Nat], NEW v \in [Replicas -> Nat]
    PROVE  MergeVec(u, v) = MergeVec(v, u)
PROOF
  <1>1. \A t \in Replicas : Max(u[t], v[t]) = Max(v[t], u[t])
    <2> TAKE t \in Replicas
    <2>1. u[t] \in Nat /\ v[t] \in Nat
      OBVIOUS
    <2> QED BY <2>1 DEF Max
  <1>2. MergeVec(u, v) = [t \in Replicas |-> Max(u[t], v[t])]  BY DEF MergeVec
  <1>3. MergeVec(v, u) = [t \in Replicas |-> Max(v[t], u[t])]  BY DEF MergeVec
  <1> QED BY <1>1, <1>2, <1>3
=============================================================================
