------------------------------ MODULE RGA ------------------------------
(***************************************************************************)
(* Axiom — Phase 1, Week 4.  A Replicated Growable Array (RGA) — a sequence  *)
(* CRDT — replicated over `Replicas`.  This is the hardest spec.            *)
(*                                                                          *)
(* Each inserted element has a unique id <<counter, replica>> and a          *)
(* predecessor (the Origin sentinel, or the id of the element it was inserted  *)
(* after).  Deletes tombstone an id (the element stays, holding its tree     *)
(* position).  Merge is the union of the element and tombstone sets.         *)
(*                                                                          *)
(* The visible sequence `Visible(r)` is the RGA pre-order traversal from     *)
(* Origin — children of a node ordered DESCENDING by id (newest-after-the-     *)
(* reference first) — with tombstoned ids filtered out.                      *)
(*                                                                          *)
(* ABSTRACTION: element *content* (the payload char) is elided.  It does not *)
(* affect ordering or convergence, and the Rust impl carries it; the spec    *)
(* reasons about the sequence of ids, which determines the sequence of       *)
(* contents.                                                                 *)
(*                                                                          *)
(* SYMMETRY: NOT used.  RGA's tie-break is a total order on ids (hence on    *)
(* replica identifiers), which makes replicas distinguishable, so symmetry   *)
(* reduction would be unsound.  We bound tightly instead.                    *)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Replicas,     \* a set of NATURALS (ids need a total order for the tie-break)
    MaxInserts,   \* per-replica insert bound (keeps the model finite)
    Origin          \* the list-head sentinel (a value distinct from any id)

ASSUME MaxInserts \in Nat \ {0}
ASSUME Replicas \subseteq Nat

\* Ids are handed out deterministically and uniquely as <<counter, replica>>.
Id == (0 .. MaxInserts - 1) \X Replicas

\* Strict total order on ids: lexicographic on <<counter, replica>>.
IdLess(a, b) == (a[1] < b[1]) \/ (a[1] = b[1] /\ a[2] < b[2])

VARIABLES
    elem,   \* elem[r] : set of records [id |-> Id, pred |-> {Origin} \cup Id]
    tomb,   \* tomb[r] : set of tombstoned ids
    ins     \* ins[r]  : r's next insert counter (0 .. MaxInserts)

vars == <<elem, tomb, ins>>

Ids(r) == { e.id : e \in elem[r] }
VisibleIds(r) == Ids(r) \ tomb[r]

\* Sort a set S of ids into a sequence, DESCENDING by IdLess (RGA: newest first).
RECURSIVE SortDesc(_)
SortDesc(S) ==
    IF S = {} THEN << >>
    ELSE LET mx == CHOOSE x \in S : \A y \in S \ {x} : IdLess(y, x)
         IN  <<mx>> \o SortDesc(S \ {mx})

\* RGA pre-order traversal of element set E (records [id, pred]) under parent p.
RECURSIVE Walk(_, _)
RECURSIVE WalkSeq(_, _, _)
Walk(E, p) ==
    WalkSeq(E, SortDesc({ e.id : e \in { el \in E : el.pred = p } }), 1)
WalkSeq(E, kids, i) ==
    IF i > Len(kids) THEN << >>
    ELSE <<kids[i]>> \o Walk(E, kids[i]) \o WalkSeq(E, kids, i + 1)

\* The full id order (including tombstoned positions) and the visible sequence.
Order(r)   == Walk(elem[r], Origin)
Visible(r) == SelectSeq(Order(r), LAMBDA id : id \notin tomb[r])

\* ---- Spec -------------------------------------------------------------

TypeOK ==
    /\ elem \in [Replicas -> SUBSET [id : Id, pred : {Origin} \cup Id]]
    /\ tomb \in [Replicas -> SUBSET Id]
    /\ ins  \in [Replicas -> 0 .. MaxInserts]

Init ==
    /\ elem = [r \in Replicas |-> {}]
    /\ tomb = [r \in Replicas |-> {}]
    /\ ins  = [r \in Replicas |-> 0]

\* Insert a new element at r after an observed predecessor (or Origin).
Insert(r, after) ==
    /\ ins[r] < MaxInserts
    /\ after \in {Origin} \cup Ids(r)
    /\ elem' = [elem EXCEPT ![r] = @ \cup {[id |-> <<ins[r], r>>, pred |-> after]}]
    /\ ins'  = [ins EXCEPT ![r] = @ + 1]
    /\ UNCHANGED tomb

\* Tombstone an observed, not-yet-deleted element.
Delete(r, target) ==
    /\ target \in Ids(r)
    /\ target \notin tomb[r]
    /\ tomb' = [tomb EXCEPT ![r] = @ \cup {target}]
    /\ UNCHANGED <<elem, ins>>

\* Anti-entropy: r absorbs s's elements and tombstones.
Merge(r, s) ==
    /\ elem' = [elem EXCEPT ![r] = @ \cup elem[s]]
    /\ tomb' = [tomb EXCEPT ![r] = @ \cup tomb[s]]
    /\ UNCHANGED ins

Next ==
    \/ \E r \in Replicas, a \in {Origin} \cup Id : Insert(r, a)
    \/ \E r \in Replicas, t \in Id : Delete(r, t)
    \/ \E r, s \in Replicas : Merge(r, s)

Spec == Init /\ [][Next]_vars

\* ---- Properties checked by TLC (bounded) ------------------------------

\* Every element's predecessor is Origin or another present element, so the
\* predecessor graph is a tree rooted at Origin and Walk is well-defined.
Wellformed ==
    \A r \in Replicas : \A e \in elem[r] :
        e.pred = Origin \/ (\E f \in elem[r] : f.id = e.pred)

NoDup(s) == \A i, j \in 1 .. Len(s) : (i # j) => (s[i] # s[j])

\* The visible sequence is a valid linearization: each visible element appears
\* exactly once.  A traversal that dropped or duplicated an element fails this.
LinearizationOK ==
    \A r \in Replicas :
        /\ Len(Visible(r)) = Cardinality(VisibleIds(r))
        /\ NoDup(Visible(r))

Precedes(s, a, b) == \E i, j \in 1 .. Len(s) : i < j /\ s[i] = a /\ s[j] = b

(***************************************************************************)
(* CONVERGENCE (the KEY safety property).  Any two replicas agree on the     *)
(* relative order of every element id they BOTH show.  An element's order is  *)
(* fixed by its ancestor chain (all of which are present wherever it is, by   *)
(* Wellformed) and the global id tie-break, so the order can never invert.    *)
(* Hence replicas that received the same operations produce the same          *)
(* sequence: seq(s1) = seq(s2).                                              *)
(***************************************************************************)
Convergent ==
    \A r, s \in Replicas :
        \A a, b \in (VisibleIds(r) \cap VisibleIds(s)) :
            a # b => (Precedes(Visible(r), a, b) <=> Precedes(Visible(s), a, b))

\* No divergence: state only grows in the (elem, tomb) join-semilattice.
Monotonic ==
    [][ \A r \in Replicas :
          /\ elem[r] \subseteq elem'[r]
          /\ tomb[r] \subseteq tomb'[r] ]_vars
=============================================================================
