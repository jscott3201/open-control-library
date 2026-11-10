#!/usr/bin/env python3
"""Static rules for the workflows in .github/workflows and for .agents/gate.sh.

A workflow edit can turn the gate into a green light that proves less than it appears to. This
check fails on the edits that do that, before GitHub Actions runs them. Each rule has an id, and
test_check_workflow_gates.py holds at least one refused fixture per id.

  workflow-set      the directory holds exactly pr-gate.yml, release-gate.yml and book-pages.yml
  action-pin        every action is referenced by a full commit SHA, on a plain `uses:` line with
                    its tag in a trailing comment
  checkout-token    every actions/checkout step sets persist-credentials: false
  job-timeout       every job sets timeout-minutes, at most MAX_TIMEOUT_MINUTES
  runner            every job runs on the hosted ubuntu-latest runner, with no container and no
                    services
  shell             every run block uses the workflow's `bash` default (errexit and pipefail)
  no-soft-failure   no job and no step is allowed to fail without failing its workflow
  run-expression    no run block interpolates a ${{ }} expression (pass values through env)
  permissions       the workflow token is contents: read; only the Pages deploy job widens it, to
                    exactly pages: write and id-token: write
  pages-deploy      the Pages deploy job runs only for a push or a dispatch on main
  concurrency       one group per event and ref; only a superseded pull-request run is cancelled
  pr-gate-triggers  pr-gate.yml runs on exactly pull requests into development and dispatch
  release-triggers  release-gate.yml runs on exactly pushes to development and main, pull requests
                    into main, one daily schedule and dispatch
  release-checkout  release-gate.yml names the one commit resolved for the run, and no job or step
                    overrides it
  gate-checkout     every checkout of a gate workflow takes exactly the pinned inputs: the pull
                    request's own commit in pr-gate.yml, the run's one commit in release-gate.yml
  ci-ok-needs       each gate workflow ends in a CI OK job that always runs, needs every other
                    job and runs exactly ci_ok.py on their results
  gate-if           no job or step in a gate workflow is conditional, except CI OK's always()
  gate-commands     each gate workflow runs exactly the pinned command lines, job by job and in
                    order; release-gate.yml restores no cache
  gate-script       .agents/gate.sh runs exactly the pinned gate commands, under the pinned shell
                    options

The pinned command list is PR_GATE and RELEASE_GATE below: one source that pr-gate.yml,
release-gate.yml and .agents/gate.sh are each checked against. A gate command changes in
pr-gate.yml first, then in .agents/gate.sh, then here; until all three agree this check fails.

What the gate-script rule proves. The script's first command is `set -euo pipefail`, and no later
line changes the shell options. Every `step` and `engine_step` line holds one command and no
control operator. The commands on those lines are exactly the gate commands pr-gate.yml runs, and
the functions the other steps call are the pinned ones, line for line and operator for operator.
No other function is defined in the `name() {` form.

What it does not prove. The check reads the script; it does not run it or follow its control
flow, so it does not show that a step is reached or that its result counts. A final `exit 0`, a
redefined `step` or `record`, an `if false` around a step, a `trap`, a heredoc that swallows a
step, and a function written in another form (`function name {`, or on one line) that shadows a
command all pass.

Usage: python3 .github/scripts/check_workflow_gates.py [workflow-directory [gate-script]]
Needs PyYAML (pinned with the other gate requirements).
"""

from __future__ import annotations

import re
import shlex
import sys
from pathlib import Path

import yaml

