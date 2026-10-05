# Changelog

All notable changes to open-control-library are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are
git tags (`vX.Y.Z`); each pins one open-control engine revision through
`ENGINE_PIN`, and every rule marked `verified` in that release passed its
vectors against that revision.

A rule's identity across releases is its `verified.content_id`, the engine's
exported `cxf:fnv1a128:` tag. A change to a rule's logic, thresholds or vectors
is a rule change and is listed here by rule ID.

## [Unreleased]

## [0.1.0] - 2026-10-05

First tagged release. The date is the preparation date; use the tag date if
they differ.

### Engine pin

- `ENGINE_PIN` is `41e997fd130c5e454446b40bcc3ba576429876b4`, open-control-engine
  `development` commit 41e997f (open-control-engine#308). It replaces `e2ff2f8`, the tip of that
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
  by a single `CI OK` status. The same jobs remain in
  `.forgejo/workflows/ci.yml`.

[Unreleased]: https://github.com/jscott3201/open-control-library/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jscott3201/open-control-library/releases/tag/v0.1.0
