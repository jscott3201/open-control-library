"""Fixtures for check_workflow_gates.py: a passing fixture, and one refused edit per property.

The positive fixture is three small workflows and a small gate script that satisfy every rule.
The gate workflows and the script are generated from the checker's pinned command list, so the
fixture holds no second copy of it. Each case in CASES applies one edit to the positive fixture
and must be refused under exactly the rule it names, so a rule that stops firing, or an edit that
trips a different rule than intended, fails here. ACCEPTED holds edits that stay inside a rule and
must pass. Run with `-v` to see each refusal:

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
GATE_SCRIPT_NAME = "gate.sh"
SHA = "0123456789abcdef0123456789abcdef01234567"
CHECKOUT = f"""      - uses: actions/checkout@{SHA} # v6.1.0
        with:
          persist-credentials: false
"""
RELEASE_CHECKOUT = f"""      - uses: actions/checkout@{SHA} # v6.1.0
        with:
          ref: ${{{{ env.CHECKOUT_REF }}}}
          persist-credentials: false
"""
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


def gate_workflow(on, pinned, checkout, timeout):
    """A gate workflow that runs exactly `pinned`: one job per entry, then CI OK."""
    jobs = [job_id for job_id in pinned if job_id != gates.CI_OK_JOB]
    text = f"on:\n{on}{HEADER}jobs:\n"
    for job_id in jobs:
        text += (
            f"  {job_id}:\n    runs-on: ubuntu-latest\n    timeout-minutes: {timeout}\n"
            f"    steps:\n{checkout}" + "".join(run_step(line) for line in pinned[job_id])
        )
    return text + (
        f"  ci-ok:\n    name: CI OK\n    if: always()\n    needs: [{', '.join(jobs)}]\n"
        f"    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n{checkout}"
        "      - env:\n          NEEDS_JSON: ${{ toJSON(needs) }}\n"
        "        run: python3 .github/scripts/ci_ok.py\n"
    )


def pr_gate(pinned):
    on = "  pull_request:\n    branches: [development]\n  workflow_dispatch:\n"
    return gate_workflow(on, pinned, CHECKOUT, 10)


def release_gate(pinned):
    on = (
        "  pull_request:\n    branches: [main]\n  push:\n    branches: [main, development]\n"
        '  schedule:\n    - cron: "41 9 * * *"\n  workflow_dispatch:\n'
        f"env:\n  CHECKOUT_REF: {gates.CHECKOUT_REF}\n"
    )
    return gate_workflow(on, pinned, RELEASE_CHECKOUT, 45)


def gate_script(commands):
    """A gate script whose steps run `commands`, and the script's own commands in a function."""
    return (
        "#!/usr/bin/env bash\nset -euo pipefail\n"
        'step() {\n  shift\n  "$@"\n}\n'
        "prepare() {\n" + "".join(f"  {line}\n" for line in gates.SCRIPT_ONLY) + "}\n"
        "step 'the commands only the script runs' prepare\n"
        + "".join(f"step 'a gate command' {line}\n" for line in commands)
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
    "pr-gate.yml": pr_gate(gates.PR_GATE),
    "release-gate.yml": release_gate(gates.RELEASE_GATE),
    "book-pages.yml": BOOK_PAGES,
    GATE_SCRIPT_NAME: gate_script(gates.GATE_COMMANDS),
}

