"""Fixtures for the other gate scripts: the CI OK result check, the tracked-file scan, the
pinned-sources check and the mdBook installer. Each control proves that the script can fail.

No case touches the network: the installer runs against a stub `curl`, and the pinned-sources
cases build throwaway local repositories.
"""

import hashlib
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_ok  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
SCRIPTS = REPO / ".github" / "scripts"


def run(command, cwd, **env):
    return subprocess.run(
        command, cwd=cwd, env={**os.environ, **env}, text=True, capture_output=True, check=False
    )


def git(cwd, *args):
    identity = ["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid"]
    result = run(["git", *identity, *args], cwd)
    if result.returncode != 0:
        raise AssertionError(result.stderr)
    return result.stdout.strip()


class CiOkResultCheck(unittest.TestCase):
    def test_every_job_succeeded(self):
        report = ci_ok.check({"lint": {"result": "success"}, "book": {"result": "success"}})
        self.assertEqual(report, ["book: success", "lint: success"])

    def test_anything_but_success_fails(self):
        for result in ("skipped", "failure", "cancelled", None, ""):
            with self.subTest(result=result), self.assertRaises(ci_ok.CheckError):
                ci_ok.check({"lint": {"result": "success"}, "verify": {"result": result}})

    def test_no_results_or_malformed_results_fail(self):
        for needs in ({}, [], "success", {"lint": "success"}, {"lint": {}}):
            with self.subTest(needs=needs), self.assertRaises(ci_ok.CheckError):
                ci_ok.check(needs)

    def test_the_command_exits_non_zero_on_a_skip_and_without_input(self):
        script = [sys.executable, str(SCRIPTS / "ci_ok.py")]
        green = run(script, REPO, NEEDS_JSON='{"lint": {"result": "success"}}')
        skipped = run(script, REPO, NEEDS_JSON='{"lint": {"result": "skipped"}}')
        garbled = run(script, REPO, NEEDS_JSON="{")
        environment = {key: value for key, value in os.environ.items() if key != "NEEDS_JSON"}
        absent = subprocess.run(script, cwd=REPO, env=environment, capture_output=True, check=False)
        self.assertEqual(green.returncode, 0, green.stderr)
        self.assertEqual(skipped.returncode, 1)
        self.assertIn("gating jobs not green", skipped.stderr)
        self.assertEqual(garbled.returncode, 1)
        self.assertEqual(absent.returncode, 1)