WORKFLOWS = ("book-pages.yml", "pr-gate.yml", "release-gate.yml")
GATES = ("pr-gate.yml", "release-gate.yml")
CI_OK_JOB = "ci-ok"
RUNNER = "ubuntu-latest"
SHELL = "bash"
# The longest job today is the release gate's, at 45. GitHub's own default is 360.
MAX_TIMEOUT_MINUTES = 60
CANCEL_SUPERSEDED_PR = "${{ github.event_name == 'pull_request' }}"
# Written in two halves so that a grep for the key over .github finds real uses only.
SOFT_FAILURE_KEY = "continue-on-" + "error"
ACTION_LINE = re.compile(r"^\s*(?:-\s+)?uses:\s*(?P<ref>.*?)\s*$")
PINNED_REFERENCE = r"[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40}"
PINNED_ACTION_LINE = re.compile(rf"^{PINNED_REFERENCE} # \S+$")
PINNED_ACTION = re.compile(rf"^{PINNED_REFERENCE}$")
CACHING_ACTIONS = ("actions/cache", "swatinem/rust-cache")
DAILY_CRON = re.compile(r"^\d{1,2} \d{1,2} \* \* \*$")
# The commit a release-gate run tests. `github.sha` is resolved once, when the event fires: the
# pushed commit, the merge commit of a release pull request, the tip of the ref a run was
# dispatched on. Only the scheduled run names a branch, so that it follows `development` whatever
# the default branch is.
CHECKOUT_REF = "${{ github.event_name == 'schedule' && 'development' || github.sha }}"
CHECKOUT_REF_USE = "${{ env.CHECKOUT_REF }}"
DEPLOY_JOB = ("book-pages.yml", "deploy")
DEPLOY_PERMISSIONS = {"pages": "write", "id-token": "write"}
DEPLOY_IF = (
    "github.ref == 'refs/heads/main' &&"
    " (github.event_name == 'push' || github.event_name == 'workflow_dispatch')"
)

