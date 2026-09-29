# Documentation book

An [mdBook](https://rust-lang.github.io/mdBook/) site generated from the
repository's canonical files. Nothing here is authored twice: fault cards,
playbooks, clusters, point dictionaries, and SCHEMA.md are the source of
truth, and `tools/book/generate.py` derives `book/src/` from them at build
time (`book/src/` and `book/book/` are gitignored).

## Local preview

```sh
brew install mdbook            # pinned in CI at v0.5.4; keep local in sync
python3 tools/book/generate.py
mdbook serve book --open       # or: mdbook build book
```

## CI and publishing posture

The project's primary CI (`.forgejo/workflows/ci.yml`, job "Book build")
generates and builds the book on every push to `main` and every pull request,
so the generator and every generated page are continuously validated.
**Deployment to GitHub Pages is manual**: `.github/workflows/book.yml` runs
only on `workflow_dispatch` from `main`, on the GitHub mirror.

To publish: enable GitHub Actions and Pages on the GitHub mirror (Settings →
Pages → Source: "GitHub Actions"), run the `book` workflow from `main`, then
disable Actions again if it is not otherwise needed. The site lands at
`https://jscott3201.github.io/open-control-library/` (matching `site-url` in
`book.toml`).

## Open-source readiness checklist

Work through this before flipping the repo public or enabling deployment:

- [ ] **License**: choose and add a LICENSE file (none exists today). Note
      the CXF rules themselves are data-like artifacts; a permissive license
      (MIT/Apache-2.0/BSD) or an open-data license both fit — decide
      deliberately.
- [ ] **Third-party text audit**: cards transcribe equations, defaults, and
      diagnosis lists adapted from ASHRAE Guideline 36 §5.16.14 and quote the
      HVAC FDD Reference v1.0. Equations and facts are fine; review verbatim
      prose passages against ASHRAE's copyright before publishing (the
      addendum PDFs carry an explicit reproduction notice).
- [ ] **Provenance statement**: the HVAC FDD Reference v1.0 is cited
      throughout as grounding — confirm its own redistribution terms and add
      an attribution section to the book introduction.
- [x] **Repo hygiene**: `_research/` is local-only (untracked, gitignored 2026-08-18) — decided
      whether they ship, move, or are excluded from the book (the generator
      currently ignores them).
- [ ] **README framing**: the introduction currently says "Private library";
      reword for a public audience.
- [ ] **Engine cross-link**: cards pin open-control engine revisions; if that
      repo stays private, the book should say so where it links out.