PR_JOBS = "workflow-rules, no-secret-scan, lint, verify, book"
PR_CI_OK = f"  ci-ok:\n    name: CI OK\n    if: always()\n    needs: [{PR_JOBS}]\n"
REGISTRY_LINT = "      - run: |\n          python3 tools/lint/registry.py\n"
FAULT_LIBRARY = (
    "      - run: |\n"
    "          cargo run --locked --manifest-path tools/verify/Cargo.toml -- --all\n"
)
FULL_GATE = "      - run: |\n          bash .agents/gate.sh full\n"
RESULT_CHECK = "        run: python3 .github/scripts/ci_ok.py\n"
FIRST_PR_CHECKOUT = (
    "  workflow-rules:\n    runs-on: ubuntu-latest\n    timeout-minutes: 10\n    steps:\n"
    f"      - uses: actions/checkout@{SHA} # v6.1.0"
)
GATE_CHECKOUT = (
    "    timeout-minutes: 45\n    steps:\n"
    f"      - uses: actions/checkout@{SHA} # v6.1.0\n        with:\n"
    "          ref: ${{ env.CHECKOUT_REF }}\n"
)
DEPLOY_IF = (
    "    if: >-\n      github.ref == 'refs/heads/main' &&\n"
    "      (github.event_name == 'push' || github.event_name == 'workflow_dispatch')\n"
)
BUILD_JOB = "  build:\n    runs-on: ubuntu-latest\n"
BUILD_TIMEOUT = "    timeout-minutes: 15\n"
SCRIPT_REGISTRY_LINT = "python3 tools/lint/registry.py\n"
SCRIPT_FAULT_LIBRARY = "cargo run --locked --manifest-path tools/verify/Cargo.toml -- --all\n"