class TrackedFileScan(unittest.TestCase):
    """The offending strings are assembled here, so this file holds none of the shapes."""

    def scan(self, content):
        with tempfile.TemporaryDirectory() as tmp:
            git(tmp, "init", "--quiet")
            Path(tmp, "note.md").write_text(content, encoding="utf-8")
            git(tmp, "add", "note.md")
            return run(["bash", str(SCRIPTS / "check-no-secrets.sh")], tmp)

    def test_a_clean_tree_passes(self):
        result = self.scan("see ../open-control and the commit 41e997f\n")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_each_shape_is_refused(self):
        shapes = {
            "absolute developer home-directory path": "/" + "Users" + "/someone/checkout",
            "absolute developer home-directory path ": "/" + "home" + "/runner/work",
            "UUID-shaped string": "-".join(["0a1b2c3d", "4e5f", "6071", "8293", "a4b5c6d7e8f9"]),
            "private key block": "-----BEGIN " + "OPENSSH PRIVATE KEY-----",
            "GitHub token": "gh" + "p_" + "A" * 36,
        }
        for label, text in shapes.items():
            with self.subTest(shape=label.strip()):
                result = self.scan(f"value: {text}\n")
                self.assertEqual(result.returncode, 1, result.stdout)
                self.assertIn(label.strip(), result.stdout)

    def test_a_search_that_cannot_run_is_not_a_clean_scan(self):
        """Outside a repository `git grep` exits above 1 for every pattern."""
        with tempfile.TemporaryDirectory() as tmp:
            result = run(
                ["bash", str(SCRIPTS / "check-no-secrets.sh")], tmp,
                GIT_CEILING_DIRECTORIES=str(Path(tmp).resolve().parent),
                GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
            )
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        patterns = (SCRIPTS / "check-no-secrets.sh").read_text(encoding="utf-8").count("\nscan '")
        self.assertGreater(patterns, 0)
        self.assertEqual(result.stdout.count("FAIL: git grep exited"), patterns, result.stdout)
        self.assertIn("The scan did not complete", result.stdout)
        self.assertNotIn("OK: tracked-file scan clean", result.stdout)
        self.assertIn("not a git repository", result.stderr)

    def test_one_search_that_errors_fails_an_otherwise_clean_scan(self):
        """A `git` that fails on one pattern and searches the rest: exit status 2 is not 1."""
        with tempfile.TemporaryDirectory() as tmp:
            stub = Path(tmp, "stub")
            stub.mkdir()
            (stub / "git").write_text(
                '#!/usr/bin/env bash\n'
                'for argument in "$@"; do\n'
                '  case "$argument" in xox*) echo "stub git: cannot search" >&2; exit 2 ;; esac\n'
                'done\n'
                'exec "$REAL_GIT" "$@"\n',
                encoding="utf-8",
            )
            (stub / "git").chmod(0o755)
            tree = Path(tmp, "tree")
            tree.mkdir()
            git(tree, "init", "--quiet")
            (tree / "note.md").write_text("nothing to find\n", encoding="utf-8")
            git(tree, "add", "note.md")
            result = run(
                ["bash", str(SCRIPTS / "check-no-secrets.sh")], tree,
                PATH=f"{stub}{os.pathsep}{os.environ['PATH']}", REAL_GIT=shutil.which("git"),
            )
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("FAIL: git grep exited 2 on the Slack token pattern", result.stdout)
        self.assertEqual(result.stdout.count("FAIL:"), 1, result.stdout)
        self.assertIn("stub git: cannot search", result.stderr)
        self.assertNotIn("OK: tracked-file scan clean", result.stdout)


class PinnedSourcesCheck(unittest.TestCase):
    """`check` names each sibling checkout that is missing, off its pin or modified."""

    NAMES = ("open-control", "modelica-buildings-release", "modelica-buildings-development")
    PINS = ("ENGINE_PIN", "routines/g36/SOURCE_RELEASE_PIN", "routines/g36/SOURCE_DEVELOPMENT_PIN")

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.parent = Path(self.tmp.name)
        # A stand-in repository root: the script reads the pin files relative to itself.
        self.root = self.parent / "library"
        (self.root / ".github" / "scripts").mkdir(parents=True)
        (self.root / "routines" / "g36").mkdir(parents=True)
        script = self.root / ".github" / "scripts" / "pinned-sources.sh"
        script.write_bytes((SCRIPTS / "pinned-sources.sh").read_bytes())
        self.heads = {}
        for name, pin in zip(self.NAMES, self.PINS):
            checkout = self.parent / name
            checkout.mkdir()
            git(checkout, "init", "--quiet")
            (checkout / "rust-toolchain.toml").write_text(
                '[toolchain]\nchannel = "1.97.1"\n', encoding="utf-8"
            )
            (checkout / "name").write_text(name, encoding="utf-8")
            git(checkout, "add", ".")
            git(checkout, "commit", "--quiet", "-m", "fixture")
            self.heads[name] = git(checkout, "rev-parse", "HEAD")
            (self.root / pin).write_text(self.heads[name] + "\n", encoding="utf-8")

    def check(self, mode="check"):
        return run(["bash", str(self.root / ".github/scripts/pinned-sources.sh"), mode], self.root)

    def test_checkouts_on_their_pins_pass(self):
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout.count("pinned sources: ok ../"), 3)
        self.assertEqual(self.check("toolchain").stdout.strip(), "1.97.1")

    def test_a_missing_checkout_is_named(self):
        (self.parent / "modelica-buildings-release").rename(self.parent / "elsewhere")
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("MISSING ../modelica-buildings-release", result.stdout)
        self.assertIn("1 of 3 checkouts are not usable", result.stdout)

    def test_a_checkout_off_its_pin_is_named(self):
        engine = self.parent / "open-control"
        git(engine, "commit", "--quiet", "--allow-empty", "-m", "one commit past the pin")
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("OFF PIN ../open-control is at", result.stdout)
        self.assertIn(f"ENGINE_PIN names {self.heads['open-control']}", result.stdout)

    def test_a_modified_engine_checkout_is_named(self):
        (self.parent / "open-control" / "name").write_text("edited", encoding="utf-8")
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("MODIFIED ../open-control", result.stdout)

    def test_a_pin_file_that_is_not_one_commit_is_refused(self):
        (self.root / "ENGINE_PIN").write_text("development\n", encoding="utf-8")
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("ENGINE_PIN does not hold one 40-hex commit", result.stderr)


