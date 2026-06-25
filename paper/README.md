# Axiom paper

`axiom.md` is the working draft of *"Axiom: From Formal Specification to Verified
Implementation of CRDTs"* (PLDI/OSDI framing).

Build a PDF with [pandoc](https://pandoc.org):

```sh
pandoc axiom.md -o axiom.pdf
```

Every evaluation number is pulled from the repository and should be regenerated
before submission:

- **TLC state counts** — the bounds table in [`../tla/README.md`](../tla/README.md),
  produced by the CI `tlc` job.
- **TLAPS obligations** — the CI `tlaps` job (`tlapm` over `tla/*Proofs.tla`).
- **proptest / test counts** — `cargo test --workspace`.

The draft is deliberately calibrated: it distinguishes *model-checked (bounded)*,
*machine-proved (TLAPS, unbounded)*, *property-tested*, and *trace-validated*
throughout, and §9 states exactly what is not established.
