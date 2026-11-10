"""Fixtures for check_workflow_gates.py: a passing fixture, and one refused edit per property.

The positive fixture is three small workflows and a small gate script that satisfy every rule.
The gate workflows and the script are generated from the checker's pins, so the fixture holds no
second copy of them. Each case in CASES applies one edit to the positive fixture and must be
refused under exactly the rule it names, so a rule that stops firing, or an edit that trips a
different rule than intended, fails here. ACCEPTED holds edits that stay inside a rule and must
pass. Run with `-v` to see each refusal:

    python3 -m unittest discover -s .github/scripts -p 'test_*.py' -v
"""

import re
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_workflow_gates as gates  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
PR, RELEASE, BOOK = "pr-gate.yml", "release-gate.yml", "book-pages.yml"
GATE_SCRIPT_NAME = "gate.sh"
SHA = "0123456789abcdef0123456789abcdef01234567"
USE_CHECKOUT = f"      - uses: actions/checkout@{SHA} # v6.1.0\n"
CHECKOUT = USE_CHECKOUT + "        with:\n          persist-credentials: false\n"
HEADER = """permissions:
  contents: read
concurrency:
  group: ${{ github.workflow }}-${{ github.event_name }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
defaults:
  run:
    shell: bash
"""
ENV_PREFIX = re.compile(r"^((?:[A-Z][A-Z0-9_]*=\S+ )+)(.*)$")


def run_step(command):
    """One step that runs one pinned line; a NAME=value prefix becomes the step's env."""
    prefix = ENV_PREFIX.match(command)
    if not prefix:
        return f"      - run: |\n          {command}\n"
    env = "".join(
        "          {}: {}\n".format(*pair.split("=", 1)) for pair in prefix[1].split()
    )
    return f"      - env:\n{env}        run: |\n          {prefix[2]}\n"


def checkout(name, job_id):
    """The checkout step of a gate job, with exactly the inputs pinned for it."""
    inputs = dict(gates.CHECKOUT_INPUTS[name])
    if (name, job_id) in gates.CHECKOUT_PATH:
        inputs["path"] = gates.CHECKOUT_PATH[(name, job_id)]
    written = {key: "false" if value is False else value for key, value in inputs.items()}
    return USE_CHECKOUT + "        with:\n" + "".join(
        f"          {key}: {value}\n" for key, value in written.items()
    )


def gate_job(name, job_id, lines, timeout):
    return (
        f"  {job_id}:\n    runs-on: ubuntu-latest\n    timeout-minutes: {timeout}\n"
        f"    steps:\n{checkout(name, job_id)}" + "".join(run_step(line) for line in lines)
    )


def gate_workflow(name, on, pinned, timeout):
    """A gate workflow that runs exactly `pinned`: one job per entry, then CI OK."""
    jobs = [job_id for job_id in pinned if job_id != gates.CI_OK_JOB]
    return (
        f"on:\n{on}{HEADER}jobs:\n"
        + "".join(gate_job(name, job_id, pinned[job_id], timeout) for job_id in jobs)
        + f"  ci-ok:\n    name: CI OK\n    if: always()\n    needs: [{', '.join(jobs)}]\n"
        f"    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n"
        f"{checkout(name, gates.CI_OK_JOB)}"
        "      - env:\n          NEEDS_JSON: ${{ toJSON(needs) }}\n"
        "        run: python3 .github/scripts/ci_ok.py\n"
    )


def pr_gate(pinned):
    on = "  pull_request:\n    branches: [development]\n  workflow_dispatch:\n"
    return gate_workflow(PR, on, pinned, 10)


def release_gate(pinned):
    on = (
        "  pull_request:\n    branches: [main]\n  push:\n    branches: [main, development]\n"
        '  schedule:\n    - cron: "41 9 * * *"\n  workflow_dispatch:\n'
        f"env:\n  CHECKOUT_REF: {gates.CHECKOUT_REF}\n"
    )
    return gate_workflow(RELEASE, on, pinned, 45)


def with_variables(line):
    """A pinned script line as .agents/gate.sh writes it, through its two path variables."""
    return line.replace("target/gate-venv", '"$VENV"').replace("target/mdbook-bin", '"$MDBOOK_DIR"')


DIRECT_COMMANDS = tuple(
    command for command in gates.GATE_COMMANDS if not gates.embedded(command)
)


def gate_script(commands=DIRECT_COMMANDS, functions=gates.SCRIPT_FUNCTIONS):
    """A gate script in the shape of .agents/gate.sh: the pinned functions, then one step each."""
    return (
        "#!/usr/bin/env bash\n# The gate.\nset -euo pipefail\n\n"
        "VENV=target/gate-venv\nMDBOOK_DIR=target/mdbook-bin\n"
        'record() {\n  echo "$@"\n}\n'
        'step() {\n  shift\n  if "$@"; then\n    record ok\n  fi\n}\n'
        'engine_step() {\n  step "$@"\n}\n'
        + "".join(
            f"{name}() {{\n" + "".join(f"  {with_variables(line)}\n" for line in body) + "}\n"
            for name, body in functions.items()
        )
        + "observed_totals() {\n  echo totals\n}\n"
        + "".join(f"step 'a step function' {name}\n" for name in gates.SCRIPT_STEP_FUNCTIONS)
        + "".join(f"step 'a gate command' {with_variables(line)}\n" for line in commands)
        + "observed_totals\ncat <<COVERAGE\nstep 'a heredoc line is not a step' true\nCOVERAGE\n"
    )