# (rule, what the edit does, file, text to replace, replacement). A `None` text adds or removes the
# whole file: the replacement is its content, or None to delete it.
CASES = [
    ("workflow-set", "a fourth workflow file appears", "ci.yml", None, POSITIVE["pr-gate.yml"]),
    ("workflow-set", "the book workflow is deleted", "book-pages.yml", None, None),
    ("action-pin", "an action is referenced by a movable tag", "pr-gate.yml",
     FIRST_PR_CHECKOUT, FIRST_PR_CHECKOUT.replace(f"{SHA} # v6.1.0", "v6")),
    ("action-pin", "a pinned action loses its tag comment", "book-pages.yml",
     f"actions/deploy-pages@{SHA} # v4.0.5", f"actions/deploy-pages@{SHA}"),
    ("action-pin", "an action is referenced by a tag through a quoted key", "book-pages.yml",
     f"      - uses: actions/deploy-pages@{SHA} # v4.0.5\n",
     '      - "uses": actions/deploy-pages@v4\n'),
    ("action-pin", "an action is referenced by a tag through a flow mapping", "book-pages.yml",
     f"      - uses: actions/deploy-pages@{SHA} # v4.0.5\n",
     "      - {uses: actions/deploy-pages@v4}\n"),
    ("action-pin", "a pinned action is written as a flow mapping, without its tag line",
     "book-pages.yml", f"      - uses: actions/deploy-pages@{SHA} # v4.0.5\n",
     f"      - {{uses: actions/deploy-pages@{SHA}}}\n"),
    ("checkout-token", "a checkout keeps the job token", "release-gate.yml",
     GATE_CHECKOUT + "          persist-credentials: false\n", GATE_CHECKOUT),
    ("job-timeout", "a job loses its timeout", "book-pages.yml",
     BUILD_JOB + BUILD_TIMEOUT,
     BUILD_JOB),
    ("job-timeout", "a job's timeout is past the bound", "book-pages.yml",
     BUILD_JOB + BUILD_TIMEOUT,
     BUILD_JOB + f"    timeout-minutes: {gates.MAX_TIMEOUT_MINUTES + 1}\n"),
    ("runner", "a job runs on a self-hosted runner", "pr-gate.yml",
     "  lint:\n    runs-on: ubuntu-latest\n", "  lint:\n    runs-on: self-hosted\n"),
    ("runner", "a job selects a runner by labels", "release-gate.yml",
     "  gate:\n    runs-on: ubuntu-latest\n", "  gate:\n    runs-on: [self-hosted, linux]\n"),
    ("shell", "a step's shell drops errexit and pipefail", "pr-gate.yml",
     REGISTRY_LINT, "      - shell: bash {0}\n        run: |\n"
                    "          python3 tools/lint/registry.py\n"),
    ("shell", "the workflow sets no default shell", "release-gate.yml",
     "defaults:\n  run:\n    shell: bash\n", ""),
    ("shell", "a job's default shell is not bash", "book-pages.yml",
     BUILD_JOB,
     "  build:\n    defaults:\n      run:\n        shell: sh\n    runs-on: ubuntu-latest\n"),
    ("no-soft-failure", "a step is allowed to fail", "pr-gate.yml",
     REGISTRY_LINT, f"      - {gates.SOFT_FAILURE_KEY}: true\n        run: |\n"
                    "          python3 tools/lint/registry.py\n"),
    ("run-expression", "a run block interpolates the event payload", "book-pages.yml",
     "      - run: mdbook build book\n",
     '      - run: echo "${{ github.event.pull_request.title }}"\n'),
    ("run-expression", "a run block interpolates the head branch name", "book-pages.yml",
     "      - run: mdbook build book\n", '      - run: echo "${{ github.head_ref }}"\n'),
    ("run-expression", "a run block indexes the event context", "book-pages.yml",
     "      - run: mdbook build book\n",
     "      - run: echo \"${{ github['event']['pull_request']['title'] }}\"\n"),
    ("permissions", "the workflow token can write", "pr-gate.yml",
     "permissions:\n  contents: read\n", "permissions:\n  contents: write\n"),
    ("permissions", "a gate job widens its own token", "release-gate.yml",
     "  gate:\n    runs-on: ubuntu-latest\n",
     "  gate:\n    permissions:\n      contents: write\n    runs-on: ubuntu-latest\n"),
    ("permissions", "the deploy job can write to the repository", "book-pages.yml",
     "      pages: write\n      id-token: write\n",
     "      pages: write\n      id-token: write\n      contents: write\n"),
    ("pages-deploy", "the deploy job loses its condition", "book-pages.yml", DEPLOY_IF, ""),
    ("pages-deploy", "the deploy job runs for a pull request", "book-pages.yml",
     DEPLOY_IF, "    if: github.ref == 'refs/heads/main' || github.event_name == 'pull_request'\n"),
    ("concurrency", "a push run in progress can be cancelled", "release-gate.yml",
     "cancel-in-progress: ${{ github.event_name == 'pull_request' }}",
     "cancel-in-progress: true"),
    ("concurrency", "every event shares one group", "pr-gate.yml",
     "group: ${{ github.workflow }}-${{ github.event_name }}-${{ github.ref }}",
     "group: ${{ github.workflow }}"),
    ("pr-gate-triggers", "the per-PR gate gains a push trigger", "pr-gate.yml",
     "  workflow_dispatch:\n", "  workflow_dispatch:\n  push:\n    branches: [development]\n"),
    ("pr-gate-triggers", "the per-PR gate runs for every base branch", "pr-gate.yml",
     "  pull_request:\n    branches: [development]\n", "  pull_request:\n"),
    ("pr-gate-triggers", "the per-PR gate is filtered by path", "pr-gate.yml",
     "    branches: [development]\n", "    branches: [development]\n    paths: [faults/**]\n"),
    ("release-triggers", "merges into development no longer run the release gate",
     "release-gate.yml", "    branches: [main, development]\n", "    branches: [main]\n"),
    ("release-triggers", "the daily run is removed", "release-gate.yml",
     '  schedule:\n    - cron: "41 9 * * *"\n', ""),
    ("release-triggers", "the daily run becomes a weekly one", "release-gate.yml",
     '    - cron: "41 9 * * *"\n', '    - cron: "41 9 * * 1"\n'),
    ("release-triggers", "the release gate also runs on pull_request_target", "release-gate.yml",
     "  workflow_dispatch:\n", "  workflow_dispatch:\n  pull_request_target:\n"),
    ("release-triggers", "the release gate also runs after another workflow", "release-gate.yml",
     "  workflow_dispatch:\n",
     "  workflow_dispatch:\n  workflow_run:\n    workflows: [pr-gate]\n"),
    ("release-triggers", "release pull requests are filtered by path", "release-gate.yml",
     "    branches: [main]\n", "    branches: [main]\n    paths: [faults/**]\n"),
    ("release-checkout", "the scheduled run checks out main", "release-gate.yml",
     "'schedule' && 'development'", "'schedule' && 'main'"),
    ("release-checkout", "a run checks out a ref that can move, not the resolved commit",
     "release-gate.yml", "|| github.sha }}",
     "|| github.event_name == 'push' && github.sha || github.ref }}"),
    ("release-checkout", "a checkout takes the default ref", "release-gate.yml",
     GATE_CHECKOUT, GATE_CHECKOUT.replace("          ref: ${{ env.CHECKOUT_REF }}\n", "")),
    ("release-checkout", "a job overrides the commit to check out", "release-gate.yml",
     "  gate:\n    runs-on: ubuntu-latest\n",
     "  gate:\n    env:\n      CHECKOUT_REF: main\n    runs-on: ubuntu-latest\n"),
    ("ci-ok-needs", "a job is missing from CI OK's needs", "pr-gate.yml",
     f"    needs: [{PR_JOBS}]\n", "    needs: [workflow-rules, no-secret-scan, lint, verify]\n"),
    ("ci-ok-needs", "CI OK no longer runs when a job fails", "release-gate.yml",
     "    if: always()\n", ""),
    ("ci-ok-needs", "CI OK stops checking the results", "pr-gate.yml",
     RESULT_CHECK, "        run: echo ok\n"),
    ("ci-ok-needs", "the result check is allowed to fail", "pr-gate.yml",
     RESULT_CHECK, "        run: python3 .github/scripts/ci_ok.py || true\n"),
    ("ci-ok-needs", "the result check reads something other than the job results",
     "release-gate.yml", "          NEEDS_JSON: ${{ toJSON(needs) }}\n",
     "          NEEDS_JSON: ${{ toJSON(needs.gate) }}\n"),
    ("gate-if", "a gate job is conditional", "pr-gate.yml",
     "  book:\n    runs-on: ubuntu-latest\n",
     "  book:\n    if: github.event_name == 'pull_request'\n    runs-on: ubuntu-latest\n"),
    ("gate-if", "a gate step is conditional", "release-gate.yml",
     FULL_GATE, "      - if: github.ref == 'refs/heads/main'\n        run: |\n"
                "          bash .agents/gate.sh full\n"),
    ("gate-commands", "the per-PR gate stops replaying the fault library", "pr-gate.yml",
     FAULT_LIBRARY, ""),
    ("gate-commands", "a per-PR gate command is allowed to fail", "pr-gate.yml",
     REGISTRY_LINT, "      - run: |\n          python3 tools/lint/registry.py || true\n"),
    ("gate-commands", "a per-PR gate command loses a flag", "pr-gate.yml",
     FAULT_LIBRARY,
     "      - run: |\n          cargo run --manifest-path tools/verify/Cargo.toml -- --all\n"),
    ("gate-commands", "a per-PR gate job ends before its commands", "pr-gate.yml",
     REGISTRY_LINT, "      - run: |\n          exit 0\n" + REGISTRY_LINT),
    ("gate-commands", "the per-PR gate gains a job the pin does not list", "pr-gate.yml",
     PR_CI_OK,
     f"  extra:\n    runs-on: ubuntu-latest\n    timeout-minutes: 10\n    steps:\n{CHECKOUT}"
     "      - run: |\n          python3 tools/lint/new_lint.py\n"
     + PR_CI_OK.replace(PR_JOBS, PR_JOBS + ", extra")),
    ("gate-commands", "the release gate runs the light gate script", "release-gate.yml",
     FULL_GATE, "      - run: |\n          bash .agents/gate.sh\n"),
    ("gate-commands", "the release gate's script run is allowed to fail", "release-gate.yml",
     FULL_GATE, "      - run: |\n          bash .agents/gate.sh full || true\n"),
    ("gate-commands", "the release gate restores a cache", "release-gate.yml",
     "  gate:\n    runs-on: ubuntu-latest\n    timeout-minutes: 45\n    steps:\n",
     "  gate:\n    runs-on: ubuntu-latest\n    timeout-minutes: 45\n    steps:\n"
     f"      - uses: actions/setup-python@{SHA} # v5.6.0\n        with:\n          cache: pip\n"),
    ("gate-script", "the gate script is missing", GATE_SCRIPT_NAME, None, None),
    ("gate-script", "the gate script drops a command pr-gate.yml runs", GATE_SCRIPT_NAME,
     f"step 'a gate command' {SCRIPT_REGISTRY_LINT}", ""),
    ("gate-script", "a gate script command loses a flag", GATE_SCRIPT_NAME,
     SCRIPT_FAULT_LIBRARY, "cargo run --manifest-path tools/verify/Cargo.toml -- --all\n"),
    ("gate-script", "a gate script step is allowed to fail", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, "python3 tools/lint/registry.py || true\n"),
    ("gate-script", "the gate script runs a command pr-gate.yml does not", GATE_SCRIPT_NAME,
     f"step 'a gate command' {SCRIPT_REGISTRY_LINT}",
     f"step 'a gate command' {SCRIPT_REGISTRY_LINT}step 'a new lint' python3 tools/new_lint.py\n"),
    ("gate-script", "a step function returns before its commands", GATE_SCRIPT_NAME,
     "prepare() {\n", "prepare() {\n  return 0\n"),
    ("gate-script", "a step runs in the background, where its result is lost", GATE_SCRIPT_NAME,
     SCRIPT_REGISTRY_LINT, "python3 tools/lint/registry.py &\n"),
    ("gate-script", "a step has no label the check can read", GATE_SCRIPT_NAME,
     f"step 'a gate command' {SCRIPT_REGISTRY_LINT}", f'step "$label" {SCRIPT_REGISTRY_LINT}'),
]

