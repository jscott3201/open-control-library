"""Fixtures for check_workflow_gates.py: one refused workflow edit per rule.

The positive fixture is three small workflows that satisfy every rule. Each case below applies one
edit to it and must be refused under exactly the rule it names, so a rule that stops firing, or an
edit that trips a different rule than intended, fails here. Run with `-v` to see each refusal:

    python3 -m unittest discover -s .github/scripts -p 'test_*.py' -v
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_workflow_gates as gates  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
SHA = "0123456789abcdef0123456789abcdef01234567"
CHECKOUT = f"""      - uses: actions/checkout@{SHA} # v6.1.0
        with:
          persist-credentials: false
"""
HEADER = """permissions:
  contents: read
concurrency:
  group: ${{ github.workflow }}-${{ github.event_name }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
"""
CI_OK = f"""  ci-ok:
    name: CI OK
    if: always()
    needs: [@NEEDS@]
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
{CHECKOUT}      - env:
          NEEDS_JSON: ${{{{ toJSON(needs) }}}}
        run: python3 .github/scripts/ci_ok.py
"""
PR_GATE = (
    f"""on:
  pull_request:
    branches: [development]
  workflow_dispatch:
{HEADER}jobs:
  checks:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
{CHECKOUT}"""
    + "".join(f"      - run: {command}\n" for command in gates.PR_GATE_COMMANDS)
    + f"""  book:
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
{CHECKOUT}      - run: echo built
"""
    + CI_OK.replace("@NEEDS@", "checks, book")
)
RELEASE_GATE = (
    f"""on:
  pull_request:
    branches: [main]
  push:
    branches: [main, development]
  schedule:
    - cron: "41 9 * * *"
  workflow_dispatch:
{HEADER}jobs:
  gate:
    runs-on: ubuntu-latest
    timeout-minutes: 45
    steps:
{CHECKOUT}      - uses: actions/setup-python@{SHA} # v5.6.0
        with:
          python-version: "3.12"
      - run: bash .agents/gate.sh full
"""
    + CI_OK.replace("@NEEDS@", "gate")
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
    if: github.ref == 'refs/heads/main'
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
    "pr-gate.yml": PR_GATE,
    "release-gate.yml": RELEASE_GATE,
    "book-pages.yml": BOOK_PAGES,
}

# (rule, what the edit does, file, text to replace, replacement). A `None` text adds or removes the
# whole file: the replacement is its content, or None to delete it.
CASES = [
    ("workflow-set", "a fourth workflow file appears", "ci.yml", None, PR_GATE),
    ("workflow-set", "the book workflow is deleted", "book-pages.yml", None, None),
    ("action-pin", "an action is referenced by a movable tag", "pr-gate.yml",
     f"  checks:\n    runs-on: ubuntu-latest\n    timeout-minutes: 10\n    steps:\n"
     f"      - uses: actions/checkout@{SHA} # v6.1.0",
     "  checks:\n    runs-on: ubuntu-latest\n    timeout-minutes: 10\n    steps:\n"
     "      - uses: actions/checkout@v6"),
    ("action-pin", "a pinned action loses its tag comment", "book-pages.yml",
     f"actions/deploy-pages@{SHA} # v4.0.5", f"actions/deploy-pages@{SHA}"),
    ("checkout-token", "a checkout keeps the job token", "release-gate.yml",
     "    timeout-minutes: 45\n    steps:\n"
     f"      - uses: actions/checkout@{SHA} # v6.1.0\n        with:\n"
     "          persist-credentials: false",
     "    timeout-minutes: 45\n    steps:\n"
     f"      - uses: actions/checkout@{SHA} # v6.1.0\n        with:\n"
     "          fetch-depth: 1"),
    ("job-timeout", "a job loses its timeout", "book-pages.yml",
     "  build:\n    runs-on: ubuntu-latest\n    timeout-minutes: 15\n",
     "  build:\n    runs-on: ubuntu-latest\n"),
    ("no-soft-failure", "a step is allowed to fail", "pr-gate.yml",
     "      - run: echo built\n",
     f"      - run: echo built\n        {gates.SOFT_FAILURE_KEY}: true\n"),
    ("event-in-run", "a run block interpolates the event payload", "pr-gate.yml",
     "      - run: echo built\n",
     '      - run: echo "${{ github.event.pull_request.title }}"\n'),
    ("permissions", "the workflow token can write", "pr-gate.yml",
     "permissions:\n  contents: read\n", "permissions:\n  contents: write\n"),
    ("permissions", "a gate job widens its own token", "release-gate.yml",
     "  gate:\n    runs-on: ubuntu-latest\n",
     "  gate:\n    permissions:\n      contents: write\n    runs-on: ubuntu-latest\n"),
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
    ("release-triggers", "merges into development no longer run the release gate",
     "release-gate.yml", "    branches: [main, development]\n", "    branches: [main]\n"),
    ("release-triggers", "the daily run is removed", "release-gate.yml",
     '  schedule:\n    - cron: "41 9 * * *"\n', ""),
    ("ci-ok-needs", "a job is missing from CI OK's needs", "pr-gate.yml",
     "    needs: [checks, book]\n", "    needs: [checks]\n"),
    ("ci-ok-needs", "CI OK no longer runs when a job fails", "release-gate.yml",
     "    if: always()\n", ""),
    ("ci-ok-needs", "CI OK stops checking the results", "pr-gate.yml",
     "        run: python3 .github/scripts/ci_ok.py\n", "        run: echo ok\n"),
    ("gate-if", "a gate job is conditional", "pr-gate.yml",
     "  book:\n    runs-on: ubuntu-latest\n",
     "  book:\n    if: github.event_name == 'pull_request'\n    runs-on: ubuntu-latest\n"),
    ("gate-if", "a gate step is conditional", "release-gate.yml",
     "      - run: bash .agents/gate.sh full\n",
     "      - if: github.ref == 'refs/heads/main'\n        run: bash .agents/gate.sh full\n"),
    ("gate-commands", "the per-PR gate stops replaying the fault library", "pr-gate.yml",
     "      - run: cargo run --locked --manifest-path tools/verify/Cargo.toml -- --all\n", ""),
    ("gate-commands", "the release gate runs the light gate script", "release-gate.yml",
     "      - run: bash .agents/gate.sh full\n", "      - run: bash .agents/gate.sh\n"),
    ("gate-commands", "the release gate restores a cache", "release-gate.yml",
     '          python-version: "3.12"\n', '          python-version: "3.12"\n          cache: pip\n'),
]


def write_workflows(directory, files):
    directory.mkdir(parents=True, exist_ok=True)
    for name, text in files.items():
        (directory / name).write_text(text, encoding="utf-8")


def rule_of(failure):
    return failure.split(": ")[1]


class WorkflowRuleFixtures(unittest.TestCase):
    def test_the_positive_fixture_satisfies_every_rule(self):
        with tempfile.TemporaryDirectory() as tmp:
            write_workflows(Path(tmp), POSITIVE)
            self.assertEqual(gates.check(Path(tmp)), [])

    def test_the_repository_workflows_satisfy_every_rule(self):
        self.assertEqual(gates.check(REPO / ".github" / "workflows"), [])

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
                files = dict(POSITIVE)
                if old is None and new is None:
                    del files[name]
                elif old is None:
                    files[name] = new
                else:
                    self.assertEqual(files[name].count(old), 1, "the edit must apply exactly once")
                    files[name] = files[name].replace(old, new)
                write_workflows(Path(tmp), files)
                failures = gates.check(Path(tmp))
                self.assertTrue(failures, f"{what}: accepted")
                self.assertEqual({rule_of(failure) for failure in failures}, {rule}, failures)
                shown = failures[0].replace(str(Path(tmp)), "<fixture>")
                print(f"\n  refused [{rule}] {what}\n    {shown}", end="", file=sys.stderr)


if __name__ == "__main__":
    unittest.main()