BOOK_PAGES = f"""on:
  pull_request:
    branches: [development]
  push:
    branches: [main]
  workflow_dispatch:
{HEADER}jobs:
  build:
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
{CHECKOUT}      - run: mdbook build book
      - if: github.ref == 'refs/heads/main'
        uses: actions/upload-pages-artifact@{SHA} # v4.0.0
  deploy:
    if: >-
      github.ref == 'refs/heads/main' &&
      (github.event_name == 'push' || github.event_name == 'workflow_dispatch')
    needs: build
    runs-on: ubuntu-latest
    timeout-minutes: 15
    permissions:
      pages: write
      id-token: write
    steps:
      - uses: actions/deploy-pages@{SHA} # v4.0.5
"""
POSITIVE = {
    PR: pr_gate(gates.PR_GATE),
    RELEASE: release_gate(gates.RELEASE_GATE),
    BOOK: BOOK_PAGES,
    GATE_SCRIPT_NAME: gate_script(),
}

# Text of the positive fixture that the edits below replace.
PR_JOBS = "workflow-rules, no-secret-scan, lint, verify, book"
PR_CI_OK = f"  ci-ok:\n    name: CI OK\n    if: always()\n    needs: [{PR_JOBS}]\n"
BOOK_JOB = gate_job(PR, "book", gates.PR_GATE["book"], 10)
LINT_JOB = "  lint:\n    runs-on: ubuntu-latest\n"
LINT_CHECKOUT = LINT_JOB + "    timeout-minutes: 10\n    steps:\n" + CHECKOUT
GATE_JOB = "  gate:\n    runs-on: ubuntu-latest\n"
GATE_USE_CHECKOUT = GATE_JOB + "    timeout-minutes: 45\n    steps:\n" + USE_CHECKOUT
GATE_CHECKOUT_REF = GATE_USE_CHECKOUT + "        with:\n          ref: ${{ env.CHECKOUT_REF }}\n"
REGISTRY_LINT = "      - run: |\n          python3 tools/lint/registry.py\n"
REGISTRY_LINT_RUN = "        run: |\n          python3 tools/lint/registry.py\n"
FAULT_LIBRARY = f"      - run: |\n          {gates.REPLAY_FAULT_LIBRARY}\n"
FULL_GATE = "      - run: |\n          bash .agents/gate.sh full\n"
RESULT_CHECK = "        run: python3 .github/scripts/ci_ok.py\n"
BOOK_CHECKOUT_TOKEN = "          persist-credentials: false\n      - run: mdbook build book\n"
DEPLOY_ACTION = f"      - uses: actions/deploy-pages@{SHA} # v4.0.5\n"
DEPLOY_IF = (
    "    if: >-\n      github.ref == 'refs/heads/main' &&\n"
    "      (github.event_name == 'push' || github.event_name == 'workflow_dispatch')\n"
)
BUILD_JOB = "  build:\n    runs-on: ubuntu-latest\n"
BUILD_TIMEOUT = "    timeout-minutes: 15\n"
ESCAPED_SOFT_FAILURE_KEY = '"continue-on-\\x65rror"'
SCRIPT_REGISTRY_LINT = "step 'a gate command' python3 tools/lint/registry.py\n"
SCRIPT_CARGO_TEST = "cargo test --locked --manifest-path tools/verify/Cargo.toml\n"
SCRIPT_FAULT_LIBRARY = f"  {gates.REPLAY_FAULT_LIBRARY} | tee target/gate/verify-all.log\n"
SCRIPT_PINNED_SOURCES = f"  {gates.CHECK_SOURCES} && select_engine_toolchain && sources_ok=1\n"
SCRIPT_REPLAY_THEN_ASSERT = "--output target/dataset-harness/ci-lbl-fpu-results.json &&\n"
SCRIPT_VENV = '  "$GATE_PYTHON" -m venv "$VENV" || return 1\n'
SCRIPT_TOOLCHAIN_CHECK = '  if ! rustup run "$channel" rustc --version; then\n'