# (rule, what the edit does, file, text to replace, replacement): edits that stay inside a rule.
ACCEPTED = [
    ("job-timeout", "a job's timeout is at the bound", "book-pages.yml",
     BUILD_JOB + BUILD_TIMEOUT,
     BUILD_JOB + f"    timeout-minutes: {gates.MAX_TIMEOUT_MINUTES}\n"),
    ("release-triggers", "the daily run moves to another time of day", "release-gate.yml",
     '    - cron: "41 9 * * *"\n', '    - cron: "7 23 * * *"\n'),
    ("release-triggers", "the pushed branches are listed in another order", "release-gate.yml",
     "    branches: [main, development]\n", "    branches: [development, main]\n"),
    ("shell", "a step names the default shell", "pr-gate.yml",
     REGISTRY_LINT, "      - shell: bash\n        run: |\n"
                    "          python3 tools/lint/registry.py\n"),
    ("gate-commands", "a pinned line is written with a continuation", "pr-gate.yml",
     FAULT_LIBRARY, "      - run: |\n          cargo run --locked \\\n"
                    "            --manifest-path tools/verify/Cargo.toml -- --all\n"),
    ("gate-script", "a gate script command is written with a continuation", GATE_SCRIPT_NAME,
     SCRIPT_FAULT_LIBRARY,
     "\\\n  cargo run --locked --manifest-path tools/verify/Cargo.toml \\\n  -- --all\n"),
]

