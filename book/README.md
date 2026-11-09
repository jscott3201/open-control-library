# Documentation book

An [mdBook](https://rust-lang.github.io/mdBook/) site generated from the
repository's canonical files. Nothing here is authored twice: fault cards,
playbooks, clusters, point dictionaries, and SCHEMA.md are the source of
truth, and `tools/book/generate.py` derives `book/src/` from them at build
time (`book/src/` and `book/book/` are gitignored).

## Local preview

```sh
bash tools/book/install-mdbook.sh           # the pinned release, into target/mdbook-bin
python3 tools/book/generate.py              # needs PyYAML
target/mdbook-bin/mdbook serve book --open
```

`tools/book/install-mdbook.sh` installs the mdBook release named in
`tools/book/mdbook-version` and verifies the archive and the binary against the
SHA-256 digests in `tools/book/mdbook-checksums.txt`. CI and the gate script use
the same installer, so a local preview is built by the binary CI uses.

## CI and publishing posture

The per-PR gate (`.github/workflows/pr-gate.yml`, job "Book build") generates and
builds the book on every pull request into `development`, under the required
`CI OK` status, and `bash .agents/gate.sh` runs the same steps locally. The
build commands are not restated here; the gate script is their source.

`.github/workflows/book-pages.yml` builds the book again on pull requests that
touch its inputs, and **publishes it from `main`**: on every push to `main` (a
release) and on a manual dispatch from `main`. A dispatch from any other branch
builds and publishes nothing. The published book is therefore the released
tree, not the `development` tip.

Pages is configured to build from a workflow (Settings → Pages → Source:
"GitHub Actions"). The site lands at
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