# (rule, what the edit does, file, text to replace, replacement). A `None` text adds or removes the
# whole file: the replacement is its content, or None to delete it.
CASES = [
    ("workflow-set", "a fourth workflow file appears", "ci.yml", None, POSITIVE[PR]),
    ("workflow-set", "the book workflow is deleted", BOOK, None, None),
    ("action-pin", "an action is referenced by a movable tag", PR,
     LINT_CHECKOUT, LINT_CHECKOUT.replace(f"{SHA} # v6.1.0", "v6")),
    ("action-pin", "a pinned action loses its tag comment", BOOK,
     f"actions/deploy-pages@{SHA} # v4.0.5", f"actions/deploy-pages@{SHA}"),
    ("action-pin", "an action is referenced by a tag through a quoted key", BOOK,
     DEPLOY_ACTION, '      - "uses": actions/deploy-pages@v4\n'),
    ("action-pin", "an action is referenced by a tag through a flow mapping", BOOK,
     DEPLOY_ACTION, "      - {uses: actions/deploy-pages@v4}\n"),
    ("action-pin", "a pinned action is written as a flow mapping, without its tag line", BOOK,
     DEPLOY_ACTION, f"      - {{uses: actions/deploy-pages@{SHA}}}\n"),
    ("action-pin", "a tag through a quoted key, beside a pinned line that is only text", BOOK,
     DEPLOY_ACTION,
     '      - "uses": actions/deploy-pages@v4\n        with:\n          note: |\n'
     f"            uses: actions/deploy-pages@{SHA} # v4.0.5\n"),
    ("checkout-token", "a checkout keeps the job token", BOOK,
     BOOK_CHECKOUT_TOKEN, "          fetch-depth: 1\n      - run: mdbook build book\n"),
    ("job-timeout", "a job loses its timeout", BOOK, BUILD_JOB + BUILD_TIMEOUT, BUILD_JOB),
    ("job-timeout", "a job's timeout is past the bound", BOOK,
     BUILD_JOB + BUILD_TIMEOUT,
     BUILD_JOB + f"    timeout-minutes: {gates.MAX_TIMEOUT_MINUTES + 1}\n"),
    ("runner", "a job runs on a self-hosted runner", PR,
     LINT_JOB, "  lint:\n    runs-on: self-hosted\n"),
    ("runner", "a job selects a runner by labels", RELEASE,
     GATE_JOB, "  gate:\n    runs-on: [self-hosted, linux]\n"),
    ("runner", "a job runs its commands in a container", PR,
     LINT_JOB, LINT_JOB + "    container: alpine:3\n"),
    ("runner", "a job runs beside a service container", RELEASE,
     GATE_JOB, GATE_JOB + "    services:\n      cache:\n        image: redis\n"),
    ("shell", "a step's shell drops errexit and pipefail", PR,
     REGISTRY_LINT, "      - shell: bash {0}\n" + REGISTRY_LINT_RUN),
    ("shell", "the workflow sets no default shell", RELEASE,
     "defaults:\n  run:\n    shell: bash\n", ""),
    ("shell", "a job's default shell is not bash", BOOK,
     BUILD_JOB,
     "  build:\n    defaults:\n      run:\n        shell: sh\n    runs-on: ubuntu-latest\n"),
    ("no-soft-failure", "a step is allowed to fail", PR,
     REGISTRY_LINT, f"      - {gates.SOFT_FAILURE_KEY}: true\n" + REGISTRY_LINT_RUN),
    ("no-soft-failure", "a step is allowed to fail through an escaped key", PR,
     REGISTRY_LINT, f"      - {ESCAPED_SOFT_FAILURE_KEY}: true\n" + REGISTRY_LINT_RUN),
    ("no-soft-failure", "a step is allowed to fail through an aliased key", PR,
     REGISTRY_LINT,
     f"      - name: &key {ESCAPED_SOFT_FAILURE_KEY}\n        *key : true\n" + REGISTRY_LINT_RUN),
    ("no-soft-failure", "a job is allowed to fail", PR,
     LINT_JOB, LINT_JOB + f"    {gates.SOFT_FAILURE_KEY}: true\n"),
    ("no-soft-failure", "a job is allowed to fail through an escaped key", RELEASE,
     GATE_JOB, GATE_JOB + f"    {ESCAPED_SOFT_FAILURE_KEY}: true\n"),
    ("run-expression", "a run block interpolates the event payload", BOOK,
     "      - run: mdbook build book\n",
     '      - run: echo "${{ github.event.pull_request.title }}"\n'),
    ("run-expression", "a run block interpolates the head branch name", BOOK,
     "      - run: mdbook build book\n", '      - run: echo "${{ github.head_ref }}"\n'),
    ("run-expression", "a run block indexes the event context", BOOK,
     "      - run: mdbook build book\n",
     "      - run: echo \"${{ github['event']['pull_request']['title'] }}\"\n"),
    ("permissions", "the workflow token can write", PR,
     "permissions:\n  contents: read\n", "permissions:\n  contents: write\n"),
    ("permissions", "a gate job widens its own token", RELEASE,
     GATE_JOB, "  gate:\n    permissions:\n      contents: write\n    runs-on: ubuntu-latest\n"),
    ("permissions", "the deploy job can write to the repository", BOOK,
     "      pages: write\n      id-token: write\n",
     "      pages: write\n      id-token: write\n      contents: write\n"),
    ("pages-deploy", "the deploy job loses its condition", BOOK, DEPLOY_IF, ""),
    ("pages-deploy", "the deploy job runs for a pull request", BOOK,
     DEPLOY_IF, "    if: github.ref == 'refs/heads/main' || github.event_name == 'pull_request'\n"),
    ("concurrency", "a push run in progress can be cancelled", RELEASE,
     "cancel-in-progress: ${{ github.event_name == 'pull_request' }}",
     "cancel-in-progress: true"),
    ("concurrency", "every event shares one group", PR,
     "group: ${{ github.workflow }}-${{ github.event_name }}-${{ github.ref }}",
     "group: ${{ github.workflow }}"),
    ("pr-gate-triggers", "the per-PR gate gains a push trigger", PR,
     "  workflow_dispatch:\n", "  workflow_dispatch:\n  push:\n    branches: [development]\n"),
    ("pr-gate-triggers", "the per-PR gate runs for every base branch", PR,
     "  pull_request:\n    branches: [development]\n", "  pull_request:\n"),
    ("pr-gate-triggers", "the per-PR gate is filtered by path", PR,
     "    branches: [development]\n", "    branches: [development]\n    paths: [faults/**]\n"),
    ("release-triggers", "merges into development no longer run the release gate", RELEASE,
     "    branches: [main, development]\n", "    branches: [main]\n"),
    ("release-triggers", "the daily run is removed", RELEASE,
     '  schedule:\n    - cron: "41 9 * * *"\n', ""),
    ("release-triggers", "the daily run becomes a weekly one", RELEASE,
     '    - cron: "41 9 * * *"\n', '    - cron: "41 9 * * 1"\n'),
    ("release-triggers", "the release gate also runs on pull_request_target", RELEASE,
     "  workflow_dispatch:\n", "  workflow_dispatch:\n  pull_request_target:\n"),
    ("release-triggers", "the release gate also runs after another workflow", RELEASE,
     "  workflow_dispatch:\n",
     "  workflow_dispatch:\n  workflow_run:\n    workflows: [pr-gate]\n"),
    ("release-triggers", "release pull requests are filtered by path", RELEASE,
     "    branches: [main]\n", "    branches: [main]\n    paths: [faults/**]\n"),
    ("release-triggers", "a dispatched run takes inputs", RELEASE,
     "  workflow_dispatch:\n",
     "  workflow_dispatch:\n    inputs:\n      ref:\n        type: string\n"),
    ("release-checkout", "the scheduled run checks out main", RELEASE,
     "'schedule' && 'development'", "'schedule' && 'main'"),
    ("release-checkout", "a run checks out a ref that can move, not the resolved commit", RELEASE,
     "|| github.sha }}", "|| github.event_name == 'push' && github.sha || github.ref }}"),
    ("release-checkout", "a job overrides the commit to check out", RELEASE,
     GATE_JOB, "  gate:\n    env:\n      CHECKOUT_REF: main\n    runs-on: ubuntu-latest\n"),
    ("release-checkout", "a checkout step overrides the commit to check out", RELEASE,
     GATE_USE_CHECKOUT, GATE_USE_CHECKOUT + "        env:\n          CHECKOUT_REF: main\n"),
    ("gate-checkout", "a per-PR checkout takes the base branch, not the pull request", PR,
     LINT_CHECKOUT, LINT_CHECKOUT + "          ref: development\n"),
    ("gate-checkout", "a per-PR checkout takes another repository", PR,
     LINT_CHECKOUT, LINT_CHECKOUT + "          repository: octocat/hello\n"),
    ("gate-checkout", "a per-PR checkout is sparse", PR,
     LINT_CHECKOUT, LINT_CHECKOUT + "          sparse-checkout: tools\n"),
    ("gate-checkout", "a release-gate checkout takes another repository", RELEASE,
     GATE_CHECKOUT_REF, GATE_CHECKOUT_REF + "          repository: octocat/fork\n"),
    ("gate-checkout", "a release-gate checkout takes the default ref", RELEASE,
     GATE_CHECKOUT_REF, GATE_USE_CHECKOUT + "        with:\n"),
    ("ci-ok-needs", "a job is missing from CI OK's needs", PR,
     f"    needs: [{PR_JOBS}]\n", "    needs: [workflow-rules, no-secret-scan, lint, verify]\n"),
    ("ci-ok-needs", "CI OK no longer runs when a job fails", RELEASE, "    if: always()\n", ""),
    ("ci-ok-needs", "CI OK stops checking the results", PR,
     RESULT_CHECK, "        run: echo ok\n"),
    ("ci-ok-needs", "the result check is allowed to fail", PR,
     RESULT_CHECK, "        run: python3 .github/scripts/ci_ok.py || true\n"),
    ("ci-ok-needs", "the result check reads something other than the job results", RELEASE,
     "          NEEDS_JSON: ${{ toJSON(needs) }}\n",
     "          NEEDS_JSON: ${{ toJSON(needs.gate) }}\n"),
    ("gate-if", "a gate job is conditional", PR,
     "  book:\n    runs-on: ubuntu-latest\n",
     "  book:\n    if: github.event_name == 'pull_request'\n    runs-on: ubuntu-latest\n"),
    ("gate-if", "a gate step is conditional", RELEASE,
     FULL_GATE, "      - if: github.ref == 'refs/heads/main'\n        run: |\n"
                "          bash .agents/gate.sh full\n"),
    ("gate-commands", "the per-PR gate stops replaying the fault library", PR, FAULT_LIBRARY, ""),
    ("gate-commands", "a per-PR gate command is allowed to fail", PR,
     REGISTRY_LINT, "      - run: |\n          python3 tools/lint/registry.py || true\n"),
    ("gate-commands", "a per-PR gate command loses a flag", PR,
     FAULT_LIBRARY, FAULT_LIBRARY.replace(" --locked", "")),
    ("gate-commands", "a per-PR gate job ends before its commands", PR,
     REGISTRY_LINT, "      - run: |\n          exit 0\n" + REGISTRY_LINT),
    ("gate-commands", "the per-PR gate gains a job the pin does not list", PR,
     PR_CI_OK,
     f"  extra:\n    runs-on: ubuntu-latest\n    timeout-minutes: 10\n    steps:\n{CHECKOUT}"
     "      - run: |\n          python3 tools/lint/new_lint.py\n"
     + PR_CI_OK.replace(PR_JOBS, PR_JOBS + ", extra")),
    ("gate-commands", "the per-PR gate loses a pinned job", PR,
     BOOK_JOB + PR_CI_OK, PR_CI_OK.replace(", book", "")),
    ("gate-commands", "the release gate runs the light gate script", RELEASE,
     FULL_GATE, "      - run: |\n          bash .agents/gate.sh\n"),
    ("gate-commands", "the release gate's script run is allowed to fail", RELEASE,
     FULL_GATE, "      - run: |\n          bash .agents/gate.sh full || true\n"),
    ("gate-commands", "the release gate restores a cache", RELEASE,
     FULL_GATE,
     f"      - uses: actions/setup-python@{SHA} # v5.6.0\n        with:\n          cache: pip\n"
     + FULL_GATE),
    ("gate-script", "the gate script is missing", GATE_SCRIPT_NAME, None, None),
    ("gate-script", "the gate script holds no command", GATE_SCRIPT_NAME,
     None, "#!/usr/bin/env bash\n# The gate.\n"),
    ("gate-script", "the script runs without pipefail", GATE_SCRIPT_NAME,
     "set -euo pipefail\n", "set -eu\n"),
    ("gate-script", "the script's options are not its first command", GATE_SCRIPT_NAME,
     "set -euo pipefail\n\nVENV=target/gate-venv\n", "VENV=target/gate-venv\nset -euo pipefail\n"),
    ("gate-script", "a later line turns pipefail off", GATE_SCRIPT_NAME,
     "VENV=target/gate-venv\n", "VENV=target/gate-venv\nset +o pipefail\n"),
    ("gate-script", "a step function turns errexit off", GATE_SCRIPT_NAME,
     "fault_library() {\n", "fault_library() {\n  set +e\n"),
    ("gate-script", "a later line changes an option through shopt", GATE_SCRIPT_NAME,
     "VENV=target/gate-venv\n", "VENV=target/gate-venv\nshopt -u -o pipefail\n"),
    ("gate-script", "the script drops a command pr-gate.yml runs", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, ""),
    ("gate-script", "a step's command loses a flag", GATE_SCRIPT_NAME,
     SCRIPT_CARGO_TEST, SCRIPT_CARGO_TEST.replace(" --locked", "")),
    ("gate-script", "the script runs a command pr-gate.yml does not", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, SCRIPT_REGISTRY_LINT + "step 'a new lint' python3 tools/new_lint.py\n"),
    ("gate-script", "a gate script step is allowed to fail", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, "step 'a gate command' python3 tools/lint/registry.py || true\n"),
    ("gate-script", "a step's command sits behind an operator that never runs it",
     GATE_SCRIPT_NAME, SCRIPT_REGISTRY_LINT,
     "step 'a gate command' local channel || python3 tools/lint/registry.py\n"),
    ("gate-script", "a step is followed by another command on its line", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, "step 'a gate command' python3 tools/lint/registry.py; true\n"),
    ("gate-script", "a step runs in the background, where its result is lost", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, "step 'a gate command' python3 tools/lint/registry.py &\n"),
    ("gate-script", "a step has no label the check can read", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, 'step "$label" python3 tools/lint/registry.py\n'),
    ("gate-script", "a step's command has an unterminated quote", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, "step 'a gate command' python3 -c 'import json\n"),
    ("gate-script", "a step is moved into a function", GATE_SCRIPT_NAME,
     "observed_totals() {\n", "observed_totals() {\n  " + SCRIPT_REGISTRY_LINT),
    ("gate-script", "a path variable a step uses is assigned twice", GATE_SCRIPT_NAME,
     "MDBOOK_DIR=target/mdbook-bin\n", "MDBOOK_DIR=target/mdbook-bin\nMDBOOK_DIR=/usr/bin\n"),
    ("gate-script", "a step function is no longer called", GATE_SCRIPT_NAME,
     "step 'a step function' fault_library\n", ""),
    ("gate-script", "a step function is called with arguments", GATE_SCRIPT_NAME,
     "step 'a step function' fault_library\n", "step 'a step function' fault_library --dry\n"),
    ("gate-script", "a step calls a function that is not a step function", GATE_SCRIPT_NAME,
     "step 'a step function' fault_library\n",
     "step 'a step function' fault_library\nstep 'totals' observed_totals\n"),
    ("gate-script", "a step function returns before its commands", GATE_SCRIPT_NAME,
     "fault_library() {\n", "fault_library() {\n  return 0\n"),
    ("gate-script", "a step function's command is allowed to fail", GATE_SCRIPT_NAME,
     SCRIPT_FAULT_LIBRARY, SCRIPT_FAULT_LIBRARY.replace("\n", " || true\n")),
    ("gate-script", "a step function's command sits behind an operator that never runs it",
     GATE_SCRIPT_NAME, SCRIPT_FAULT_LIBRARY, "  sources_ok=1 ||" + SCRIPT_FAULT_LIBRARY[1:]),
    ("gate-script", "the replay's assertion runs only when the replay fails", GATE_SCRIPT_NAME,
     SCRIPT_REPLAY_THEN_ASSERT, SCRIPT_REPLAY_THEN_ASSERT.replace("&&", "||")),
    ("gate-script", "a failing pin check no longer stops the step", GATE_SCRIPT_NAME,
     SCRIPT_PINNED_SOURCES, SCRIPT_PINNED_SOURCES.replace(" &&", ";")),
    ("gate-script", "a failure branch is taken on success", GATE_SCRIPT_NAME,
     SCRIPT_VENV, SCRIPT_VENV.replace("||", "&&")),
    ("gate-script", "a negated test loses its negation", GATE_SCRIPT_NAME,
     SCRIPT_TOOLCHAIN_CHECK, SCRIPT_TOOLCHAIN_CHECK.replace("if ! ", "if ")),
    ("gate-script", "a step function loses a line of its own", GATE_SCRIPT_NAME,
     '  export RUSTUP_TOOLCHAIN="$channel"\n', ""),
    ("gate-script", "a step function gains a line", GATE_SCRIPT_NAME,
     '  export RUSTUP_TOOLCHAIN="$channel"\n', '  export RUSTUP_TOOLCHAIN="$channel"\n  true\n'),
    ("gate-script", "a step function's line has an unterminated quote", GATE_SCRIPT_NAME,
     "  local channel\n", "  local 'channel\n"),
    ("gate-script", "a pinned function is missing", GATE_SCRIPT_NAME,
     'select_engine_toolchain() {\n', 'toolchain() {\n'),
    ("gate-script", "a function takes the name of a command", GATE_SCRIPT_NAME,
     "observed_totals() {\n", "python3() {\n  true\n}\nobserved_totals() {\n"),
    ("gate-script", "a function is defined twice", GATE_SCRIPT_NAME,
     "observed_totals() {\n", "record() {\n  true\n}\nobserved_totals() {\n"),
]

