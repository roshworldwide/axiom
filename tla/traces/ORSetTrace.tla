--------------------------- MODULE ORSetTrace ---------------------------
(***************************************************************************)
(* Axiom — a DETERMINISTIC, scripted OR-Set behavior used to pin a trace    *)
(* fixture for Rust trace-replay testing.                                   *)
(*                                                                          *)
(* It EXTENDS the verified ORSet spec and drives its actual Add/Remove/Merge *)
(* actions from a fixed Script via a program counter. The invariant          *)
(* TraceMatches asserts that the final per-replica MEMBERSHIP equals the     *)
(* authored Expected; a clean TLC run pins it. The Rust test replays the     *)
(* same ops and must reproduce the same membership.                          *)
(*                                                                          *)
(* We compare MEMBERSHIP (which elements are live), not raw tags: the spec    *)
(* uses <<replica, counter>> tags and the Rust impl uses Uuids, so tag        *)
(* encoding differs, but the add-wins membership must match exactly.         *)
(***************************************************************************)
EXTENDS ORSet, Sequences

VARIABLE pc

varsT == <<added, removed, clock, pc>>

\* A fixed op sequence over replicas {1,2} and elements {1,2}, exercising both
\* add-wins (a concurrent re-add survives a remove) and remove-wins.
Ops ==
    << [op |-> "add",    r |-> 1, x |-> 1],   \* tag <<1,0>>
       [op |-> "merge",  r |-> 2, s |-> 1],   \* r2 observes <<1,0>>
       [op |-> "add",    r |-> 1, x |-> 1],   \* tag <<1,1>> (concurrent re-add)
       [op |-> "remove", r |-> 2, x |-> 1],   \* tombstones <<1,0>> only
       [op |-> "merge",  r |-> 1, s |-> 2],
       [op |-> "merge",  r |-> 2, s |-> 1],   \* x=1 survives via <<1,1>>: ADD WINS
       [op |-> "add",    r |-> 2, x |-> 2],   \* tag <<2,0>>
       [op |-> "merge",  r |-> 1, s |-> 2],
       [op |-> "remove", r |-> 1, x |-> 2],   \* tombstones <<2,0>> (observed)
       [op |-> "merge",  r |-> 2, s |-> 1] >> \* x=2 gone: REMOVE WINS

DoStep(a) ==
    CASE a.op = "add"    -> Add(a.r, a.x)
      [] a.op = "remove" -> Remove(a.r, a.x)
      [] a.op = "merge"  -> Merge(a.r, a.s)

TInit == Init /\ pc = 1
TStep == /\ pc <= Len(Ops)
         /\ DoStep(Ops[pc])
         /\ pc' = pc + 1
TDone == pc > Len(Ops)
TNext == TStep \/ (TDone /\ UNCHANGED varsT)
TSpec == TInit /\ [][TNext]_varsT

\* The live elements at replica r.
Membership(r) == { x \in Elements : Contains(r, x) }

\* Authored; PINNED by TLC via TraceMatches. Both replicas end with {1} present.
Expected == [r \in Replicas |-> {1}]

TraceMatches == TDone => (\A r \in Replicas : Membership(r) = Expected[r])
=========================================================================
