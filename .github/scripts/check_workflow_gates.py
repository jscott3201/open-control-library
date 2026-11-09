#!/usr/bin/env python3
"""Static rules for the workflows in .github/workflows.

A workflow edit can turn the gate into a green light that proves less than it appears to. This
check fails on the edits that do that, before GitHub Actions runs them. Each rule has an id, and
test_check_workflow_gates.py holds one failing fixture per id.

  workflow-set      the directory holds exactly pr-gate.yml, release-gate.yml and book-pages.yml
  action-pin        every action is referenced by a full commit SHA with its tag in a trailing
                    comment
  checkout-token    every actions/checkout step sets persist-credentials: false
  job-timeout       every job sets timeout-minutes
  no-soft-failure   no job or step may be allowed to fail without failing its job
  event-in-run      no run block interpolates the github.event context (pass values through env)
  permissions       the workflow token is contents: read; only the Pages deploy job may widen it
  concurrency       one group per event and ref; only a superseded pull-request run is cancelled
  pr-gate-triggers  pr-gate.yml runs on pull requests into development and on dispatch, never on
                    a push
  release-triggers  release-gate.yml runs on pushes to development and main, pull requests into
                    main, a schedule and dispatch
  ci-ok-needs       each gate workflow ends in a CI OK job that always runs, needs every other
                    job and runs ci_ok.py on their results
  gate-if           no job or step in a gate workflow is conditional, except CI OK's always()
  gate-commands     pr-gate.yml still runs each gating command; release-gate.yml executes
                    .agents/gate.sh full and restores no cache

Usage: python3 .github/scripts/check_workflow_gates.py [workflow-directory]
Needs PyYAML (pinned with the other gate requirements).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import yaml

WORKFLOWS = ("book-pages.yml", "pr-gate.yml", "release-gate.yml")
GATES = ("pr-gate.yml", "release-gate.yml")
CI_OK_JOB = "ci-ok"
CANCEL_SUPERSEDED_PR = "${{ github.event_name == 'pull_request' }}"
# Written in two halves so that a grep for the key over .github finds real uses only.
SOFT_FAILURE_KEY = "continue-on-" + "error"
ACTION_LINE = re.compile(r"^\s*(?:-\s+)?uses:\s*(?P<ref>.*?)\s*$")
PINNED_ACTION = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40} # \S+$")
EVENT_CONTEXT = re.compile(r"github\.event(?![_A-Za-z0-9])")
CACHING_ACTIONS = ("actions/cache", "swatinem/rust-cache")
# Commands pr-gate.yml must keep running. Removing a job removes it from CI OK's needs as well, so
# nothing else would notice a gate that silently lost one of these.
PR_GATE_COMMANDS = (
    "python3 .github/scripts/check_workflow_gates.py",
    "python3 -m unittest discover -s .github/scripts",
    "bash .github/scripts/check-no-secrets.sh",
    "python3 tools/lint/registry.py",
    "python3 -m unittest discover -s tools/lint/tests",
    "bash .github/scripts/pinned-sources.sh fetch",
    "python3 tools/lint/g36_source.py --check",
    "cargo test --locked --manifest-path tools/routine-compiler/Cargo.toml",
    "cargo test --locked --manifest-path tools/verify/Cargo.toml",
    "cargo run --locked --manifest-path tools/verify/Cargo.toml -- --all",
    "python3 tools/routines/reference.py",
    "python3 tools/dataset_harness/harness.py replay",
    "bash tools/book/install-mdbook.sh",
    "python3 tools/book/generate.py",
    "mdbook build book",
)


def triggers(workflow: dict) -> dict:
    """The `on:` mapping. YAML 1.1 reads the bare key `on` as the boolean true."""
    value = workflow.get("on", workflow.get(True))
    if isinstance(value, str):
        return {value: None}
    if isinstance(value, list):
        return {name: None for name in value}
    return value if isinstance(value, dict) else {}


def branches(trigger: object) -> list:
    value = trigger.get("branches") if isinstance(trigger, dict) else None
    return sorted(value) if isinstance(value, list) else []


def jobs_of(workflow: dict) -> dict:
    jobs = workflow.get("jobs")
    return jobs if isinstance(jobs, dict) else {}


def steps_of(job: object) -> list:
    steps = job.get("steps") if isinstance(job, dict) else None
    return [step for step in steps or [] if isinstance(step, dict)]


def run_text(workflow: dict) -> str:
    """Every run block, with shell line continuations and runs of blanks folded to one space."""
    blocks = [str(step["run"]) for job in jobs_of(workflow).values() for step in steps_of(job)
              if "run" in step]
    return " ".join(re.sub(r"\\\n", " ", "\n".join(blocks)).split())


def check_text(name: str, text: str) -> list[str]:
    failures = []
    for number, line in enumerate(text.splitlines(), start=1):
        if line.lstrip().startswith("#"):
            continue
        match = ACTION_LINE.match(line)
        if match and not PINNED_ACTION.match(match["ref"]):
            failures.append(
                f"{name}: action-pin: line {number}: an action must be `owner/repo@<40-hex commit>"
                f" # <tag>`: {match['ref']}"
            )
        if SOFT_FAILURE_KEY in line:
            failures.append(
                f"{name}: no-soft-failure: line {number}: a failing step must fail its job"
            )
    return failures


def check_workflow(name: str, workflow: dict) -> list[str]:
    failures = []

    def fail(rule: str, message: str) -> None:
        failures.append(f"{name}: {rule}: {message}")

    if workflow.get("permissions") != {"contents": "read"}:
        fail("permissions", "top-level permissions must be exactly `contents: read`")

    concurrency = workflow.get("concurrency")
    group = str(concurrency.get("group", "")) if isinstance(concurrency, dict) else ""
    if "github.event_name" not in group or "github.ref" not in group:
        fail("concurrency", "the group must be keyed on github.event_name and github.ref")
    if not isinstance(concurrency, dict) or (
        concurrency.get("cancel-in-progress") != CANCEL_SUPERSEDED_PR
    ):
        fail(
            "concurrency",
            f"cancel-in-progress must be {CANCEL_SUPERSEDED_PR}, so that a push, scheduled or"
            " dispatched run in progress always finishes",
        )

    jobs = jobs_of(workflow)
    if not jobs:
        fail("job-timeout", "the workflow defines no jobs")
    for job_id, job in jobs.items():
        if not isinstance(job, dict):
            fail("job-timeout", f"job {job_id} is not a mapping")
            continue
        timeout = job.get("timeout-minutes")
        if not isinstance(timeout, int) or isinstance(timeout, bool) or timeout <= 0:
            fail("job-timeout", f"job {job_id} sets no timeout-minutes")
        if "permissions" in job and (name, job_id) != ("book-pages.yml", "deploy"):
            fail("permissions", f"job {job_id} may not set its own permissions")
        for index, step in enumerate(steps_of(job), start=1):
            action = str(step.get("uses", ""))
            if action.lower().startswith("actions/checkout@"):
                options = step.get("with")
                if not isinstance(options, dict) or options.get("persist-credentials") is not False:
                    fail(
                        "checkout-token",
                        f"job {job_id} step {index}: actions/checkout must set"
                        " persist-credentials: false",
                    )
            if "run" in step and EVENT_CONTEXT.search(str(step["run"])):
                fail(
                    "event-in-run",
                    f"job {job_id} step {index}: a run block interpolates github.event;"
                    " pass the value through env",
                )
    return failures


def check_ci_ok(name: str, workflow: dict) -> list[str]:
    failures = []
    jobs = jobs_of(workflow)
    aggregate = jobs.get(CI_OK_JOB)
    if not isinstance(aggregate, dict) or aggregate.get("name") != "CI OK":
        return [f"{name}: ci-ok-needs: there is no `{CI_OK_JOB}` job named `CI OK`"]
    if aggregate.get("if") != "always()":
        failures.append(f"{name}: ci-ok-needs: the CI OK job must run with `if: always()`")
    needs = aggregate.get("needs")
    needs = [needs] if isinstance(needs, str) else needs or []
    missing = sorted(set(jobs) - {CI_OK_JOB} - set(needs))
    unknown = sorted(set(needs) - set(jobs))
    if missing:
        failures.append(f"{name}: ci-ok-needs: CI OK does not need these jobs: {missing}")
    if unknown or len(needs) != len(set(needs)):
        failures.append(f"{name}: ci-ok-needs: CI OK needs unknown or repeated jobs: {needs}")
    checks = [
        step for step in steps_of(aggregate)
        if "python3 .github/scripts/ci_ok.py" in str(step.get("run", ""))
        and isinstance(step.get("env"), dict)
        and "".join(str(step["env"].get("NEEDS_JSON", "")).split()) == "${{toJSON(needs)}}"
    ]
    if not checks:
        failures.append(
            f"{name}: ci-ok-needs: CI OK must run `python3 .github/scripts/ci_ok.py` with"
            " NEEDS_JSON set to ${{ toJSON(needs) }}"
        )
    for job_id, job in jobs.items():
        if not isinstance(job, dict):
            continue
        if job_id != CI_OK_JOB and "if" in job:
            failures.append(f"{name}: gate-if: job {job_id} is conditional; CI OK cannot tell a"
                            " filtered job from one that did not run")
        for index, step in enumerate(steps_of(job), start=1):
            if "if" in step:
                failures.append(f"{name}: gate-if: job {job_id} step {index} is conditional; a"
                                " skipped step leaves its job green")
    return failures


def check_pr_gate(name: str, workflow: dict) -> list[str]:
    failures = []
    on = triggers(workflow)
    if "push" in on:
        failures.append(f"{name}: pr-gate-triggers: the per-PR gate must not run on a push; a"
                        " merge is gated by release-gate.yml")
    if sorted(on) != ["pull_request", "workflow_dispatch"] or (
        branches(on.get("pull_request")) != ["development"]
    ):
        failures.append(f"{name}: pr-gate-triggers: triggers must be exactly pull_request into"
                        " development and workflow_dispatch")
    commands = run_text(workflow)
    for command in PR_GATE_COMMANDS:
        if command not in commands:
            failures.append(f"{name}: gate-commands: no step runs `{command}`")
    return failures


def check_release_gate(name: str, workflow: dict) -> list[str]:
    failures = []
    on = triggers(workflow)
    if not {"pull_request", "push", "schedule", "workflow_dispatch"} <= set(on):
        failures.append(f"{name}: release-triggers: triggers must include pull_request, push,"
                        " schedule and workflow_dispatch")
    if branches(on.get("pull_request")) != ["main"]:
        failures.append(f"{name}: release-triggers: pull_request must be limited to main")
    if branches(on.get("push")) != ["development", "main"]:
        failures.append(f"{name}: release-triggers: push must be limited to development and main")
    if "bash .agents/gate.sh full" not in run_text(workflow):
        failures.append(f"{name}: gate-commands: no step runs `bash .agents/gate.sh full`")
    for job_id, job in jobs_of(workflow).items():
        for index, step in enumerate(steps_of(job), start=1):
            action = str(step.get("uses", "")).lower()
            options = step.get("with") if isinstance(step.get("with"), dict) else {}
            if action.startswith(CACHING_ACTIONS) or "cache" in options:
                failures.append(f"{name}: gate-commands: job {job_id} step {index} restores a"
                                " cache; the release gate builds from nothing")
    return failures


def check(directory: Path) -> list[str]:
    """Return every rule failure for the workflows in `directory` (empty when all rules hold)."""
    # GitHub reads every .yml and .yaml file in the directory as a workflow, and nothing else.
    present = sorted(
        path.name for path in directory.iterdir() if path.suffix in (".yml", ".yaml")
    ) if directory.is_dir() else []
    failures = []
    if present != sorted(WORKFLOWS):
        failures.append(f"{directory}: workflow-set: expected exactly {sorted(WORKFLOWS)}, found"
                        f" {present}")
    for name in WORKFLOWS:
        path = directory / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        failures += check_text(name, text)
        try:
            workflow = yaml.safe_load(text)
        except yaml.YAMLError as error:
            failures.append(f"{name}: workflow-set: not valid YAML: {error}")
            continue
        if not isinstance(workflow, dict):
            failures.append(f"{name}: workflow-set: not a workflow mapping")
            continue
        failures += check_workflow(name, workflow)
        if name in GATES:
            failures += check_ci_ok(name, workflow)
        if name == "pr-gate.yml":
            failures += check_pr_gate(name, workflow)
        if name == "release-gate.yml":
            failures += check_release_gate(name, workflow)
    return failures


def main(argv: list[str]) -> int:
    root = Path(__file__).resolve().parents[2]
    directory = Path(argv[1]) if len(argv) > 1 else root / ".github" / "workflows"
    failures = check(directory)
    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        print(f"\n{len(failures)} workflow rule failure(s). Fix the workflow, not this check.")
        return 1
    print(f"OK: workflow rules hold for {', '.join(WORKFLOWS)}.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
