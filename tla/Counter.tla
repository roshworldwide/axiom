---------------------------- MODULE Counter ----------------------------
(***************************************************************************)
(* Axiom — Phase 1, Week 1 warm-up.                                        *)
(*                                                                         *)
(* A single, non-replicated counter.  Its only purpose is to exercise the  *)
(* TLA+ toolchain end to end (parse -> TLC model-check -> CI) so that the   *)
(* rest of Phase 1 can focus on the CRDTs rather than on tooling.          *)
(***************************************************************************)
EXTENDS Naturals

VARIABLE counter

(* Type invariant: the counter is always a natural number. *)
TypeOK == counter \in Nat

(* Initially the counter is zero. *)
Init == counter = 0

(* The two actions. *)
Increment == counter' = counter + 1
Read      == UNCHANGED counter

Next == Increment \/ Read

(* The temporal specification: start in Init, and every step is either      *)
(* Increment or Read (stuttering is allowed via the _counter subscript).    *)
Spec == Init /\ [][Next]_counter

(* Safety property we ask TLC to check: the counter never goes negative.    *)
NonNegative == counter >= 0

(* Bound the state space so TLC terminates; see Counter.cfg's CONSTRAINT.    *)
StateConstraint == counter <= 5
=============================================================================