# Refusals whose rule another refusal of the same rule would also report: what each must say.
MESSAGES = {
    "a gate script step is allowed to fail": "holds a control operator",
    "a step's command sits behind an operator that never runs it": "holds a control operator",
    "a step is followed by another command on its line": "holds a control operator",
    "a step runs in the background, where its result is lost": "holds a control operator",
}

# (rule, what the edit does, file, text to replace, replacement): edits that stay inside a rule.
ACCEPTED = [
    ("action-pin", "an action moves to a new commit with its tag comment", PR,
     LINT_CHECKOUT, LINT_CHECKOUT.replace(f"{SHA} # v6.1.0", f"{SHA[::-1]} # v6.2.0")),
    ("job-timeout", "a gate job's timeout is raised inside the bound", RELEASE,
     "    timeout-minutes: 45\n    steps:\n" + USE_CHECKOUT + "        with:\n"
     "          ref: ${{ env.CHECKOUT_REF }}\n          persist-credentials: false\n"
     "          path:",
     "    timeout-minutes: 55\n    steps:\n" + USE_CHECKOUT + "        with:\n"
     "          ref: ${{ env.CHECKOUT_REF }}\n          persist-credentials: false\n"
     "          path:"),
    ("job-timeout", "a job's timeout is at the bound", BOOK,
     BUILD_JOB + BUILD_TIMEOUT,
     BUILD_JOB + f"    timeout-minutes: {gates.MAX_TIMEOUT_MINUTES}\n"),
    ("release-triggers", "the daily run moves to another time of day", RELEASE,
     '    - cron: "41 9 * * *"\n', '    - cron: "7 23 * * *"\n'),
    ("release-triggers", "the pushed branches are listed in another order", RELEASE,
     "    branches: [main, development]\n", "    branches: [development, main]\n"),
    ("shell", "a step names the default shell", PR,
     REGISTRY_LINT, "      - shell: bash\n" + REGISTRY_LINT_RUN),
    ("ci-ok-needs", "CI OK's needs are listed in another order", PR,
     f"    needs: [{PR_JOBS}]\n",
     "    needs: [book, verify, lint, no-secret-scan, workflow-rules]\n"),
    ("gate-commands", "a pinned line is written with a continuation", PR,
     FAULT_LIBRARY, "      - run: |\n          cargo run --locked \\\n"
                    "            --manifest-path tools/verify/Cargo.toml -- --all\n"),
    ("gate-commands", "a run block has a blank line and its step a name", PR,
     REGISTRY_LINT, "      - name: Registry lint\n        run: |\n\n"
                    "          python3 tools/lint/registry.py\n"),
    ("gate-commands", "a comment is added to a gate workflow", PR,
     "jobs:\n", "# a note\njobs:\n"),
    ("gate-commands", "a gate job is given a display name", PR,
     LINT_JOB, "  lint:\n    name: Lint and unit tests\n    runs-on: ubuntu-latest\n"),
    ("gate-script", "a step's command is written with a continuation", GATE_SCRIPT_NAME,
     SCRIPT_CARGO_TEST,
     "\\\n  cargo test --locked \\\n  --manifest-path tools/verify/Cargo.toml\n"),
    ("gate-script", "a step's label is reworded", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, "step 'registry lint (points)' python3 tools/lint/registry.py\n"),
    ("gate-script", "a step function has a comment and a blank line", GATE_SCRIPT_NAME,
     "fault_library() {\n", "fault_library() {\n  # the log feeds the totals\n\n"),
    ("gate-script", "a path variable is written with braces", GATE_SCRIPT_NAME,
     '"$MDBOOK_DIR"/mdbook build book', '"${MDBOOK_DIR}/mdbook" build book'),
]

