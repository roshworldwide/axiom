# OR-Set

A G-Counter only grows. Most data is messier: you add things, then remove them, and two replicas may add and remove the *same* thing at the same time. The Observed-Remove Set (OR-Set) is the CRDT for that. Axiom's lives in `crates/axiom-core/src/orset.rs`, refining `tla/ORSet.tla`.

## The problem with the naive set

Imagine a set where `add(x)` inserts `x`, `remove(x)` drops it, and merge is set union. Now run this:

- Replica A and B both know `{x}`.
- B removes `x`, so B holds `{}`.
- A and B merge: `{x} ∪ {} = {x}`.

The element comes back from the dead. Union has no way to know B's empty set means "I deleted x" versus "I never saw x." Removal information is lost the instant it touches a union. This is the **resurrection** bug, and it is why a one-set design cannot work.

## Tags and tombstones

The OR-Set fixes this by making every add unique. Each `add(x)` stamps the element with a fresh tag, and removal targets *tags*, not values.

In Rust, tags are `Uuid` v4:

```rust
pub struct ORSet<T> {
    added:   BTreeSet<(T, Uuid)>, // observed (element, tag) adds
    removed: BTreeSet<Uuid>,      // tombstoned tags
}
```

An element is present iff it has some added tag that is not tombstoned:

```rust
pub fn contains(&self, x: &T) -> bool {
    self.added.iter().any(|(e, tag)| e == x && !self.removed.contains(tag))
}
```

Both sets only grow, and `merge` is just their union — so the state still lives in a join-semilattice and converges, exactly like the counters.

## Add-wins semantics

The rule that makes this work is in `remove`: a replica may only tombstone tags **it has actually observed**.

```rust
pub fn remove(&mut self, x: &T) -> ORSetOp<T> {
    let tags: BTreeSet<Uuid> = self.added.iter()
        .filter(|(e, _)| e == x)
        .map(|(_, tag)| *tag)
        .collect();
    self.removed.extend(tags.iter().copied());
    ORSetOp::Remove { tags }
}
```

Now replay the resurrection scenario, but with a concurrent add:

1. A and B both observe `x` under tag `t1`.
2. A adds `x` again, getting a fresh tag `t2` that B has never seen.
3. B removes `x`, tombstoning only `t1`.
4. They merge: `added = {(x,t1),(x,t2)}`, `removed = {t1}`.

`t2` survives untombstoned, so `x` is still present. The concurrent add wins. But if B's remove had *causally* observed the add (no concurrent re-add), it holds that exact tag, tombstones it, and the remove wins. This is **add-wins**: a concurrent add beats a concurrent remove, and a remove only beats the adds it saw.

## The Rust API

| Method | Effect |
|---|---|
| `add(x) -> ORSetOp` | Insert `x` under a fresh `Uuid`; returns the op to broadcast. |
| `remove(&x) -> ORSetOp` | Tombstone the observed tags of `x`. |
| `contains(&x) -> bool` | Is `x` present? |
| `iter()` | Iterate distinct present elements. |
| `len()` / `is_empty()` | Count / emptiness of present elements. |
| `merge(&other)` | Union both `added` and `removed`. |
| `apply(&op)` | Apply a remote `Add`/`Remove` op. |

`add` and `remove` return an `ORSetOp` for op-based causal broadcast, but the state-based `merge` is the convergence workhorse.

## What the spec guarantees

`tla/ORSet.tla` models the same design (`added`, `removed`, and a per-replica `clock` issuing `<<replica, k>>` tags) and pins two properties, **model-checked with TLC (bounded) at 7,239 distinct states**:

- **`TombstonesObserved`** — `removed[r] ⊆ ObservedTags(r)`. A tag is tombstoned only if the replica observed it. This is the add-wins mechanism stated as an invariant: a buggy remove-by-value would violate it.
- **`Monotonic`** — a box-action property: neither `added[r]` nor `removed[r]` ever shrinks. Each replica climbs the join-semilattice, so exchanging updates converges to the least upper bound.

The Rust `tla_state()` method exposes `{added, removed}` as the refinement mapping, and proptest checks the same shapes randomly — convergence, idempotence, add-wins, and `TombstonesObserved` — among Axiom's 31 property tests. The spec's `<<replica, counter>>` tags and the implementation's `Uuid`s differ only in representation; both are unique opaque tokens, and the spec's reasoning is agnostic to which you use.
