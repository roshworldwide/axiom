--------------------------- MODULE GCounter ---------------------------
(***************************************************************************)
(* Axiom — Phase 1, Week 2.  A grow-only counter (G-Counter) CRDT,         *)
(* replicated over a set `Replicas`.                                        *)
(*                                                                          *)
(* `counts[r]` is replica r's local knowledge vector: `counts[r][s]` is the *)
(* number of increments that r knows replica s has performed.  A replica    *)
(* increments only its OWN component; a merge takes the component-wise max   *)
(* (the `MergeVec` operator shared with the TLAPS proof, in GCounterBase).   *)
(***************************************************************************)
EXTENDS GCounterBase, TLC

CONSTANT MaxIncrements   \* per-replica increment bound (keeps model finite)
ASSUME MaxIncrements \in Nat

VARIABLE counts

\* Value of a knowledge vector = sum of its components (RECURSIVE over a set).
\* This lives here (not in GCounterBase) because tlapm cannot process
\* RECURSIVE; TLC handles it fine.
RECURSIVE SumOver(_, _)
SumOver(f, S) ==
    IF S = {} THEN 0
    ELSE LET x == CHOOSE e \in S : TRUE
         IN  f[x] + SumOver(f, S \ {x})

Value(v) == SumOver(v, Replicas)

\* ---- Spec -------------------------------------------------------------

TypeOK == counts \in [Replicas -> [Replicas -> Nat]]

Init == counts = [r \in Replicas |-> [s \in Replicas |-> 0]]

Increment(r) ==
    /\ counts[r][r] < MaxIncrements
    /\ counts' = [counts EXCEPT ![r][r] = @ + 1]

Merge(r, s) ==
    counts' = [counts EXCEPT ![r] = MergeVec(counts[r], counts[s])]

Next ==
    \/ \E r \in Replicas : Increment(r)
    \/ \E r, s \in Replicas : Merge(r, s)

Spec == Init /\ [][Next]_counts

\* ---- Properties checked by TLC (bounded) ------------------------------

\* Replica identifiers are interchangeable; used for symmetry reduction.
Symm == Permutations(Replicas)

\* Each replica's value never decreases across a step: Increment adds 1, Merge
\* takes a max (>= old), and a stuttering step leaves it equal.  This is a
\* SAFETY (box-action) property, so symmetry reduction stays sound.
Monotonic ==
    [][ \A r \in Replicas : Value(counts'[r]) >= Value(counts[r]) ]_counts
=============================================================================