# The shapes .agents/gate.sh is written in, in a script small enough to read the result off.
READER_SCRIPT = """#!/usr/bin/env bash
# step 'a comment is not a step' true
set -euo pipefail

BIN=target/bin
step() {
  shift
  "$@"
}
replay() {
  # the log feeds the totals

  cargo run --locked -- --all | \\
    tee "$BIN/replay.log"
}
if [ "$MODE" = full ]; then
  step 'clean (full)' rm -rf "$BIN"
fi
engine_step 'integration' \\
  env ROOT=../release \\
  cargo test --locked
cat <<'COVERAGE'
step 'a heredoc line is not a step' true
replay() {
COVERAGE
"""
READER_TOP = [
    "set -euo pipefail",
    "BIN=target/bin",
    'if [ "$MODE" = full ]; then',
    "step 'clean (full)' rm -rf \"$BIN\"",
    "fi",
    "engine_step 'integration'    env ROOT=../release    cargo test --locked",
    "cat <<'COVERAGE'",
]
READER_FUNCTIONS = {
    "step": ["shift", '"$@"'],
    "replay": ['cargo run --locked -- --all |      tee "$BIN/replay.log"'],
}


def check_files(root, files):
    """Write `files` under `root` and return the checker's failures for them."""
    workflows = root / "workflows"
    workflows.mkdir()
    for name, text in files.items():
        target = root / name if name == GATE_SCRIPT_NAME else workflows / name
        target.write_text(text, encoding="utf-8")
    return gates.check(workflows, root / GATE_SCRIPT_NAME)


