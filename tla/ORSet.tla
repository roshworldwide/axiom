----------------------------- MODULE ORSet -----------------------------
(***************************************************************************)
(* Axiom — Phase 1, Week 3.  An Observed-Remove Set (OR-Set) CRDT,          *)
(* replicated over `Replicas`.                                              *)
(*                                                                          *)
(* Each add of an element is stamped with a UNIQUE tag.  A replica's state   *)
(* is the set of (element, tag) pairs it has observed-added, plus a set of   *)
(* tombstoned tags it has observed-removed.  An element is present iff it     *)
(* has some observed-added tag that has NOT been tombstoned.  Merge is the    *)
(* component-wise union of both sets.                                         *)
(*                                                                          *)
(* DESIGN NOTE: the textbook one-set version (merge = union of (elem,tag)    *)
(* pairs, remove = drop pairs, no tombstones) lets a removed pair RESURRECT  *)
(* on the next union merge, so it cannot satisfy "remove wins when it        *)
(* observed the add".  We therefore tombstone observed TAGS — the canonical  *)
(* OR-Set — which makes both halves of the add-wins property hold.  Phase 2  *)
(* (Rust) mirrors this exact design.                                         *)
(***************************************************************************)
EXTENDS Naturals, TLC

CONSTANTS
    Replicas,    \* the set of replica identifiers
    Elements,    \* the set of values that can be added/removed
    MaxAdds      \* per-replica add bound (keeps the tag space, and TLC, finite)

ASSUME MaxAdds \in Nat \ {0}

\* Tags are handed out deterministically and uniquely as <<replica, k>>, where
\* k is the issuing replica's local add-counter.  This keeps the tag space
\* finite WITHOUT a CHOOSE over a symmetry set.
Tags == Replicas \X (0 .. MaxAdds - 1)

VARIABLES
    added,     \* added[r]   : set of <<element, tag>> observed-added at r
    removed,   \* removed[r] : set of tombstoned tags at r
    clock      \* clock[r]   : r's next local add-counter (0 .. MaxAdds)

vars == <<added, removed, clock>>

\* Tags that replica r has observed (appear in some added pair at r).
ObservedTags(r) == { t \in Tags : \E x \in Elements : <<x, t>> \in added[r] }

\* OR-Set membership: an element is present at r iff it has an observed-added
\* tag that r has not tombstoned.
Contains(r, x) == \E t \in Tags : <<x, t>> \in added[r] /\ t \notin removed[r]

\* ---- Spec -------------------------------------------------------------

TypeOK ==
    /\ added   \in [Replicas -> SUBSET (Elements \X Tags)]
    /\ removed \in [Replicas -> SUBSET Tags]
    /\ clock   \in [Replicas -> 0 .. MaxAdds]

Init ==
    /\ added   = [r \in Replicas |-> {}]
    /\ removed = [r \in Replicas |-> {}]
    /\ clock   = [r \in Replicas |-> 0]

\* Add x at r with a fresh tag <<r, clock[r]>>.
Add(r, x) ==
    /\ clock[r] < MaxAdds
    /\ added' = [added EXCEPT ![r] = @ \cup {<<x, <<r, clock[r]>>>>}]
    /\ clock' = [clock EXCEPT ![r] = @ + 1]
    /\ UNCHANGED removed

\* Remove x at r: tombstone exactly the tags of x that r has OBSERVED.
Remove(r, x) ==
    /\ \E t \in Tags : <<x, t>> \in added[r]
    /\ removed' = [removed EXCEPT ![r] = @ \cup { t \in Tags : <<x, t>> \in added[r] }]
    /\ UNCHANGED <<added, clock>>

\* Anti-entropy: r absorbs s's observations of both adds and removes.
Merge(r, s) ==
    /\ added'   = [added   EXCEPT ![r] = @ \cup added[s]]
    /\ removed' = [removed EXCEPT ![r] = @ \cup removed[s]]
    /\ UNCHANGED clock

Next ==
    \/ \E r \in Replicas, x \in Elements : Add(r, x)
    \/ \E r \in Replicas, x \in Elements : Remove(r, x)
    \/ \E r, s \in Replicas : Merge(r, s)

Spec == Init /\ [][Next]_vars

Symm == Permutations(Replicas)

\* ---- Properties checked by TLC (bounded) ------------------------------

(***************************************************************************)
(* Add-wins MECHANISM (the KEY safety property).  A tag is tombstoned at r   *)
(* only if r has OBSERVED it.  Hence:                                        *)
(*  - a Remove concurrent with an Add has not observed the Add's fresh tag,   *)
(*    so it cannot tombstone it; the tag survives the union merge and the     *)
(*    element stays present  ==>  the ADD wins;                              *)
(*  - a Remove that causally observed the Add holds that exact tag and        *)
(*    tombstones it  ==>  the REMOVE wins.                                    *)
(* A buggy model (e.g. remove-by-value, or tombstoning unseen tags) would     *)
(* violate this invariant.                                                    *)
(***************************************************************************)
TombstonesObserved == \A r \in Replicas : removed[r] \subseteq ObservedTags(r)

\* No divergence: each replica's state only moves UP the (added, removed)
\* join-semilattice (neither set ever shrinks), and Merge computes joins, so
\* replicas that exchange updates converge to the same least upper bound.
\* This is a SAFETY (box-action) property, so symmetry reduction stays sound.
Monotonic ==
    [][ \A r \in Replicas :
          /\ added[r]   \subseteq added'[r]
          /\ removed[r] \subseteq removed'[r] ]_vars
=============================================================================
