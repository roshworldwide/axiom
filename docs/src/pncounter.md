# PN-Counter

A G-Counter only goes up. Plenty of real counters need to go down too: active connections, items in a cart, a like that gets un-liked. The **PN-Counter** ("positive-negative") supports both `increment` and `decrement`, and its trick is delightfully simple — it is *two* G-Counters.

One G-Counter, `P`, accumulates increments. The other, `N`, accumulates decrements. The value is just their difference:

```
value = P.value() - N.value()
```

Because both halves are grow-only, every CRDT property you get for the G-Counter (commutative, associative, idempotent merge) comes along for free. But the *value* can now go down — which is exactly the point.

## Rust API

`crates/axiom-core/src/pncounter.rs` wraps two `GCounter`s and a replica id.

```rust
use axiom_core::{PNCounter, ReplicaId};

let mut likes = PNCounter::new(ReplicaId(0));

likes.increment();          // -> PNCounterOp::Inc(..)
likes.increment();
let op = likes.decrement(); // -> PNCounterOp::Dec(..)

assert_eq!(likes.value(), 1);   // 2 up, 1 down
```

The methods:

| Method | Returns | Effect |
|---|---|---|
| `new(id)` | `PNCounter` | fresh counter owned by `id`, value `0` |
| `increment()` | `PNCounterOp` | bumps the `P` side; returns the op to broadcast |
| `decrement()` | `PNCounterOp` | bumps the `N` side; returns the op to broadcast |
| `value()` | `i64` | net `P - N` (**may be negative**) |
| `apply(&op)` | `()` | applies a remote op to the correct side |
| `merge(&other)` | `()` | component-wise max on *both* `P` and `N` |

Note `value()` returns `i64`, not `u64`: a counter that has seen more decrements than increments is genuinely negative. The `value_can_decrease` test pins this down — increment once, decrement twice, and the value is `-1`.

`increment` and `decrement` hand back a `PNCounterOp` so an op-based transport can ship the single delta instead of the whole state; `merge` is the state-based path. Both routes converge to the same place.

## The spec it refines

`tla/PNCounter.tla` models `P` and `N` as a pair of knowledge matrices (`[Replicas -> [Replicas -> Nat]]`), exactly mirroring the Rust `TlaPNCounterState`. The value is defined as

```tla
Value(r) == SumOver(P[r], Replicas) - SumOver(N[r], Replicas)
```

with a comment that says it plainly: it "may be negative; that is allowed."

The safety property is **`NoFabrication`** — a merge never invents an operation:

```tla
NoFabrication ==
    \A r, s \in Replicas :
        /\ P[r][s] <= P[s][s]
        /\ N[r][s] <= N[s][s]
```

In words: no replica's belief about how many ops `s` performed can exceed what `s` itself recorded on its own diagonal entry. `Increment(r)` and `Decrement(r)` only ever touch `r`'s own slot, and `Merge` only takes maxima, so knowledge can spread but never exceed its source. This was **model-checked with TLC (bounded)**, exploring **2,020 distinct states** — no fabricated op was reachable within the bounds.

## Monotonic where it counts, not where it doesn't

The subtle part worth internalizing: the *value* is intentionally **non-monotonic**, while the machinery underneath is strictly monotonic.

`P` and `N` only ever grow — that is what keeps merges convergent and order-independent. The proptest `merge_grows_both_sides` checks exactly this: after a merge, both sides are `>=` either input. But their *difference* is free to wobble up and down as decrements pile into `N`.

So the PN-Counter never "forgets." A decrement is not an undo of an increment; it is a new, additive fact recorded in `N`. That is why it stays a well-behaved CRDT despite supporting an operation that, naively, looks like it should break grow-only convergence.

The property tests in `pncounter.rs` (`merge_is_commutative`, `merge_is_idempotent`, `merge_grows_both_sides`, plus a MessagePack round-trip) are part of Axiom's **proptest** suite. For the deductive, unbounded result on the underlying merge, see the G-Counter chapter and `tla/GCounterProofs.tla`.
