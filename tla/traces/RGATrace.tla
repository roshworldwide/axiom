---------------------------- MODULE RGATrace ----------------------------
(***************************************************************************)
(* Axiom — a DETERMINISTIC, scripted RGA behavior used to pin a trace        *)
(* fixture for Rust trace-replay testing.                                   *)
(*                                                                          *)
(* It EXTENDS the verified RGA spec and drives its Insert/Delete/Merge        *)
(* actions from a fixed Script via a program counter. The invariant          *)
(* TraceMatches pins the final visible id sequence AND the tombstone set.    *)
(*                                                                          *)
(* The Rust test replays the SAME ops, feeding the trace's <<counter,        *)
(* replica>> ids into the implementation (insert_after_with_id) so the id    *)
(* tie-break matches the spec's, and compares the visible id sequence and    *)
(* tombstones. The scenario exercises the tie-break: two replicas insert      *)
(* after the same element, ordered descending by id.                         *)
(***************************************************************************)
EXTENDS RGA

VARIABLE pc

varsT == <<elem, tomb, ins, pc>>

\* Ids are <<counter, replica>>; Insert(r, after) mints id <<ins[r], r>>.
Ops ==
    << [op |-> "insert", r |-> 1, after |-> Origin],     \* id <<0,1>>
       [op |-> "merge",  r |-> 2, s |-> 1],              \* r2 observes <<0,1>>
       [op |-> "insert", r |-> 1, after |-> <<0, 1>>],   \* id <<1,1>>, after <<0,1>>
       [op |-> "insert", r |-> 2, after |-> <<0, 1>>],   \* id <<0,2>>, after <<0,1>>
       [op |-> "merge",  r |-> 1, s |-> 2],
       [op |-> "merge",  r |-> 2, s |-> 1],              \* siblings of <<0,1>>:
                                                         \* <<1,1>> before <<0,2>> (desc)
       [op |-> "delete", r |-> 1, target |-> <<1, 1>>],  \* tombstone <<1,1>>
       [op |-> "merge",  r |-> 2, s |-> 1] >>

DoStep(a) ==
    CASE a.op = "insert" -> Insert(a.r, a.after)
      [] a.op = "delete" -> Delete(a.r, a.target)
      [] a.op = "merge"  -> Merge(a.r, a.s)

TInit == Init /\ pc = 1
TStep == /\ pc <= Len(Ops)
         /\ DoStep(Ops[pc])
         /\ pc' = pc + 1
TDone == pc > Len(Ops)
TNext == TStep \/ (TDone /\ UNCHANGED varsT)
TSpec == TInit /\ [][TNext]_varsT

\* Authored; PINNED by TLC. After tombstoning <<1,1>>, the visible sequence is
\* <<0,1>> then <<0,2>>, at both replicas.
ExpectedVisible == << <<0, 1>>, <<0, 2>> >>
ExpectedTomb == { <<1, 1>> }

TraceMatches ==
    TDone =>
        /\ \A r \in Replicas : Visible(r) = ExpectedVisible
        /\ \A r \in Replicas : tomb[r] = ExpectedTomb
=========================================================================