# ── The pinned gate ──────────────────────────────────────────────────────────────────────────
# Every command line a gate workflow runs, job by job and in order. A line is written as the
# workflow has it, with a backslash continuation joined and blanks folded to one space; a step's
# `env` is written as a NAME=value prefix on each of its lines.
INSTALL_PARSER = "python3 -m pip install PyYAML==6.0.3"
INSTALL_REQUIREMENTS = (
    "python3 -m pip install --requirement tools/lint/requirements-routine-schemas.txt"
    " PyYAML==6.0.3"
)
FETCH_SOURCES = "bash .github/scripts/pinned-sources.sh fetch"
TOOLCHAIN_CHANNEL = 'channel="$(bash .github/scripts/pinned-sources.sh toolchain)"'
INSTALL_TOOLCHAIN = 'rustup toolchain install "$channel" --profile minimal --no-self-update'
EXPORT_TOOLCHAIN = 'echo "RUSTUP_TOOLCHAIN=$channel" >> "$GITHUB_ENV"'
CI_OK_COMMAND = "NEEDS_JSON=${{ toJSON(needs) }} python3 .github/scripts/ci_ok.py"
CHECK_SOURCES = "bash .github/scripts/pinned-sources.sh check"
REPLAY_FAULT_LIBRARY = "cargo run --locked --manifest-path tools/verify/Cargo.toml -- --all"
REPLAY_DATASET = (
    "python3 tools/dataset_harness/harness.py replay"
    " --adapter lbl-fpu"
    " --dataset tools/dataset_harness/tests/fixtures/lbl_fpu_tiny"
    " --rules FPB-0001,FPB-0002,FPB-0003,FPB-0004,FPB-0005,FPB-0006"
    " --verifier tools/verify/target/debug/cxf-verify"
    " --output target/dataset-harness/ci-lbl-fpu-results.json"
)
ASSERT_DATASET = (
    "python3 -c 'import json;"
    ' p="target/dataset-harness/ci-lbl-fpu-results.json"; d=json.load(open(p));'
    ' assert d["schema"] == "cxf-library/dataset-validation/v1";'
    ' assert d["summary"]["fault_free_alarm_samples"] == 0;'
    ' assert d["summary"]["target_declared_rule_case_pairs"] == 1;'
    ' assert d["summary"]["target_detected_rule_case_pairs"] == 1\''
)
PR_GATE = {
    "workflow-rules": (
        INSTALL_PARSER,
        "python3 .github/scripts/check_workflow_gates.py",
        "python3 -m unittest discover -s .github/scripts -p 'test_*.py' -v",
    ),
    "no-secret-scan": (
        "bash .github/scripts/check-no-secrets.sh",
    ),
    "lint": (
        INSTALL_REQUIREMENTS,
        "python3 tools/lint/svg_fitment.py",
        "python3 tools/lint/registry.py",
        "python3 tools/lint/point_semantics.py",
        "python3 -m unittest discover -s tools/lint/tests -v",
        "python3 tools/lint/routine_schemas.py",
        "python3 tools/lint/routine_semantics.py",
        "python3 tools/lint/routines.py",
        "python3 tools/authoring/g36_reference.py --check",
        "python3 -m unittest discover -s tools/routines/tests -v",
        "python3 -m unittest discover -s tools/book/tests -v",
        "python3 -m unittest discover -s tools/dataset_harness/tests -v",
    ),
    "verify": (
        FETCH_SOURCES,
        CHECK_SOURCES,
        TOOLCHAIN_CHANNEL,
        INSTALL_TOOLCHAIN,
        EXPORT_TOOLCHAIN,
        INSTALL_REQUIREMENTS,
        "python3 tools/lint/g36_source.py --check"
        " --release-root ../modelica-buildings-release"
        " --development-root ../modelica-buildings-development",
        "cargo test --locked --manifest-path tools/routine-compiler/Cargo.toml",
        "OCL_G36_RELEASE_ROOT=../../../modelica-buildings-release"
        " OCL_G36_DEVELOPMENT_ROOT=../../../modelica-buildings-development"
        " cargo test --locked --manifest-path tools/routine-compiler/Cargo.toml"
        " --test g36_trim_and_respond -- --include-ignored",
        "cargo run --locked --manifest-path tools/verify/Cargo.toml --"
        " --g36-declarations ../modelica-buildings-release",
        "cargo run --locked --manifest-path tools/verify/Cargo.toml -- --routines",
        "python3 tools/routines/reference.py"
        " --verifier tools/verify/target/debug/cxf-verify"
        " --report target/g36-reference-results.json",
        "cargo test --locked --manifest-path tools/verify/Cargo.toml",
        REPLAY_FAULT_LIBRARY,
        REPLAY_DATASET,
        ASSERT_DATASET,
    ),
    "book": (
        "bash tools/book/install-mdbook.sh target/mdbook-bin",
        INSTALL_PARSER,
        "python3 tools/book/generate.py",
        "target/mdbook-bin/mdbook build book",
    ),
    CI_OK_JOB: (CI_OK_COMMAND,),
}
RELEASE_GATE = {
    "gate": (
        FETCH_SOURCES,
        TOOLCHAIN_CHANNEL,
        INSTALL_TOOLCHAIN,
        "bash .agents/gate.sh full",
    ),
    CI_OK_JOB: (CI_OK_COMMAND,),
}
PINNED_GATES = {"pr-gate.yml": PR_GATE, "release-gate.yml": RELEASE_GATE}
# Lines of pr-gate.yml that prepare the runner or report its result. .agents/gate.sh has its own
# form of each (SCRIPT_FUNCTIONS), or needs none: it installs no toolchain and fetches no source.
CI_ONLY = (
    INSTALL_PARSER,
    INSTALL_REQUIREMENTS,
    FETCH_SOURCES,
    INSTALL_TOOLCHAIN,
    EXPORT_TOOLCHAIN,
    CI_OK_COMMAND,
)
# The gate commands: what pr-gate.yml and .agents/gate.sh must both run.
GATE_COMMANDS = tuple(dict.fromkeys(
    command for commands in PR_GATE.values() for command in commands if command not in CI_ONLY
))

