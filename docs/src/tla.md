# TLA+ from scratch

You don't need to know TLA+ to use Axiom. But to *read* its specs — and to understand exactly what each verified claim does and doesn't promise — you need a working vocabulary. This chapter gives you just enough, built around one tiny spec you can read in full.

## A spec is a state machine

TLA+ describes a system as a state machine: some **state variables**, a rule for the **initial** state, and a rule for **how state may change** in one step. That's it. Everything else is notation.

Here is `tla/Counter.tla`, a non-replicated counter, in its entirety:

```tla
EXTENDS Naturals

VARIABLE counter

TypeOK == counter \in Nat

Init == counter = 0

Increment == counter' = counter + 1
Read      == UNCHANGED counter

Next == Increment \/ Read

Spec == Init /\ [][Next]_counter

NonNegative == counter >= 0

StateConstraint == counter <= 5
```

Read it top to bottom:

- **`VARIABLE counter`** — the entire state of the system is one number.
- **`Init == counter = 0`** — the system starts at zero. `Init` is a predicate on the *current* state.
- **Actions** describe one step. `counter'` (with a prime) means "the value of `counter` in the *next* state." So `Increment == counter' = counter + 1` says: next state's counter is this state's plus one. `Read == UNCHANGED counter` leaves it alone.
- **`Next == Increment \/ Read`** — a step is *either* an increment or a read. `\/` is "or", `/\` is "and".
- **`Spec == Init /\ [][Next]_counter`** — the full behavior: start in `Init`, and every step satisfies `Next` (the `[]` is "always"; the `_counter` subscript permits stuttering steps that change nothing). You can mostly skim past this line; it's boilerplate that stitches `Init` and `Next` together.

## Invariants

An **invariant** is a predicate that must hold in *every* reachable state. `Counter.tla` has two:

```tla
TypeOK      == counter \in Nat      \* it's always a natural number
NonNegative == counter >= 0         \* it never goes negative
```

`TypeOK` is a *type invariant* — a convention in every Axiom spec, asserting the shape of the state. `NonNegative` is a real safety property. Invariants are the claims we check. The interesting question is *how* we check them, and that splits into two very different tools.

## TLC vs. TLAPS — the distinction that matters

These are not two flavors of the same thing. They give different *strengths* of guarantee, and Axiom's claims policy depends on never conflating them.

**TLC** is a model checker. You give it a *finite* model, and it mechanically enumerates every reachable state, testing each invariant. If one can be violated within those bounds, TLC prints the shortest counterexample trace. It is **exhaustive up to finite bounds**: automatic, great at finding design bugs, and a producer of concrete counterexamples — but **bounded**. "TLC found no violation" means *no violation exists in the finite model you specified*, not a proof for all inputs. We call this **model-checked (TLC, bounded)** and always state the bounds. A run reports a count of distinct states explored — for `Counter` that's just **6** states; the larger Axiom specs reach into the thousands (`ORSet` 7,239, `RGA` 35,441), roughly 62,000 across all specs.

**TLAPS** is a proof system. You *write* a deductive proof — a hierarchy of steps and justifications — and TLAPS, with backend provers (Z3 here), verifies each step follows. A successful check is **unbounded**: it holds for all replicas, all values, forever. We call this **machine-proved (TLAPS)**. It is real work, so Axiom reserves it for the highest-value lemmas. Only **two** results carry this label: G-Counter merge commutativity (`tla/GCounterProofs.tla`, 11 obligations) and acoustic-auth freshness arithmetic (`tla/AcousticAuthProofs.tla`, 3 obligations).

One line: **TLC = exhaustive testing of a finite model (bounded, automatic). TLAPS = a mathematical proof (unbounded, manual).** Never write "proven" or "proved" for a TLC result.

## Keeping TLC finite: constraints and symmetry

A naive counter can increment forever, so its state space is infinite and TLC would never terminate. Two mechanisms tame this.

A **state constraint** prunes the frontier. `Counter.tla` defines `StateConstraint == counter <= 5`, and `Counter.cfg` wires it in with `CONSTRAINT StateConstraint`. TLC stops exploring past any state that fails it, bounding the search to `counter \in 0..5`. This is exactly the "finite bounds" that make a TLC result *bounded* — change the constant and you've checked a different model.

**Symmetry reduction** is the other lever, used in the replicated specs. When replicas are interchangeable, many states differ only by relabeling (replica A and B swapped). Declaring the set of replica IDs as a symmetry set lets TLC explore one representative per equivalence class instead of all permutations — a large speedup that preserves exhaustiveness for symmetric invariants.

The `.cfg` file ties everything together. `Counter.cfg` uses explicit `INIT`/`NEXT` entries, lists each invariant under `INVARIANT`, and names the `CONSTRAINT`:

```
INIT Init
NEXT Next
INVARIANT TypeOK
INVARIANT NonNegative
CONSTRAINT StateConstraint
```

TLC exits non-zero on any violation, so CI fails automatically. With this much vocabulary — variables, `Init`/`Next`/actions, primes, invariants, and the TLC/TLAPS split — you can now read every spec in `tla/`.
