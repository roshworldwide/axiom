-------------------------- MODULE GCounterTrace --------------------------
(***************************************************************************)
(* A DETERMINISTIC, scripted G-Counter behavior used to pin a trace fixture *)
(* for Rust trace-replay testing.                                           *)
(*                                                                          *)
(* It reuses GCounterBase.MergeVec, so the merge here is exactly the         *)
(* operator model-checked in GCounter.tla and machine-proved commutative in *)
(* GCounterProofs.tla. A program counter `pc` walks a fixed `Script`, giving *)
(* the spec a single behavior. The invariant `TraceMatches` asserts that the *)
(* final `counts` equal the authored `Expected`; so if TLC reports NO error, *)
(* `Expected` is exactly what the spec computes for `Script`. The Rust test  *)
(* replays the same `Script` and must reproduce `Expected` — connecting the  *)
(* implementation to the spec by a replayed trace.                           *)
(***************************************************************************)
EXTENDS GCounterBase, Naturals, Sequences

\* Replicas (a CONSTANT inherited from GCounterBase) is {1, 2, 3} in the .cfg.

VARIABLES
    counts,   \* the replicated G-Counter state, [Replicas -> [Replicas -> Nat]]
    pc        \* program counter into Script (1-based)

varsT == <<counts, pc>>

\* The scripted operations:
\*   [t |-> "inc",   r  |-> <replica>]           : r increments its own component
\*   [t |-> "merge", to |-> <r>, from |-> <s>]   : r merges s's vector (MergeVec)
Script ==
    << [t |-> "inc",   r  |-> 1],
       [t |-> "inc",   r  |-> 2],
       [t |-> "inc",   r  |-> 2],
       [t |-> "merge", to |-> 1, from |-> 2],
       [t |-> "inc",   r  |-> 1],
       [t |-> "inc",   r  |-> 3],
       [t |-> "merge", to |-> 3, from |-> 1] >>

\* The expected final state, authored here and PINNED by TLC via TraceMatches.
Expected ==
    [r \in Replicas |->
        CASE r = 1 -> [s \in Replicas |-> CASE s = 1 -> 2 [] s = 2 -> 2 [] OTHER -> 0]
          [] r = 2 -> [s \in Replicas |-> CASE s = 2 -> 2 [] OTHER -> 0]
          [] OTHER -> [s \in Replicas |-> CASE s = 1 -> 2 [] s = 2 -> 2 [] OTHER -> 1]]

ZeroVec == [r \in Replicas |-> [s \in Replicas |-> 0]]

ApplyOp(a) ==
    IF a.t = "inc"
    THEN [counts EXCEPT ![a.r][a.r] = @ + 1]
    ELSE [counts EXCEPT ![a.to] = MergeVec(counts[a.to], counts[a.from])]

Init ==
    /\ counts = ZeroVec
    /\ pc = 1

Step ==
    /\ pc <= Len(Script)
    /\ counts' = ApplyOp(Script[pc])
    /\ pc' = pc + 1

Done == pc > Len(Script)

Next == Step \/ (Done /\ UNCHANGED varsT)

Spec == Init /\ [][Next]_varsT

\* If TLC reports no error, the spec computes exactly `Expected` from `Script`.
TraceMatches == Done => counts = Expected
==========================================================================
