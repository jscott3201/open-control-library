# Changelog

All notable changes to open-control-library are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are
git tags (`vX.Y.Z`); each pins one open-control engine revision through
`ENGINE_PIN`, and every rule marked `verified` in that release passed its
vectors against that revision.

A rule's identity across releases is its `verified.content_id`, the engine's
exported `cxf:fnv1a128:` tag. A change to a rule's logic, thresholds or vectors
is a rule change and is listed here by rule ID.

Pull requests are cited with their forge: `(GitHub #N)` for this repository on
GitHub, its primary forge, and `(Forgejo #N)` for the retired Forgejo history,
whose numbers overlap GitHub's.

## [Unreleased]

### Engine pin

- `ENGINE_PIN` is `07f5c2ff6cd76cce6e01400ddc84dfbae70084ad`, the commit the
  open-control-engine tag `v0.4.0` points at (GitHub #103). It replaces
  `41e997f`, which was 146 commits behind that tag. All 137 verified cards were
  re-verified and re-recorded (`verified.engine_rev: 07f5c2f`,
  `verified.date: 2026-11-09`). No rule, vector or content id changed: every
  recorded `verified.content_id` equals the `cxf:fnv1a128:` tag the engine
  exports at `v0.4.0`, all 1,760 fault scenarios and both routine reference
  replays (21 authored scenarios and the generated matrix) pass, and across the
  1,781 authored scenarios every boundary output at every tick has the value it
  had at the old pin.

### Changed

- `tools/verify` drives the engine through complete frames (GitHub #103). The
  engine removed per-input staging and `tick` when complete frames became its
  only execution surface, so the verifier did not compile against any engine
  from `v0.2.0` on. Each tick now builds one frame in which every boundary
  input carries its latest staged value, executes it with `prepare_frame` and
  `execute_frame`, and reads expectations and traces from the boundary outputs
  that frame committed. The tick schedule is unchanged. A scenario that leaves
  a boundary input of its graph without a value at any tick now fails and names
  the input; the old driver ran it on the engine's zero or `false` seed. No
  scenario in the library relied on a seed. A `--trace-json` refusal names the
  scenario it stopped on.
- CI follows the engine repository's gate model (GitHub #104). `pr-gate.yml` runs
  on every pull request into the new `development` branch and ends in `CI OK`,
  which now requires success from every job and counts a skipped job as a
  failure. `release-gate.yml` executes the gate script after every merge into
  `development`, on release pull requests into `main`, on pushes to `main` and
  daily. `book-pages.yml` builds the book on pull requests that touch its inputs
  and publishes it from `main`. They replace `.github/workflows/ci.yml` and
  `.github/workflows/book.yml`. No rule, vector, tool source or pin changed, and
  the gate reports the same totals: 137 fault bundles, 1,760 scenarios and both
  routine reference replays.
- The verification commands run with `--locked`, on the Rust toolchain named in
  the pinned engine checkout's `rust-toolchain.toml` instead of the floating
  `stable`, so `ENGINE_PIN` selects the compiler as well as the engine.
- mdBook is installed by `tools/book/install-mdbook.sh`, which verifies the
  release archive and binary against pinned SHA-256 digests.

### Added

- `.agents/gate.sh`, the single local source of the gate commands
  (`bash .agents/gate.sh` mirrors the per-PR gate, `bash .agents/gate.sh full`
  is what the release gate executes), and `.github/scripts/pinned-sources.sh`,
  which fetches and checks the three pinned sibling checkouts (GitHub #104).
- A workflow-rules check with one refused fixture per rule, a tracked-file scan
  for absolute home paths, UUID-shaped strings and secret shapes, and fixtures
  for the `CI OK` result check, the pinned-sources check and the mdBook
  installer (GitHub #104).
- `AGENTS.md` (the issue-to-merge process, the standing rules, CI and the
  gate), a pull-request template and a work-item issue template (GitHub #104).

## [0.1.0] - 2026-10-05

First tagged release (GitHub #102). The date is the preparation date; use the tag date if
they differ.

### Engine pin

- `ENGINE_PIN` is `41e997fd130c5e454446b40bcc3ba576429876b4`, open-control-engine
  `development` commit 41e997f (open-control-engine Forgejo #308). It replaces `e2ff2f8`, the tip of that
  PR's branch, which no engine branch contains. The two commits have the same
  tree, so the verified engine source is unchanged. All 137 verified cards were
  re-verified and re-recorded (`verified.engine_rev: 41e997f`,
  `verified.date: 2026-10-05`); every recorded `content_id` is unchanged.

### Added

- **137 verified fault rules** across fourteen equipment families (AHU, VAV,
  fan-powered boxes, rooftop units, heat pumps, chillers, cooling towers,
  hot-water plants, heat exchangers, fan coils, energy recovery ventilators,
  pumps, VFDs, and cross-equipment sensor health). Each rule is a card, a CXF
  graph, executable vectors and a diagram; `faults/registry.json` lists them.
  Together the vectors hold 1,760 scenarios.
- Canonical point dictionaries for fifteen equipment and zone classes, grounded
  in Brick 1.4.4 and ASHRAE 223P.
- 22 remediation playbooks and 10 fault clusters.
- Two independently authored G36-2018 reference subsequences in the routine
  catalog (thermal-zone state classification and cooling-only VAV airflow
  setpoints). They are engine-replayed reference parts, not deployable
  controllers; the generated deployment registry is empty.
- Tooling: `tools/verify` (engine-pinned rule verifier and linter),
  `tools/routine-compiler` (G36 declaration pipeline), `tools/simharness`,
  `tools/dataset_harness` (offline dataset replay), lints, and the book
  generator.
- The published book of every rule, playbook and point dictionary.
- CI on GitHub Actions (`.github/workflows/ci.yml`): lints, G36 source and
  declaration checks, routine replay, full rule verification against
  `ENGINE_PIN`, the dataset-harness fixture replay and the book build, summarized
  by a single `CI OK` status. The Forgejo workflow is removed; GitHub is the
  primary forge.

[Unreleased]: https://github.com/jscott3201/open-control-library/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jscott3201/open-control-library/releases/tag/v0.1.0
