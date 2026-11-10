# open-control-library — Agent Guide

**open-control-library** (repo `github.com/jscott3201/open-control-library`) is a library of
control content that runs on the
[Open Control Engine](https://github.com/jscott3201/open-control-engine): fault detection rules,
Guideline 36 routine reference bundles, point dictionaries, remediation playbooks and fault
clusters. Every rule is a CDL block graph shipped as CXF JSON-LD with executable vectors, and a
rule is `verified` only when the engine at the commit in `ENGINE_PIN` replays every vector green.

What it is not: it is not the engine, and it does not grow engine-shaped tooling. Code that loads,
replays, compiles or authors control graphs belongs in the engine repository. The verifier and
routine compiler under `tools/` are inherited and are scheduled for removal once the pinned `oce`
tool replaces them (tracking issue #105). Nothing here is a deployable controller: the routine
bundles are reference parts of a sequence.

`CLAUDE.md` is a symlink to this file.

---

## Start here

Read in this order, every session, before the first change:

1. This file.
2. [`SCHEMA.md`](SCHEMA.md): the normative contracts for every file format in the tree.
3. [`Handoff.md`](Handoff.md), "Boundaries to preserve": what the routine bundles are and are not.
4. The GitHub issue you are assigned, then its milestone's tracking issue (the issue labelled
   `tracking` lists the order and the hard dependencies).
5. [`CHANGELOG.md`](CHANGELOG.md), for how changes are recorded and cited.

Decisions recorded in an issue or the tracking issue are settled. Do not re-litigate one inside a
pull request: comment on the issue and keep building.

---

## Milestone work: issue → branch → pull request → merge

Work is executed by agents starting cold, so the process is the same for every work item.

**Pick up.** Work only from an open issue in the current milestone. An issue labelled
`agent-ready` names its entry points, acceptance checks and a "do not touch" list: those are the
contract. An issue labelled `blocked` waits on another; read the body. An issue labelled
`design-note-first` gets a short design note as a **draft pull request** first, and the build waits
for a review comment.

**Branch.** From `development`: `<area>/<what-it-does>` (`ci/github-gate-model`,
`tools/verifier-complete-frames`). Never a milestone, step or issue number as the name.

**Build.** One pull request per issue unless the issue says otherwise. Keep the diff to the issue's
scope; a fix you notice elsewhere is a new issue, not a drive-by.

**Prove.** Fetch the pinned sources once (`bash .github/scripts/pinned-sources.sh fetch`), then
run `bash .agents/gate.sh full` first-hand on the final head and **paste the tail of the real
output** into the pull request: the step list, the observed totals and the closing line. A summary
of a gate is a claim about a gate. A fix round re-runs it; the tail in the description always
names the head commit that is being merged.

**Pull request.** Title `type(scope): what it does` in the imperative (`fix(verify): …`, `ci: …`,
`docs(schema): …`). Body: `Closes #N`, what changed and why in two paragraphs, the gate tail, the
`CHANGELOG.md` entry you added, and the documents you updated. Every change to a rule, a vector, a
format, a tool or a published claim gets a `CHANGELOG.md` entry **in the same pull request**,
listed by rule id where a rule changed and cited `(GitHub #N)`. A fix round pushes to the **same
branch**; never a second pull request for the same work.

**Merge.** `CI OK` is the one required status. Confirm that it **reported** green, not merely that
nothing is red: a pull request with no checks looks like one with no failing checks. Also check
`book-pages` on the same head commit when it ran; it is path-filtered and is not under `CI OK`.
Squash-merge into `development` once `CI OK` is green on a branch that is up to date with
`development`. The merge starts `release-gate.yml` on the merge commit; a red run there is fixed
forward or reverted before the next change merges.

**Release.** A release is a pull request from `development` into `main` through
`release-gate.yml`, tagged `vX.Y.Z` on `main`; the book publishes from `main`. `main` requires
`CI OK` on a branch that is up to date with it, and a merged release leaves a commit on `main` that
`development` does not contain (a merge commit or a squash; rebase merging is off). So every
release after the first starts by bringing `main` back into `development`: branch from
`development`, `git merge origin/main`, open a pull request into `development`, and merge it with
a **merge commit, not a squash**. A squash copies the content and leaves `main`'s commit outside
`development`'s history, so the next release pull request would still be behind `main` and could
not merge. It is the one pull request into `development` that is not squash-merged.

---

## Standing rules for every pull request

1. **A failing scenario is a finding.** Report it. Never edit an expectation, a tolerance or a
   vector to make it pass. A new scenario may be added, and is named in the pull request.
2. **Rule logic changes only under an issue that says so.** Blocks, classes, parameters, wiring and
   thresholds of a fault rule or a routine graph are behaviour. A content id that changes is a
   rule change and is listed in `CHANGELOG.md` by rule id.
3. **Routine `status` stays `reference`, and `routines/generated-registry.json` stays empty.** Do
   not promote a reference bundle, rename a graph to imply deployment, or treat the empty
   deployment check as an execution test.
4. **Formats and schemas stay at revision 1 until 1.0.0 and change in place.** No backwards
   compatibility: no new revision number, no reader for an older shape, no migration table. A
   change to a format edits `SCHEMA.md` and converts the corpus in the same pull request. Where a
   sentence in `SCHEMA.md` still asks for an identifier bump, or an identifier already carries a
   higher number, the bundle-contract issue resolves it; do not add to either.
5. **No engine-shaped code.** Nothing that loads, replays, compiles or authors control graphs is
   added here. What the library needs from the engine is asked for in an engine issue and consumed
   at `ENGINE_PIN`.
6. **`ENGINE_PIN` is the only pin of the engine**: one file, one 40-hex commit. It advances only
   under an issue that says so, and every advance re-verifies and re-stamps. Never change the
   engine selection to get a test green.
7. **No absolute paths (a home directory) and no UUID-shaped strings** in any tracked file. The
   tracked-file scan rejects them, and history then has to be scrubbed.
8. **Nothing depends on an archived or external prototype, and nothing new is imported without an
   issue.** What the gate fetches is pinned: the engine commit in `ENGINE_PIN`, the two Modelica
   Buildings commits, and its tools (the Python requirements by version, the crates in each
   `Cargo.lock`, mdBook by checksum, every action by commit). The dataset fixture the gate replays,
   `tools/dataset_harness/tests/fixtures/lbl_fpu_tiny`, is synthetic data written here in the
   shape of the public LBNL fan-powered-unit dataset; it holds no source records. A new outside
   spec, harness, fixture or dataset comes in only under an issue that names its source, its
   licence and how it is pinned.
9. **Change a gate command in `.github/workflows/pr-gate.yml` first, then in `.agents/gate.sh`,
   then in the pinned list in `.github/scripts/check_workflow_gates.py`.** The workflow-rules check
   holds the workflow and the script to that list and fails until all three agree. Do not restate
   the command list in any other document: point at the script. A failing gate is fixed at its
   cause, never by weakening the gate.
10. **Do not commit licensed source documents** (the ASHRAE guideline, upstream PDFs). Cite the
    clause.

---

## CI and the gate

CI is GitHub Actions on hosted Linux runners, from three workflows in `.github/workflows/`.

| Workflow | Runs on | What it does |
| --- | --- | --- |
| `pr-gate.yml` | every pull request into `development`; manual dispatch | The per-PR gate: workflow rules and their fixtures, the tracked-file scan, lints and unit tests, engine verification, the book build. Ends in `CI OK`, the required status on `development`. Never runs on a push. |
| `release-gate.yml` | every push to `development` and `main`; release pull requests into `main`; daily on the `development` tip; manual dispatch | Executes `bash .agents/gate.sh full` from a fresh checkout with no restored cache. Ends in its own `CI OK`, the required status on `main`. A push run in progress is never cancelled. |
| `book-pages.yml` | pull requests into `development` that touch the book's inputs; every push to `main`; manual dispatch | Builds the book; on `main` it also publishes it to GitHub Pages. Path-filtered, so it is not a required status. |

Today the per-PR gate is the whole gate (about five minutes), so a green `CI OK` on a pull request
does mean every check ran. The release gate's value is the re-run: on the merged tree, and daily,
which is where a changed runner image or toolchain shows up under an unchanged tree. A check that
outgrows the per-PR budget moves to the release gate by editing both workflows and the gate script
together. `CI OK` requires `success` from every job in its workflow: a skipped or cancelled job is
a failure.

**The local gate.** `bash .agents/gate.sh` mirrors `pr-gate.yml`. `bash .agents/gate.sh full` is
what `release-gate.yml` executes: the same steps from a clean build and a fresh Python
environment. The script is the single runnable source of the gate commands, and the workflow-rules
check holds it and `pr-gate.yml` to one pinned list. It runs every step even after a failure and
ends with each step's result, the totals it observed and a list of what it cannot cover locally. A
green local run is not a green CI.

**Where the engine comes from.** `ENGINE_PIN` names one engine commit.
`bash .github/scripts/pinned-sources.sh fetch` fetches that exact commit into the sibling
directory `../open-control`, and the Modelica Buildings commits named in
`routines/g36/SOURCE_RELEASE_PIN` and `SOURCE_DEVELOPMENT_PIN` into `../modelica-buildings-release`
and `../modelica-buildings-development`. CI runs the same script. `tools/verify` builds against
the engine checkout as a path dependency, and its build script refuses a checkout whose `HEAD`
differs from the pin or whose tracked files are modified. The Rust toolchain is the channel named
in that checkout's `rust-toolchain.toml`, in CI and in the gate script, so the pin selects the
compiler too. Nothing is downloaded as a built artifact: the engine publishes no release objects
and nothing to crates.io.

**Workflow rules.** `.github/scripts/check_workflow_gates.py` fails the gate when a workflow or
the gate script breaks one of these, and `test_check_workflow_gates.py` holds at least one refused
fixture for each. In every workflow: each action pinned to a full commit SHA on a plain `uses:`
line with its tag in a trailing comment; each checkout without the job token; a timeout of at most
an hour on every job; the hosted `ubuntu-latest` runner; the `bash` default shell for every `run:`
block; no step allowed to fail without failing its job; no `${{ }}` expression inside a `run:`
block; a read-only token, widened only by the Pages deploy job, which runs only for a push or a
dispatch on `main`; exactly these three workflow files. In the gate workflows: exactly the
triggers in the table above; no job or step condition except `CI OK`'s `always()`; every job
listed in its `CI OK`, which runs exactly the result check; in the release gate, no cache and every
checkout at the one commit resolved for the run (`github.sha`; `development` for the scheduled
run). **The gate commands are pinned** in the check: each job of `pr-gate.yml` and
`release-gate.yml` must run exactly the pinned command lines, in order, and the steps of
`.agents/gate.sh` must run exactly the same gate commands, so a command cannot leave the workflow
or the script, lose a flag or gain `|| true` under a green `CI OK`. The check reads the files; it
does not follow the script's control flow.

---

## Repository layout

| Path | Purpose |
| --- | --- |
| `SCHEMA.md` | The normative contracts. |
| `faults/<family>/<ID>/` | One fault rule per folder: `card.md`, `rule.cxf.jsonld`, `vectors.json`, `diagram.svg`. `faults/registry.json` maps them. |
| `routines/` | The routine catalog: two Guideline 36 reference bundles, schemas, the source inventory and its pins. |
| `points/`, `playbooks/`, `clusters/` | Point dictionaries, remediation playbooks, fault clusters. |
| `tools/` | The inherited verifier and routine compiler, the lints, the dataset harness and the book generator. |
| `book/` | mdBook configuration; `book/src/` and `book/book/` are generated and untracked. |
| `ENGINE_PIN` | The engine commit every `verified` stamp refers to. |
| `.agents/gate.sh` | The gate. |
| `.github/scripts/` | The workflow rules check, the `CI OK` result check, the tracked-file scan, the pinned-sources script and their fixtures. |

GitHub is the host: branches, pull requests, issues, milestones and CI live there; use `gh`. Pull
requests merged on the retired Forgejo host share numbers with GitHub's, so cite a pull request or
issue on GitHub as `(GitHub #N)`. Old `CHANGELOG.md` entries that cite Forgejo are history and stay
as written.
