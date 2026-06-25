# The Axiom Book

The project's long-form documentation, built with
[mdBook](https://rust-lang.github.io/mdBook/): CRDTs from scratch, TLA+ from
scratch, and the refinement mapping that connects a verified specification to a
real Rust implementation. Every data-type chapter links to the TLA+ invariant it
refines.

## Build locally

```sh
# mdBook 0.5.3 (cargo install --version 0.5.3 mdbook, or a release binary)
mdbook build docs     # outputs to docs/book/
mdbook serve docs     # live-reloading preview at http://localhost:3000
```

CI builds the book on every push (the `book` job in `ci.yml`) and deploys it to
GitHub Pages from `main` (`.github/workflows/pages.yml`). To enable the deploy,
set **Settings → Pages → Build and deployment → Source** to **GitHub Actions**.

Chapters live in [`src/`](src/); the table of contents is
[`src/SUMMARY.md`](src/SUMMARY.md).