# The shapes .agents/gate.sh uses, in a script small enough to read the expected result off.
READER_SCRIPT = """#!/usr/bin/env bash
# step 'a comment is not a step' python3 commented_out.py
BIN=target/bin
MODE="${1:-light}"
step() {
  shift
  "$@"
}
engine_step() {
  step "$@"
}
select_toolchain() {
  local channel
  channel="$(bash pinned.sh toolchain)" || return 1
  if ! rustup run "$channel" rustc --version; then
    echo "not installed: $channel" >&2
    return 1
  fi
}
pinned_sources() {
  bash pinned.sh check && select_toolchain && sources_ok=1
}
replay() {
  cargo run --locked -- --all | tee "$BIN/replay.log"
}
if [ "$MODE" = full ]; then
  step 'clean (full)' rm -rf "${BIN}" "$BINARIES"
fi
step 'unit tests' python3 -m unittest discover -s tests -p 'test_*.py' -v
step 'pinned sources' pinned_sources
engine_step 'integration' \\
  env ROOT=../release \\
  cargo test --locked -- --include-ignored
engine_step 'replay' replay
step 'book' "$BIN/mdbook" build book
cat <<COVERAGE
step 'a heredoc line is not a step' python3 in_heredoc.py
COVERAGE
"""
READER_COMMANDS = {
    ("rm", "-rf", "target/bin", "$BINARIES"),
    ("python3", "-m", "unittest", "discover", "-s", "tests", "-p", "test_*.py", "-v"),
    ("bash", "pinned.sh", "check"),
    ("local", "channel"),
    ("channel=$(bash pinned.sh toolchain)",),
    ("return", "1"),
    ("rustup", "run", "$channel", "rustc", "--version"),
    ("echo", "not installed: $channel", ">&", "2"),
    ("sources_ok=1",),
    ("ROOT=../release", "cargo", "test", "--locked", "--", "--include-ignored"),
    ("cargo", "run", "--locked", "--", "--all"),
    ("tee", "target/bin/replay.log"),
    ("target/bin/mdbook", "build", "book"),
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
                shown = failures[0].replace(str(Path(tmp)), "<fixture>")
                shown = shown.replace(Path(tmp).name, "<fixture>")
                print(f"\n  refused [{rule}] {what}\n    {shown}", end="", file=sys.stderr)

    def test_each_edit_inside_a_rule_is_accepted(self):
        for rule, what, name, old, new in ACCEPTED:
            with self.subTest(rule=rule, edit=what), tempfile.TemporaryDirectory() as tmp:
                self.assertEqual(check_files(Path(tmp), edited(self, name, old, new)), [])


class PinnedGateCommands(unittest.TestCase):
    """No pinned line can leave a gate workflow or the gate script, whichever line it is."""

    def test_a_gate_workflow_without_any_one_pinned_line_is_refused(self):
        builders = {"pr-gate.yml": pr_gate, "release-gate.yml": release_gate}
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

    def test_a_gate_script_without_any_one_gate_command_is_refused(self):
        for index, line in enumerate(gates.GATE_COMMANDS):
            with self.subTest(line=line), tempfile.TemporaryDirectory() as tmp:
                without = gates.GATE_COMMANDS[:index] + gates.GATE_COMMANDS[index + 1:]
                files = {**POSITIVE, GATE_SCRIPT_NAME: gate_script(without)}
                failures = check_files(Path(tmp), files)
                self.assertEqual([rule_of(failure) for failure in failures], ["gate-script"],
                                 failures)
                self.assertIn(f"no step runs the gate command `{line}`", failures[0])

    def test_the_pin_separates_gate_commands_from_runner_preparation(self):
        pr_gate_lines = {line for lines in gates.PR_GATE.values() for line in lines}
        release_lines = {line for lines in gates.RELEASE_GATE.values() for line in lines}
        self.assertLessEqual(set(gates.CI_ONLY), pr_gate_lines)
        self.assertEqual(set(gates.GATE_COMMANDS), pr_gate_lines - set(gates.CI_ONLY))
        self.assertFalse(set(gates.GATE_COMMANDS) & set(gates.SCRIPT_ONLY))
        # The release gate runs the gate script and nothing else that is not runner preparation.
        self.assertEqual(release_lines - set(gates.CI_ONLY) - set(gates.GATE_COMMANDS),
                         {"bash .agents/gate.sh full"})


class GateScriptReader(unittest.TestCase):
    def test_the_step_commands_of_a_script_are_read(self):
        found, problems = gates.script_commands(READER_SCRIPT)
        self.assertEqual(problems, [])
        self.assertEqual(found, READER_COMMANDS)

    def test_a_variable_assigned_twice_is_not_replaced(self):
        found, problems = gates.script_commands(READER_SCRIPT + "BIN=elsewhere\n")
        self.assertEqual(problems, [])
        self.assertIn(("$BIN/mdbook", "build", "book"), found)

    def test_what_cannot_be_read_is_reported(self):
        unreadable = {
            "an unterminated quote": "step 'lint' python3 -c 'import json\n",
            "a step function called with arguments": (
                "lint() {\n  python3 lint.py\n}\nstep 'lint' lint --fast\n"
            ),
            "a step function that calls itself": (
                "lint() {\n  lint\n}\nstep 'lint' lint\n"
            ),
            "a step inside a step": "step 'outer' step 'inner' python3 lint.py\n",
            "a label that is not a literal": 'step "$label" python3 lint.py\n',
            "a step in the background": "step 'lint' python3 lint.py &\n",
        }
        for what, lines in unreadable.items():
            with self.subTest(what):
                found, problems = gates.script_commands('step() {\n  shift\n  "$@"\n}\n' + lines)
                self.assertEqual(len(problems), 1, (found, problems))


if __name__ == "__main__":
    unittest.main()