# ── The pinned gate script ───────────────────────────────────────────────────────────────────
# The first command of .agents/gate.sh. Without `pipefail` the replay piped into `tee` reports
# tee's status; without `errexit` and `nounset` a broken step function runs on.
SCRIPT_OPTIONS = "set -euo pipefail"
# A step of the script runs one gate command, alone on its line, or one of the functions in
# SCRIPT_STEP_FUNCTIONS. The body of each function below is pinned line for line, operators and
# keywords included, in the script's own form with `$VENV` and `$MDBOOK_DIR` replaced by their
# values. Five gate commands are run inside these bodies, not on a step line of their own.
SCRIPT_FUNCTIONS = {
    "clean_build_outputs": (
        "rm -rf target/gate-venv target/mdbook-bin tools/routine-compiler/target"
        " tools/verify/target",
    ),
    "python_environment": (
        "if [ ! -x target/gate-venv/bin/python3 ]; then",
        '"$GATE_PYTHON" -m venv target/gate-venv || return 1',
        "fi",
        "target/gate-venv/bin/python3 -m pip install --quiet --disable-pip-version-check"
        " --requirement tools/lint/requirements-routine-schemas.txt PyYAML==6.0.3",
    ),
    "select_engine_toolchain": (
        "local channel",
        TOOLCHAIN_CHANNEL + " || return 1",
        'if ! rustup run "$channel" rustc --version; then',
        'echo "the toolchain the pinned engine names is not installed:'
        ' rustup toolchain install $channel" >&2',
        "return 1",
        "fi",
        'export RUSTUP_TOOLCHAIN="$channel"',
    ),
    "pinned_sources": (
        CHECK_SOURCES + " && select_engine_toolchain && sources_ok=1",
    ),
    "fault_library": (
        REPLAY_FAULT_LIBRARY + " | tee target/gate/verify-all.log",
    ),
    "dataset_fixture_replay": (
        REPLAY_DATASET + " &&",
        ASSERT_DATASET,
    ),
}
SCRIPT_STEP_FUNCTIONS = (
    "clean_build_outputs",
    "python_environment",
    "pinned_sources",
    "fault_library",
    "dataset_fixture_replay",
)
# Functions the script also defines. Their bodies are not compared: they are its control flow.
SCRIPT_PLUMBING = ("record", "step", "engine_step", "observed_totals")

# ── Checkouts of the gate workflows ──────────────────────────────────────────────────────────
# The exact `with:` of every checkout. pr-gate.yml names no ref and no repository, so each job
# tests the pull request's own merge commit; release-gate.yml names the one commit of the run.
CHECKOUT_INPUTS = {
    "pr-gate.yml": {"persist-credentials": False},
    "release-gate.yml": {"ref": CHECKOUT_REF_USE, "persist-credentials": False},
}
# Jobs that check the library out one level down, beside the pinned sources.
CHECKOUT_PATH = {
    ("pr-gate.yml", "verify"): "open-control-library",
    ("release-gate.yml", "gate"): "open-control-library",
}

