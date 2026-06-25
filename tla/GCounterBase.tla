------------------------- MODULE GCounterBase -------------------------
(***************************************************************************)
(* Axiom — Phase 1, Week 2.  Shared math for the G-Counter merge.          *)
(*                                                                          *)
(* Used by BOTH the model-checking module (GCounter) and the TLAPS proof    *)
(* module (GCounterProofs).  Deliberately kept free of RECURSIVE operators  *)
(* and of the TLC standard module, because the prover (tlapm) cannot        *)
(* elaborate those constructs.  Keeping the merge defined in exactly one     *)
(* place is what lets the proof and the model check the SAME operator.       *)
(***************************************************************************)
EXTENDS Naturals

CONSTANT Replicas        \* the set of replica identifiers

Max(a, b) == IF a >= b THEN a ELSE b

\* Component-wise max of two knowledge vectors: this IS the G-Counter merge.
MergeVec(u, v) == [t \in Replicas |-> Max(u[t], v[t])]
=============================================================================
