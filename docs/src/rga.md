# RGA: a Sequence CRDT

A Replicated Growable Array (RGA) is a list that several replicas can edit concurrently — insert a character here, delete one there — and that always converges to the same sequence on every replica, with no coordination. It's the CRDT behind collaborative text editors.

Axiom's RGA lives in `crates/axiom-core/src/rga.rs` and refines `tla/RGA.tla`. This page is the API reference, with pointers to the invariants the spec checks.

## The model in one paragraph

Every inserted element gets a globally-unique, totally-ordered id: an `ElementId` pairing an HLC timestamp with the originating `ReplicaId`. An element also records its *predecessor* — the element it was inserted after, or `None` for the list head (the spec's `Origin` sentinel). Deleting an element *tombstones* its id: the element keeps its position so later elements stay correctly placed, it just stops being visible. Merge is a set union of elements and tombstones. The visible sequence is a pre-order traversal from the origin, with siblings ordered *descending* by id (newest-after-a-reference comes first), tombstoned ids filtered out.

## Rust API

```rust
use axiom_core::rga::{Rga, ElementId};
use axiom_core::ReplicaId;

let mut doc: Rga<char> = Rga::new(ReplicaId(1));

// Insert by visible position (clamped to the end).
doc.insert(0, 'a');                 // -> ElementId
doc.insert(1, 'b');

// Insert relative to an element's id (None = front).
let a = doc.insert(0, 'x');
let _ = doc.insert_after(Some(a), 'y');

// Delete by id (tombstones it; position is kept).
doc.delete(a);

// Read out the visible sequence.
let chars: Vec<&char> = doc.to_vec();
let n = doc.len();                  // visible count; is_empty() too
```

- `new(replica)` — a fresh empty array owned by `replica`.
- `insert(index, content) -> ElementId` — insert at a visible position, clamped to the end.
- `insert_after(after, content) -> ElementId` — insert immediately after element `after` (or at the front when `None`). Ids come from the replica's HLC clock, so they are unique and roughly time-ordered.
- `delete(id)` — tombstone `id` if present.
- `to_vec() -> Vec<&T>` / `ids()` / `len()` / `is_empty()` — read the visible sequence.
- `merge(&other)` — union element and tombstone sets, then advance this replica's clock past the merged ids (so future local ids stay fresh). Order-independent and idempotent.

One RGA quirk worth seeing: insert `'a'`, then `'b'` after `'a'`, then `'c'` after `'a'` again — you get `"acb"`, not `"abc"`. The newest insert after a reference lands first. That descending-by-id tie-break is exactly what makes concurrent inserts at the same spot converge.

## What the spec guarantees (and how it's checked)

The refinement mapping `tla_state()` projects each Rust array to the abstract `(id, predecessor)` element set and tombstone set of `tla/RGA.tla`. If the mapping is faithful and the spec holds, the implementation inherits these invariants:

- **`Convergent`** — the KEY safety property. For any two replicas, every id they *both* show appears in the same relative order. Replicas that saw the same operations produce the same sequence.
- **`Wellformed`** — every predecessor is the origin or another present element, so the predecessor graph is a tree and the traversal is well-defined.
- **`LinearizationOK`** — the visible sequence lists each visible element exactly once (no drops, no duplicates).

These are **model-checked (TLC, bounded)** at 2 replicas, exploring **35,441 distinct states**. Symmetry reduction is deliberately *not* used here: RGA's tie-break is a total order on ids, which depends on replica identifiers and makes replicas distinguishable, so symmetry would be unsound. The model is bounded tightly instead. The Rust side mirrors the same three invariants under **property tests (proptest)**: order-independent convergence, idempotent merge, predecessors-present, and visible-sequence-is-a-permutation.

## The iterative traversal

A text buffer built by appending is a *linear* predecessor chain whose depth equals its length. A recursive pre-order walk would recurse once per element and overflow the call stack on a long document. Adversarial review caught this, so `order()` uses an explicit heap-allocated stack (siblings sorted ascending, popped LIFO to visit largest-id first). A regression test builds a 100,000-element chain that the old recursive walk could not survive on a default test-thread stack.

The same function also re-roots any element whose predecessor is absent — a state `Wellformed` forbids, but which a hostile or corrupt deserialized input could contain — at the origin, so a bad link is never silently dropped and `len()` stays consistent with `ids()`.