HEREDOC = re.compile(r"(?<!<)<<-?\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\1")
FUNCTION = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\(\) \{$")
STEP_CALL = re.compile(r"^(?:step|engine_step)\s")
STEP = re.compile(r"^(?:step|engine_step)\s+'[^']*'\s+(\S.*)$")
LITERAL_VARIABLE = re.compile(r"^([A-Z][A-Z0-9_]*)=([A-Za-z0-9_./-]+)$")
CONTROL_OPERATOR = re.compile(r"^[;&|]+$")
SHELL_OPTIONS = re.compile(r"(?<![\w./$-])(?:set\s+[-+]|shopt(?![\w-]))")


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
    return [step for step in steps if isinstance(step, dict)] if isinstance(steps, list) else []


def default_shell(mapping: object) -> object:
    """`defaults.run.shell` of a workflow or a job; None when it sets none."""
    defaults = mapping.get("defaults") if isinstance(mapping, dict) else None
    run = defaults.get("run") if isinstance(defaults, dict) else None
    return run.get("shell") if isinstance(run, dict) else None


def folded(text: object) -> str:
    """Runs of blanks folded to one space, so that a folded YAML scalar compares as one line."""
    return " ".join(str(text).split())


def run_lines(job: object) -> list[str]:
    """Every command line a job's run blocks hold, in order, in the form the pin is written in."""
    lines = []
    for step in steps_of(job):
        if "run" not in step:
            continue
        env = step.get("env", {})
        pairs = env.items() if isinstance(env, dict) else [("env", env)]
        prefix = "".join(f"{key}={value} " for key, value in pairs)
        for line in re.sub(r"\\\n", " ", str(step["run"])).splitlines():
            if line.strip():
                lines.append(prefix + folded(line))
    return lines


def words(line: str) -> tuple[str, ...]:
    """A shell line as words: quotes removed, operators and redirections as words of their own."""
    lexer = shlex.shlex(line, posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    lexer.commenters = ""
    return tuple(lexer)


def check_text(name: str, text: str, actions: list[str]) -> list[str]:
    failures = []
    plain = 0
    for number, line in enumerate(text.splitlines(), start=1):
        if line.lstrip().startswith("#"):
            continue
        match = ACTION_LINE.match(line)
        if match and PINNED_ACTION_LINE.match(match["ref"]):
            plain += 1
        elif match:
            failures.append(
                f"{name}: action-pin: line {number}: an action must be `owner/repo@<40-hex commit>"
                f" # <tag>`: {match['ref']}"
            )
    # The lines above are read as text, for the tag comment. A quoted key or a flow mapping is
    # not such a line, so the parsed references are checked too, and counted against the lines.
    for action in actions:
        if not PINNED_ACTION.match(action):
            failures.append(
                f"{name}: action-pin: an action must be pinned to a full commit SHA: {action}"
            )
    if len(actions) != plain:
        failures.append(
            f"{name}: action-pin: {len(actions)} actions are used, and {plain} are written as a"
            " plain `uses: owner/repo@<40-hex commit> # <tag>` line"
        )
    return failures


def actions_of(workflow: dict) -> list[str]:
    """Every `uses` reference of the parsed workflow: steps, and jobs that call a workflow."""
    actions = []
    for job in jobs_of(workflow).values():
        if isinstance(job, dict) and "uses" in job:
            actions.append(str(job["uses"]))
        actions += [str(step["uses"]) for step in steps_of(job) if "uses" in step]
    return actions


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

    if default_shell(workflow) != SHELL:
        fail("shell", f"defaults.run.shell must be `{SHELL}`: GitHub then runs every block with"
                      " errexit and pipefail")

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
        elif timeout > MAX_TIMEOUT_MINUTES:
            fail("job-timeout", f"job {job_id} sets timeout-minutes: {timeout}; the bound is"
                                f" {MAX_TIMEOUT_MINUTES}")
        if job.get("runs-on") != RUNNER:
            fail("runner", f"job {job_id} must run on `{RUNNER}`, a hosted runner:"
                           f" {job.get('runs-on')!r}")
        for key in ("container", "services"):
            if key in job:
                fail("runner", f"job {job_id} sets `{key}`; its commands must run on the"
                               " runner itself, beside nothing else")
        if SOFT_FAILURE_KEY in job:
            fail("no-soft-failure", f"job {job_id} is allowed to fail; a failing job must fail"
                                    " its workflow")
        if default_shell(job) not in (None, SHELL):
            fail("shell", f"job {job_id} sets its own default shell: {default_shell(job)!r}")
        if (name, job_id) == DEPLOY_JOB:
            if job.get("permissions") != DEPLOY_PERMISSIONS:
                fail("permissions", f"job {job_id} must hold exactly `pages: write` and"
                                    " `id-token: write`")
            if folded(job.get("if", "")) != DEPLOY_IF:
                fail("pages-deploy", f"job {job_id} must run only under `if: {DEPLOY_IF}`")
        elif "permissions" in job:
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
            if SOFT_FAILURE_KEY in step:
                fail("no-soft-failure", f"job {job_id} step {index} is allowed to fail; a"
                                        " failing step must fail its job")
            if step.get("shell", SHELL) != SHELL:
                fail("shell", f"job {job_id} step {index} sets its own shell:"
                              f" {step['shell']!r}")
            if "run" in step and "${{" in str(step["run"]):
                fail(
                    "run-expression",
                    f"job {job_id} step {index}: a run block interpolates a ${{{{ }}}}"
                    " expression; pass the value through env",
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


def difference(actual: list[str], pinned: list[str]) -> str:
    """What separates the command lines a job runs from the lines pinned for it."""
    for command in pinned:
        if command not in actual:
            return f"does not run `{command}`"
    for command in actual:
        if command not in pinned:
            return f"runs `{command}`, which the pin does not list"
    return "runs the pinned commands in another order, or one of them twice"


def check_commands(name: str, workflow: dict, pinned: dict) -> list[str]:
    """A gate workflow runs exactly the pinned command lines, job by job and in order."""
    failures = []
    jobs = jobs_of(workflow)
    for job_id in sorted(set(jobs) | set(pinned)):
        # CI OK's own command is the result check, so a change to it is a change to CI OK.
        rule = "ci-ok-needs" if job_id == CI_OK_JOB else "gate-commands"
        if job_id not in pinned:
            failures.append(f"{name}: {rule}: job {job_id} is not in the pinned gate")
        elif job_id not in jobs:
            failures.append(f"{name}: {rule}: pinned job {job_id} is missing")
        elif run_lines(jobs[job_id]) != list(pinned[job_id]):
            failures.append(f"{name}: {rule}: job {job_id} "
                            + difference(run_lines(jobs[job_id]), list(pinned[job_id])))
    return failures


def check_checkouts(name: str, workflow: dict) -> list[str]:
    """Every checkout of a gate workflow takes exactly the pinned inputs."""
    failures = []
    for job_id, job in jobs_of(workflow).items():
        pinned = dict(CHECKOUT_INPUTS[name])
        if (name, job_id) in CHECKOUT_PATH:
            pinned["path"] = CHECKOUT_PATH[(name, job_id)]
        for index, step in enumerate(steps_of(job), start=1):
            if not str(step.get("uses", "")).lower().startswith("actions/checkout@"):
                continue
            if step.get("with") != pinned:
                failures.append(
                    f"{name}: gate-checkout: job {job_id} step {index}: actions/checkout must"
                    f" take exactly these inputs: {pinned}; it has {step.get('with')}"
                )
    return failures


def check_pr_gate(name: str, workflow: dict) -> list[str]:
    failures = []
    on = triggers(workflow)
    if "push" in on:
        failures.append(f"{name}: pr-gate-triggers: the per-PR gate must not run on a push; a"
                        " merge is gated by release-gate.yml")
    if on != {"pull_request": {"branches": ["development"]}, "workflow_dispatch": None}:
        failures.append(f"{name}: pr-gate-triggers: triggers must be exactly pull_request into"
                        " development, with no other filter, and workflow_dispatch")
    return failures


def check_release_gate(name: str, workflow: dict) -> list[str]:
    failures = []

    def fail(rule: str, message: str) -> None:
        failures.append(f"{name}: {rule}: {message}")

    on = triggers(workflow)
    expected = {"pull_request", "push", "schedule", "workflow_dispatch"}
    if set(on) - expected:
        fail("release-triggers", f"triggers must not include {sorted(set(on) - expected)}")
    if expected - set(on):
        fail("release-triggers", f"triggers must include {sorted(expected - set(on))}")
    if "pull_request" in on and on["pull_request"] != {"branches": ["main"]}:
        fail("release-triggers", "pull_request must be exactly `branches: [main]`")
    push = on.get("push")
    if "push" in on and not (
        isinstance(push, dict) and set(push) == {"branches"}
        and branches(push) == ["development", "main"]
    ):
        fail("release-triggers", "push must be exactly `branches: [main, development]`")
    schedule = on.get("schedule")
    if "schedule" in on and not (
        isinstance(schedule, list) and len(schedule) == 1 and isinstance(schedule[0], dict)
        and set(schedule[0]) == {"cron"} and DAILY_CRON.match(str(schedule[0]["cron"]))
    ):
        fail("release-triggers", "schedule must be one cron entry that runs every day")
    if on.get("workflow_dispatch") is not None:
        fail("release-triggers", "workflow_dispatch must be written with no inputs and no value")

    env = workflow.get("env")
    if not isinstance(env, dict) or folded(env.get("CHECKOUT_REF", "")) != CHECKOUT_REF:
        fail("release-checkout", f"env.CHECKOUT_REF must be exactly {CHECKOUT_REF}")
    for job_id, job in jobs_of(workflow).items():
        if isinstance(job, dict) and "CHECKOUT_REF" in (job.get("env") or {}):
            fail("release-checkout", f"job {job_id} sets its own CHECKOUT_REF")
        for index, step in enumerate(steps_of(job), start=1):
            action = str(step.get("uses", "")).lower()
            options = step.get("with") if isinstance(step.get("with"), dict) else {}
            if "CHECKOUT_REF" in (step.get("env") or {}):
                fail("release-checkout", f"job {job_id} step {index} sets its own CHECKOUT_REF")
            if action.startswith(CACHING_ACTIONS) or "cache" in options:
                fail("gate-commands", f"job {job_id} step {index} restores a cache; the release"
                                      " gate builds from nothing")
    return failures


def logical_lines(text: str) -> list[str]:
    """Script lines without comment lines and heredoc bodies, backslash continuations joined."""
    lines, pending, closing = [], "", None
    for raw in text.splitlines():
        if closing is not None:
            closing = None if raw.strip() == closing else closing
            continue
        if not pending and raw.lstrip().startswith("#"):
            continue
        if raw.endswith("\\"):
            pending += raw[:-1] + " "
            continue
        line, pending = pending + raw, ""
        heredoc = HEREDOC.search(line)
        closing = heredoc[2] if heredoc else None
        lines.append(line)
    return lines


def read_script(text: str) -> tuple[list[str], dict[str, list[str]], list[str]]:
    """A gate script as its top-level lines, its functions and the names defined twice.

    A function is one written `name() {` on a line of its own and closed by `}` on a line of its
    own. Lines are stripped and blank ones dropped; the lines of a function are its body.
    """
    top: list[str] = []
    functions: dict[str, list[str]] = {}
    repeated, current = [], None
    for line in logical_lines(text):
        opening = FUNCTION.match(line) if current is None else None
        if opening:
            current = opening[1]
            if current in functions:
                repeated.append(current)
            functions[current] = []
        elif current is not None and line == "}":
            current = None
        elif line.strip():
            (top if current is None else functions[current]).append(line.strip())
    return top, functions, repeated


def literal_variables(text: str) -> dict[str, str]:
    """NAME to value, for each name the script assigns one literal value, once, at column 0."""
    variables = {}
    for line in logical_lines(text):
        literal = LITERAL_VARIABLE.match(line)
        if literal and len(re.findall(rf"\b{literal[1]}=", text)) == 1:
            variables[literal[1]] = literal[2]
    return variables


def embedded(command: str) -> bool:
    """Whether a gate command is run inside a pinned function body, not on a step line."""
    wanted = words(command)
    for body in SCRIPT_FUNCTIONS.values():
        for line in body:
            line_words = words(line)
            if any(line_words[at:at + len(wanted)] == wanted for at in range(len(line_words))):
                return True
    return False


def check_gate_script(path: Path) -> list[str]:
    """The gate script runs exactly the pinned gate commands, under the pinned shell options.

    Read from the script: its first command; every later line that changes the shell options;
    every `step` and `engine_step` line, which must hold a single-quoted label and one command
    with no control operator; and every function. `$NAME` is replaced where the script assigns
    NAME one literal value, and a leading `env` is dropped, before a line is compared as words.
    """
    name = f"{path.parent.name}/{path.name}"
    if not path.is_file():
        return [f"{name}: gate-script: the gate script is missing"]
    text = path.read_text(encoding="utf-8")
    failures = []

    def fail(message: str) -> None:
        failures.append(f"{name}: gate-script: {message}")

    variables = literal_variables(text)

    def line_words(line: str) -> tuple[str, ...] | None:
        try:
            read = words(line)
        except ValueError as error:
            fail(f"cannot read `{line}`: {error}")
            return None
        for variable, value in variables.items():
            pattern = rf"\$(?:{variable}(?![A-Za-z0-9_])|\{{{variable}\}})"
            read = tuple(re.sub(pattern, value, word) for word in read)
        return read

    top, functions, repeated = read_script(text)
    bodies = [line for body in functions.values() for line in body]
    if not top or top[0] != SCRIPT_OPTIONS:
        fail(f"the first command must be `{SCRIPT_OPTIONS}`")
    for line in top[1:] + bodies:
        if SHELL_OPTIONS.search(line):
            fail(f"`{line}` changes the shell options; only the first command,"
                 f" `{SCRIPT_OPTIONS}`, may set them")

    for function in repeated:
        fail(f"the function `{function}` is defined twice")
    for function in sorted(set(functions) - set(SCRIPT_FUNCTIONS) - set(SCRIPT_PLUMBING)):
        fail(f"the script defines a function `{function}` that the pin does not list")
    for function, pinned in SCRIPT_FUNCTIONS.items():
        if function not in functions:
            fail(f"the pinned function `{function}` is missing")
            continue
        body = [line_words(line) for line in functions[function]]
        if None in body:
            continue
        for at, (line, read) in enumerate(zip(functions[function], body), start=1):
            if at > len(pinned) or read != words(pinned[at - 1]):
                expected = f"`{pinned[at - 1]}`" if at <= len(pinned) else "no such line"
                fail(f"function `{function}` line {at} is `{line}`; the pin has {expected}")
                break
        else:
            if len(body) < len(pinned):
                fail(f"function `{function}` ends before the pinned line"
                     f" `{pinned[len(body)]}`")

    direct, called = set(), set()
    for line in top:
        if not STEP_CALL.match(line):
            continue
        step = STEP.match(line)
        if not step:
            fail(f"cannot read the step `{line}`: a step is `step '<label>' <command>`")
            continue
        command = line_words(step[1])
        if command is None:
            continue
        if any(CONTROL_OPERATOR.match(word) for word in command):
            fail(f"the step `{line}` holds a control operator; a step line is one command, so"
                 " that the result `step` records is that command's")
        elif command[0] == "env":
            direct.add(command[1:])
        elif command[0] not in functions:
            direct.add(command)
        elif len(command) > 1 or command[0] not in SCRIPT_STEP_FUNCTIONS:
            fail(f"the step `{line}` calls a function of the script that is not a pinned step"
                 " function, or calls one with arguments")
        else:
            called.add(command[0])
    for line in bodies:
        if STEP_CALL.match(line) and line != 'step "$@"':
            fail(f"the step `{line}` is inside a function; steps are top-level lines")

    pinned_direct = {words(command): command for command in GATE_COMMANDS
                     if not embedded(command)}
    for command, written in pinned_direct.items():
        if command not in direct:
            fail(f"no step runs the gate command `{written}`, which pr-gate.yml runs")
    for command in sorted(direct - set(pinned_direct)):
        fail(f"a step runs `{shlex.join(command)}`, which the pin does not list and"
             " pr-gate.yml does not run")
    for function in SCRIPT_STEP_FUNCTIONS:
        if function not in called:
            fail(f"no step runs the pinned step function `{function}`")
    return failures


def check(directory: Path, gate_script: Path) -> list[str]:
    """Return every rule failure for the workflows in `directory` and for the gate script."""
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
        try:
            workflow = yaml.safe_load(text)
        except yaml.YAMLError as error:
            failures.append(f"{name}: workflow-set: not valid YAML: {error}")
            continue
        if not isinstance(workflow, dict):
            failures.append(f"{name}: workflow-set: not a workflow mapping")
            continue
        failures += check_text(name, text, actions_of(workflow))
        failures += check_workflow(name, workflow)
        if name in GATES:
            failures += check_ci_ok(name, workflow)
            failures += check_commands(name, workflow, PINNED_GATES[name])
            failures += check_checkouts(name, workflow)
        if name == "pr-gate.yml":
            failures += check_pr_gate(name, workflow)
        if name == "release-gate.yml":
            failures += check_release_gate(name, workflow)
    return failures + check_gate_script(gate_script)


def main(argv: list[str]) -> int:
    root = Path(__file__).resolve().parents[2]
    directory = Path(argv[1]) if len(argv) > 1 else root / ".github" / "workflows"
    gate_script = Path(argv[2]) if len(argv) > 2 else root / ".agents" / "gate.sh"
    failures = check(directory, gate_script)
    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        print(f"\n{len(failures)} workflow rule failure(s). Fix the workflow, not this check.")
        return 1
    print(f"OK: workflow rules hold for {', '.join(WORKFLOWS)} and"
          f" {gate_script.parent.name}/{gate_script.name}.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
