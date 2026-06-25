# CRDTs from Scratch

Imagine you're building a collaborative app. Two users, on two devices, both edit the same document at the same time. The network is slow, or down. What should happen?

The easy answer is "lock the document" or "ask a server who wins." But locks and servers mean coordination, and coordination means waiting. If a user has to wait for a round-trip before their keystroke registers, the app feels broken. Worse, if the network partitions, nobody can write at all.

CRDTs are a way out of this bind.

## The convergence problem

Let's state the problem precisely. We have several **replicas** of the same data — copies living on different devices. We want three things at once:

1. **Local writes.** Any replica can accept a write immediately, without asking anyone.
2. **No coordination.** Replicas exchange updates whenever they happen to be connected, in any order, possibly more than once.
3. **Convergence.** Once replicas have seen the same set of updates, they must hold *identical* state.

The tension is obvious. If everyone writes locally and updates arrive in different orders on different devices, how can the replicas ever agree? Naively, they can't — replica A applies "set x = 1, then set x = 2" while replica B sees them reversed and ends at 1. They've diverged.

The trick is to design the data type so that order and duplication *don't matter*. If merging updates always lands at the same answer regardless of order, divergence becomes impossible.

## Strong eventual consistency

This goal has a name: **strong eventual consistency** (SEC). It promises that any two replicas which have received the same set of updates are in the same state — not "eventually after some reconciliation protocol," but immediately and by construction.

SEC is stronger than plain "eventual consistency," which only promises replicas *converge somehow* (often via a last-writer-wins clock or a conflict-resolution callback you have to write yourself). SEC bakes conflict resolution into the data type's math, so there are no conflicts to resolve. A **Conflict-free Replicated Data Type** is a data type engineered to satisfy SEC.

There are two classic ways to build one.

## State-based CRDTs (convergent)

In a **state-based** CRDT, replicas exchange their *whole state*. When a replica receives another's state, it `merge`s it into its own.

For this to give convergence, the states must form a **join-semilattice**, and `merge` must be the lattice's *join* (least upper bound). Concretely, `merge` must be:

- **Commutative**: `merge(a, b) = merge(b, a)` — order of arrival doesn't matter.
- **Associative**: `merge(merge(a, b), c) = merge(a, merge(b, c))` — grouping doesn't matter.
- **Idempotent**: `merge(a, a) = a` — receiving the same state twice is harmless.

These three laws are exactly what defeats the convergence problem. Reordering updates? Commutativity and associativity absorb it. Duplicate deliveries? Idempotence absorbs it. Local writes only ever move state "upward" in the lattice, and `merge` always climbs to the least state above both inputs. Two replicas that have seen the same updates compute the same least upper bound — so they match.

In Axiom, this is more than a slogan. The merge laws are written as TLA+ specifications and checked. G-Counter merge commutativity in particular is **machine-proved (TLAPS)** — a deductive, unbounded proof (11 obligations) in `tla/GCounterProofs.tla` — so it holds for *all* states, not just the ones a model checker reaches.

## Operation-based CRDTs (commutative)

In an **operation-based** (op-based) CRDT, replicas don't ship whole states; they broadcast individual *operations* ("increment by 3," "insert 'h' after position p"). Each replica applies operations as they arrive.

Op-based CRDTs converge under two conditions:

- **Concurrent operations commute.** If two operations could have happened concurrently, applying them in either order gives the same result.
- **Causal delivery.** Operations are delivered respecting cause-and-effect: you never see "delete the character I inserted" before the insert. (This is provided by the messaging layer, not the data type.)

Op-based designs send less data, but lean on the delivery layer to be exactly-once and causally ordered. State-based designs are more robust to a lossy, duplicating, reordering network — at the cost of sending more. The two styles are formally equivalent in power; you can emulate one with the other. Axiom's specs lean state-based, where the lattice laws are easiest to state and check.

## The four Axiom types, conceptually

Axiom ships four CRDTs. Here's the intuition for each; later chapters give the specs and the math.

### G-Counter — a grow-only counter

A counter that only goes up. The clever part: instead of one shared number, each replica keeps *its own* count, and the value is the sum across all replicas. Replica A only ever touches A's entry, B only touches B's. To `merge`, take the per-replica **maximum** of each entry. Max is commutative, associative, and idempotent — so the lattice laws come for free. See `tla/GCounter.tla`.

### PN-Counter — increments and decrements

A grow-only counter can't go down, because "subtract" doesn't have a nice maximum. The fix is delightfully simple: keep *two* G-Counters, one counting increments (P) and one counting decrements (N). The value is `P − N`. Each half merges by max as before, and the difference can move freely up or down. Two grow-only lattices glued together. See `tla/PNCounter.tla`.

### OR-Set — an add-wins set

Sets are trickier: what happens if one replica adds an element while another concurrently removes it? An **Observed-Remove Set** resolves this with **add-wins** semantics. Every `add` is tagged with a unique token. A `remove` only deletes the specific tokens it has *observed*. So a concurrent add — carrying a fresh, unseen token — survives the remove, and the element stays. An element is "in the set" when it has at least one token nobody has removed. See `tla/ORSet.tla`.

### RGA — a replicated sequence

Ordered text and lists are the hardest case. A **Replicated Growable Array** models a sequence as a set of elements, each with a stable, globally-unique identifier and a "comes after" link to the element it was inserted next to. Insertions reference an identifier, never a numeric index — so concurrent inserts at "the same spot" don't clobber each other; a deterministic tiebreak on identifiers fixes their order consistently on every replica. Deletes leave **tombstones** (the slot is marked removed but its identifier lingers) so later operations referencing it still make sense. See `tla/RGA.tla`.

## Where this leaves us

A CRDT lets every replica write locally, sync in any order over any flaky network, and still converge — because the data type's algebra makes order and duplication irrelevant. State-based types lean on the join-semilattice laws; op-based types lean on commuting operations plus causal delivery.

The rest of the book makes these claims concrete: the TLA+ specs that pin down each type's behavior, the model-checking and proofs that back them, and the Rust implementations connected to those specs by explicit refinement mappings. Next, we'll look at how Axiom states an invariant and checks it.