class MdbookInstaller(unittest.TestCase):
    """A binary or archive whose digest is not the pinned one is never accepted."""

    TRIPLES = {
        ("Darwin", "arm64"): "aarch64-apple-darwin",
        ("Darwin", "x86_64"): "x86_64-apple-darwin",
        ("Linux", "aarch64"): "aarch64-unknown-linux-musl",
        ("Linux", "arm64"): "aarch64-unknown-linux-musl",
        ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    }

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        tmp = Path(self.tmp.name)
        self.book = tmp / "root" / "tools" / "book"
        self.book.mkdir(parents=True)
        source = REPO / "tools" / "book"
        (self.book / "install-mdbook.sh").write_bytes((source / "install-mdbook.sh").read_bytes())
        version = (source / "mdbook-version").read_text(encoding="utf-8")
        (self.book / "mdbook-version").write_text(version, encoding="utf-8")
        triple = self.TRIPLES[(platform.system(), platform.machine())]
        archive = f"mdbook-v{version.strip()}-{triple}.tar.gz"
        self.pinned_binary = b"pinned-fixture-binary\n"
        digest = hashlib.sha256(self.pinned_binary).hexdigest()
        (self.book / "mdbook-checksums.txt").write_text(
            f"{'0' * 64}  {archive}\n{digest}  {archive}:mdbook\n", encoding="utf-8"
        )
        # The stub records each download attempt, then writes an archive that is not the pinned one.
        self.stub = tmp / "stub"
        self.stub.mkdir()
        self.log = tmp / "curl.log"
        (self.stub / "curl").write_text(
            '#!/usr/bin/env bash\necho "curl $*" >> "$STUB_CURL_LOG"\n'
            'while [ "$#" -gt 0 ]; do\n'
            '  if [ "$1" = --output ]; then printf not-the-release > "$2"; fi\n'
            "  shift\ndone\n",
            encoding="utf-8",
        )
        (self.stub / "curl").chmod(0o755)
        self.destination = tmp / "bin"
        self.destination.mkdir()

    def install(self):
        return run(
            ["bash", str(self.book / "install-mdbook.sh"), str(self.destination)],
            self.book,
            PATH=f"{self.stub}{os.pathsep}{os.environ['PATH']}",
            STUB_CURL_LOG=str(self.log),
        )

    def test_a_binary_with_the_pinned_digest_is_accepted_without_a_download(self):
        (self.destination / "mdbook").write_bytes(self.pinned_binary)
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("already present and verified", result.stdout)
        self.assertFalse(self.log.exists())

    def test_a_binary_with_another_digest_is_not_trusted(self):
        (self.destination / "mdbook").write_bytes(b"some other binary\n")
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.log.exists(), "the installer must go back to the release")
        self.assertEqual((self.destination / "mdbook").read_bytes(), b"some other binary\n")

    def test_a_downloaded_archive_with_another_digest_is_refused(self):
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mdBook checksum mismatch", result.stderr)
        self.assertFalse((self.destination / "mdbook").exists())


if __name__ == "__main__":
    unittest.main()
