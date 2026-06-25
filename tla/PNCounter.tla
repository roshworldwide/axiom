-------------------------- MODULE PNCounter --------------------------
(***************************************************************************)
(* Axiom — Phase 1, Week 2.  A PN-Counter built from two G-Counters:        *)
(* `P` tracks increments, `N` tracks decrements.  The value known to        *)
(* replica r is sum(P[r]) - sum(N[r]); it CAN decrease (that is the whole    *)
(* point of PN over G).  The checked safety property is that a merge never   *)
(* fabricates an operation: no replica's knowledge of another's ops can      *)
(* exceed what that replica actually did.                                    *)
(***************************************************************************)
EXTENDS Integers, TLC

CONSTANTS Replicas, MaxOps
ASSUME MaxOps \in Nat

VARIABLES P, N

\* ---- Helpers (same merge math as GCounter) ----------------------------

Max(a, b) == IF a >= b THEN a ELSE b
MergeVec(u, v) == [t \in Replicas |-> Max(u[t], v[t])]

RECURSIVE SumOver(_, _)
SumOver(f, S) ==
    IF S = {} THEN 0
    ELSE LET x == CHOOSE e \in S : TRUE
         IN  f[x] + SumOver(f, S \ {x})

Zeros == [r \in Replicas |-> [s \in Replicas |-> 0]]

\* Net value known to replica r (may be negative; that is allowed).
Value(r) == SumOver(P[r], Replicas) - SumOver(N[r], Replicas)

\* ---- Spec -------------------------------------------------------------

TypeOK ==
    /\ P \in [Replicas -> [Replicas -> Nat]]
    /\ N \in [Replicas -> [Replicas -> Nat]]

Init == P = Zeros /\ N = Zeros

Increment(r) ==
    /\ P[r][r] < MaxOps
    /\ P' = [P EXCEPT ![r][r] = @ + 1]
    /\ N' = N

Decrement(r) ==
    /\ N[r][r] < MaxOps
    /\ N' = [N EXCEPT ![r][r] = @ + 1]
    /\ P' = P

\* State-based merge: r absorbs s's knowledge of BOTH P and N.
Merge(r, s) ==
    /\ P' = [P EXCEPT ![r] = MergeVec(P[r], P[s])]
    /\ N' = [N EXCEPT ![r] = MergeVec(N[r], N[s])]

Next ==
    \/ \E r \in Replicas : Increment(r)
    \/ \E r \in Replicas : Decrement(r)
    \/ \E r, s \in Replicas : Merge(r, s)

Spec == Init /\ [][Next]_<<P, N>>

Symm == Permutations(Replicas)

\* ---- Properties checked by TLC (bounded) ------------------------------

(***************************************************************************)
(* KEY safety property: a merge NEVER invents an operation.  An Increment    *)
(* or Decrement only ever touches the actor's own diagonal entry, and Merge  *)
(* only takes maxima, so every replica's count of what s did is bounded by   *)
(* what s itself recorded (s's diagonal).  Hence no fabricated ops.          *)
(***************************************************************************)
NoFabrication ==
    \A r, s \in Replicas :
        /\ P[r][s] <= P[s][s]
        /\ N[r][s] <= N[s][s]
=============================================================================