def edited(test, name, old, new):
    """The positive fixture with one edit applied."""
    files = dict(POSITIVE)
    if old is None and new is None:
        del files[name]
    elif old is None:
        files[name] = new
    else:
        test.assertEqual(files[name].count(old), 1, "the edit must apply exactly once")
        files[name] = files[name].replace(old, new)
    return files


def rule_of(failure):
    return failure.split(": ")[1]


class WorkflowRuleFixtures(unittest.TestCase):
    def test_the_positive_fixture_satisfies_every_rule(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(check_files(Path(tmp), POSITIVE), [])

    def test_the_repository_workflows_and_gate_script_satisfy_every_rule(self):
        self.assertEqual(
            gates.check(REPO / ".github" / "workflows", REPO / ".agents" / "gate.sh"), []
        )

    def test_every_rule_has_a_refused_fixture(self):
        documented = {
            line.split()[0]
            for line in gates.__doc__.splitlines()
            if line.startswith("  ") and not line.startswith("   ")
        }
        self.assertEqual({case[0] for case in CASES}, documented)

    def test_each_edit_is_refused_under_its_own_rule(self):
        for rule, what, name, old, new in CASES:
            with self.subTest(rule=rule, edit=what), tempfile.TemporaryDirectory() as tmp:
                failures = check_files(Path(tmp), edited(self, name, old, new))
                self.assertTrue(failures, f"{what}: accepted")
                self.assertEqual({rule_of(failure) for failure in failures}, {rule}, failures)
                self.assertIn(MESSAGES.get(what, ""), failures[0])
                shown = failures[0].replace(str(Path(tmp)), "<fixture>")
                shown = shown.replace(Path(tmp).name, "<fixture>")
                print(f"\n  refused [{rule}] {what}\n    {shown}", end="", file=sys.stderr)

    def test_every_expected_message_belongs_to_one_fixture(self):
        described = [case[1] for case in CASES]
        self.assertEqual(len(described), len(set(described)), "two fixtures share a description")
        self.assertLessEqual(set(MESSAGES), set(described))

    def test_each_edit_inside_a_rule_is_accepted(self):
        for rule, what, name, old, new in ACCEPTED:
            with self.subTest(rule=rule, edit=what), tempfile.TemporaryDirectory() as tmp:
                self.assertEqual(check_files(Path(tmp), edited(self, name, old, new)), [])


class PinnedGateCommands(unittest.TestCase):
    """No pinned line can leave a gate workflow or the gate script, whichever line it is."""

    def test_a_gate_workflow_without_any_one_pinned_line_is_refused(self):
        builders = {PR: pr_gate, RELEASE: release_gate}
        for name, pinned in gates.PINNED_GATES.items():
            for job_id, lines in pinned.items():
                if job_id == gates.CI_OK_JOB:
                    continue
                for index, line in enumerate(lines):
                    with self.subTest(workflow=name, job=job_id, line=line), \
                            tempfile.TemporaryDirectory() as tmp:
                        without = {**pinned, job_id: lines[:index] + lines[index + 1:]}
                        files = {**POSITIVE, name: builders[name](without)}
                        failures = check_files(Path(tmp), files)
                        self.assertEqual([rule_of(failure) for failure in failures],
                                         ["gate-commands"], failures)
                        self.assertIn(f"job {job_id} does not run `{line}`", failures[0])

    def test_a_gate_script_without_any_one_step_command_is_refused(self):
        for index, line in enumerate(DIRECT_COMMANDS):
            with self.subTest(line=line), tempfile.TemporaryDirectory() as tmp:
                without = DIRECT_COMMANDS[:index] + DIRECT_COMMANDS[index + 1:]
                files = {**POSITIVE, GATE_SCRIPT_NAME: gate_script(commands=without)}
                failures = check_files(Path(tmp), files)
                self.assertEqual([rule_of(failure) for failure in failures], ["gate-script"],
                                 failures)
                self.assertIn(f"no step runs the gate command `{line}`", failures[0])

    def test_a_gate_script_without_any_one_command_of_a_step_function_is_refused(self):
        inside = [command for command in gates.GATE_COMMANDS if gates.embedded(command)]
        self.assertEqual(inside, [
            gates.CHECK_SOURCES, gates.TOOLCHAIN_CHANNEL, gates.REPLAY_FAULT_LIBRARY,
            gates.REPLAY_DATASET, gates.ASSERT_DATASET,
        ])
        for command in inside:
            with self.subTest(command=command), tempfile.TemporaryDirectory() as tmp:
                functions = {
                    name: tuple(line.replace(command, "true") for line in body)
                    for name, body in gates.SCRIPT_FUNCTIONS.items()
                }
                self.assertNotEqual(functions, gates.SCRIPT_FUNCTIONS)
                files = {**POSITIVE, GATE_SCRIPT_NAME: gate_script(functions=functions)}
                failures = check_files(Path(tmp), files)
                self.assertEqual([rule_of(failure) for failure in failures], ["gate-script"],
                                 failures)
                self.assertIn(f"the pin has `{command}", failures[0])

    def test_the_pin_separates_gate_commands_from_runner_preparation(self):
        pr_gate_lines = {line for lines in gates.PR_GATE.values() for line in lines}
        release_lines = {line for lines in gates.RELEASE_GATE.values() for line in lines}
        self.assertLessEqual(set(gates.CI_ONLY), pr_gate_lines)
        self.assertEqual(set(gates.GATE_COMMANDS), pr_gate_lines - set(gates.CI_ONLY))
        # The release gate runs the gate script and nothing else that is not runner preparation.
        self.assertEqual(release_lines - set(gates.CI_ONLY) - set(gates.GATE_COMMANDS),
                         {"bash .agents/gate.sh full"})

    def test_every_step_function_is_pinned_and_none_is_plumbing(self):
        self.assertLessEqual(set(gates.SCRIPT_STEP_FUNCTIONS), set(gates.SCRIPT_FUNCTIONS))
        self.assertFalse(set(gates.SCRIPT_FUNCTIONS) & set(gates.SCRIPT_PLUMBING))


class GateScriptReader(unittest.TestCase):
    def test_lines_and_functions_of_a_script_are_read(self):
        top, functions, repeated = gates.read_script(READER_SCRIPT)
        self.assertEqual(top, READER_TOP)
        self.assertEqual(functions, READER_FUNCTIONS)
        self.assertEqual(repeated, [])
        self.assertEqual(gates.literal_variables(READER_SCRIPT), {"BIN": "target/bin"})

    def test_a_variable_assigned_twice_has_no_one_value(self):
        self.assertEqual(gates.literal_variables(READER_SCRIPT + "BIN=elsewhere\n"), {})

    def test_a_function_defined_twice_is_reported(self):
        _, functions, repeated = gates.read_script(READER_SCRIPT + "step() {\n  true\n}\n")
        self.assertEqual(repeated, ["step"])
        self.assertEqual(functions["step"], ["true"])


if __name__ == "__main__":
    unittest.main()
