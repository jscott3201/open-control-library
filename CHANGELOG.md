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
- The workflow-rules check pins the gate commands (GitHub #112). Every job of
  `pr-gate.yml` and `release-gate.yml` must run exactly the pinned command
  lines, in order, so a command can no longer leave a gate workflow, lose a
  flag or gain `|| true` while `CI OK` stays green. The lines are compared,
  not what surrounds them: a workflow or job `env`, a `working-directory` or an
  added action step can still change what a pinned line does. Of
  `.agents/gate.sh` the check proves that its first command is
  `set -euo pipefail` and no later line changes the shell options; that every
  `step` and `engine_step` line holds one command and no control operator; that
  each of those commands is a gate command `pr-gate.yml` runs or a call of a
  pinned function; that each pinned function matches its pin line for line and
  operator for operator; and that between them they run every gate command
  `pr-gate.yml` runs. It does not prove that a step is reached or that its
  result counts: the check reads the script and does not follow its control
  flow, so a final `exit 0`, a redefined `step` or `record`, an `if false`
  around a step, a `trap`, a heredoc that swallows a step, and a function
  written in another form that shadows a command all pass. No gate command
  changed.
- The workflow-rules check also requires, in every workflow, a plain
  SHA-pinned `uses:` line for every action, the hosted `ubuntu-latest` runner
  with no container and no services, the `bash` default shell, a timeout of at
  most 60 minutes, no job or step that is allowed to fail, no `${{ }}`
  expression in a `run:` block, and the Pages deploy job's exact condition and
  token; and in the gate workflows, the exact trigger sets and the exact inputs
  of every checkout, so that every job of `pr-gate.yml` checks out the pull
  request itself (GitHub #112).
- `release-gate.yml` checks out `github.sha` for dispatched and pull-request
  runs as it already did for pushes, instead of a ref that can move while the
  run waits (GitHub #112). Both jobs read the commit resolved when the event
  fired; the scheduled run still checks out `development` by name.
- `cxf-library/vectors/v1` has no per-vector `tolerance` any more, and the
  format is closed (GitHub #119, issue #116). `tools/verify` put no upper bound
  on an expectation's optional `tolerance`, so a large one (`1e308`, or `1e6`
  against a setpoint of 0.6) made a Real comparison that could not fail. No
  vector, routine or fixture in the repository set the field, so it is removed
  instead of bounded: a Real output matches within a fixed 1e-9 of `equals`,
  and a vector that carries `tolerance` is refused as an unknown field, by
  `tools/verify` and by `tools/routines/reference.py` (which accepted values up
  to 1e-6). Named tolerances belong to the bundle contract (GitHub #106). A
  field `SCHEMA.md` does not name is now refused instead of ignored, in the
  file, its clock, a scenario, an expectation (the bundle is reported `ERROR`)
  and a step of an input's step list (the scenario is reported `FAIL`), and
  two scenarios of one file may not share a name (`ERROR`). `--trace-json`
  refuses the same files with status 2. No bundle, vector or card changed: no
  file in the library carried an unknown field or a repeated scenario name,
  and all 137 fault bundles (1,760 scenarios) and both routine reference
  replays pass as before.

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

### Fixed

- The tracked-file scan fails when `git grep` cannot search (GitHub #112). An
  exit status above 1 was read as "no match" and its message discarded, so a
  scan that never ran reported the tree clean. It now exits 2, names the
  pattern and leaves git's message on stderr.
- `tools/verify` refuses vectors that compare nothing instead of reporting them
  as passed (GitHub #114). A scenario with an empty `expect` list, or with an
  expectation whose window holds no tick (it ends before it starts, ends before
  the first tick, starts after the last tick or falls between two ticks), is
  reported `FAIL`. A vectors file with no scenario, or whose `clock.step_s` or
  `clock.horizon_s` is not a finite number greater than zero, is reported
  `ERROR` for the bundle; a zero `step_s` used to run without end. Each refusal
  names the bundle, the scenario and the reason, and the run exits with status
  1. `--trace-json` now refuses a vectors file with no scenario (status 2); it
  still accepts an empty `expect` list and a zero horizon, because a trace
  compares nothing itself. `SCHEMA.md` ("Verification") states the rules and
  the exit statuses. No bundle, vector or card changed: all 137 fault bundles
  (1,760 scenarios) and both routine reference replays (21 authored scenarios
  and the generated matrix) pass as before.
- `tools/verify` refuses a clock that schedules more than 1,000,000 ticks
  (`floor(horizon_s / step_s)` of 1,000,000 or more), the limit `--trace-json`
  already had (GitHub #119, issue #116). `step_s: 1e-300` against a horizon of
  one second, or a 60 s step against `horizon_s: 1e300`, ran without end: the
  replay had no limit, and for a window with no tick inside it the coverage
  check walked the whole schedule before it could refuse the window. The clock
  is now checked first and the bundle is reported `ERROR` before any tick runs.
  The largest bundle in the library schedules 2,593 ticks.
- `tools/simharness/harness.py` refuses a run that ends inside the 3600 s lead
  margin of an ungated plant rule, and says why (GitHub #119, issue #116). It
  used to emit the window from 3600 s to the last sample, which ends before it
  starts for a run shorter than an hour (a one-sample run included); the
  verifier once passed such a window without comparing anything and now
  refuses it. A run whose last sample is at 3600 s or later is unchanged.

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
