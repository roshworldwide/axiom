# TLA+ setup

This page explains how to get the TLA+ tooling, how to model-check a spec, and
the crucial difference between **TLC** and **TLAPS** — the two tools Axiom uses,
which give *different strengths of guarantee*.

## The two tools (read this first)

TLA+ specs can be analyzed two ways. They are **not** interchangeable, and the
Axiom [claims policy](../CLAUDE.md#claims-policy-read-this-before-writing-any-result-down)
depends on never confusing them.

### TLC — the model checker (what we use most)

TLC **explores states**. You give it a *finite* model (e.g. "3 replicas, at most
2 increments each") and it mechanically enumerates every reachable state,
checking your invariants at each one. If an invariant can be violated within
those bounds, TLC finds the shortest counterexample trace and prints it.

- **Strength:** fully automatic, finds concrete counterexamples, great for
  catching design bugs fast.
- **Limit:** results are **bounded**. "TLC found no violation" means *no
  violation exists within the finite model you specified* — not a proof for all
  inputs. A bug that only appears with 4 replicas won't be seen in a 3-replica
  model.
- **How we report it:** *"model-checked with TLC up to N replicas / M ops."*
  Always state the bounds. **Never** call a TLC result "proven."

### TLAPS — the proof system (what we use sparingly, for flagship lemmas)

TLAPS checks **deductive proofs**. You write the proof (a hierarchy of steps and
justifications); TLAPS, with backend provers (Zenon, Isabelle, SMT), mechanically
verifies that each step follows. A successful check is an **unbounded** proof: it
holds for *all* replicas, *all* values, forever — no finite bound.

- **Strength:** unbounded, machine-checked truth.
- **Limit:** you must *write* the proof; this is real work and not always
  feasible. Axiom proves only the simplest, highest-value lemmas this way (e.g.
  G-Counter merge commutativity in Week 2).
- **How we report it:** *"machine-checked proof via TLAPS."* Only where a proof
  actually checks.

**One-line summary:** TLC = exhaustive testing of a finite model (bounded
confidence, automatic). TLAPS = a real mathematical proof (unbounded, manual).

## Getting the tools

### TLC (`tla2tools.jar`)

Axiom pins **`tla2tools.jar` v1.7.4** ("The Xenophanes release", the latest
*stable* release — there is no v1.8.x as of 2026). It bundles the TLC model
checker (`tlc2.TLC`), the SANY parser, and the PlusCal translator.

- Requires **Java 11+**.
- Download (pinned, reproducible):

  ```sh
  curl -fLsS -o tla2tools.jar \
    https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar
  ```

The jar is **git-ignored** (`.gitignore` excludes `*.jar`); CI downloads it on
each run, and you download it once locally.

### TLAPS (`tlapm`) — only needed when checking proofs

There is no recent conventional "stable" `tlapm` binary; the most reliable
prebuilt is the **1.6.0-pre** rolling release. On Linux x86_64 (e.g. CI):

```sh
curl -fLsS -o tlapm.tar.gz \
  https://github.com/tlaplus/tlapm/releases/download/1.6.0-pre/tlapm-1.6.0-pre-x86_64-linux-gnu.tar.gz
mkdir -p tlaps && tar -xzf tlapm.tar.gz -C tlaps
./tlaps/tlapm/bin/tlapm --version
```

TLAPS-in-CI is genuinely fiddly (bundled provers; exit codes on unproved
obligations are less crisp than TLC's). We gate it behind its own job and also
scan output for unproved obligations. It is introduced in Week 2.

## Running TLC

General form (the spec's basename + its `.cfg` model):

```sh
java -cp tla2tools.jar tlc2.TLC -workers auto -config Counter.cfg Counter.tla
```

`java -jar tla2tools.jar …` is an alias for the same `tlc2.TLC` entry point.
TLC **exits non-zero** on any invariant/property violation, deadlock, or parse
error, so CI fails automatically — no output-parsing required.

### macOS note (this project's dev machine)

OpenJDK 17 here was installed with `brew install openjdk@17`, which is
**keg-only**, so `java` is *not* on `PATH` (`/usr/bin/java` is an Apple stub that
errors). Use the full path locally:

```sh
/opt/homebrew/opt/openjdk@17/bin/java -cp tla2tools.jar tlc2.TLC -config Counter.cfg Counter.tla
```

## How CI runs this

`.github/workflows/ci.yml` has a `tlc` job that installs JDK 17, downloads the
pinned `tla2tools.jar`, then runs TLC on **every** `tla/*.cfg` (matched to its
`.tla`). It no-ops cleanly when there are no specs and fails the build if any
spec reports an error. Adding a new `Foo.tla` + `Foo.cfg` is all that's needed
to get it model-checked in CI.
