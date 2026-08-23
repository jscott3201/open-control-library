import contextlib
import copy
import hashlib
import io
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools.lint import g36_source


SOURCE_ROOT = "Buildings/Controls/OBC/ASHRAE/G36"
LEGAL_PATH = "Buildings/legal.html"
LEGAL_BYTES = b"fixture revised BSD notice\nDOE notice\nenhancements paragraph\n"
GIT_ENV = {
    **os.environ,
    "GIT_AUTHOR_NAME": "G36 Source Test",
    "GIT_AUTHOR_EMAIL": "g36-source@example.invalid",
    "GIT_COMMITTER_NAME": "G36 Source Test",
    "GIT_COMMITTER_EMAIL": "g36-source@example.invalid",
    "GIT_AUTHOR_DATE": "2026-08-23T00:00:00Z",
    "GIT_COMMITTER_DATE": "2026-08-23T00:00:00Z",
}


class G36SourceTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.library_root = self.root / "library"
        (self.library_root / "routines/g36").mkdir(parents=True)
        self.repository_index = 0
        self.release_root, self.release_revision = self.create_source_checkout(
            "release"
        )
        self.development_root, self.development_revision = (
            self.create_source_checkout("development", development=True)
        )
        self.write_pins()

    def tearDown(self):
        self.temporary_directory.cleanup()

    def git(self, checkout, *arguments):
        return subprocess.run(
            ["git", "-C", str(checkout), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=GIT_ENV,
        ).stdout

    def create_source_checkout(
        self,
        label,
        *,
        development=False,
        include_root=True,
        legal: bytes | None = LEGAL_BYTES,
        executable_blob=False,
        origin=g36_source.REPOSITORY,
    ):
        self.repository_index += 1
        checkout = self.root / f"{label}-{self.repository_index}"
        checkout.mkdir()
        self.git(checkout, "init", "--quiet", "--initial-branch=main")
        self.git(checkout, "remote", "add", "origin", origin)

        files = {}
        if include_root:
            files = {
                f"{SOURCE_ROOT}/package.mo": b"within Buildings.Controls.OBC.ASHRAE;\npackage G36\nend G36;\n",
                f"{SOURCE_ROOT}/package.order": (
                    b"coolDown\nmissingInlineConstant\n../not-a-package-member\n\xff\n"
                ),
                f"{SOURCE_ROOT}/Controller.mo": (
                    b"within Buildings.Controls.OBC.ASHRAE.G36;\nblock Controller\nend Controller;\n"
                ),
                f"{SOURCE_ROOT}/Resources/data.txt": b"ordinary source-root blob\n",
            }
            if development:
                files[f"{SOURCE_ROOT}/Plants/package.mo"] = (
                    b"within Buildings.Controls.OBC.ASHRAE.G36;\npackage Plants\nend Plants;\n"
                )
            if executable_blob:
                files[f"{SOURCE_ROOT}/unsupported.sh"] = b"#!/bin/sh\nexit 0\n"
        if legal is not None:
            files[LEGAL_PATH] = legal

        for relative_path, content in files.items():
            path = checkout / relative_path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
        if executable_blob:
            (checkout / SOURCE_ROOT / "unsupported.sh").chmod(0o755)
        self.git(checkout, "add", "--all")
        self.git(checkout, "commit", "--quiet", "-m", "fixture")
        revision = self.git(checkout, "rev-parse", "HEAD").decode().strip()
        return checkout, revision

    def write_pins(self):
        (self.library_root / g36_source.RELEASE_PIN_PATH).write_text(
            f"{self.release_revision}\n", encoding="utf-8"
        )
        (self.library_root / g36_source.DEVELOPMENT_PIN_PATH).write_text(
            f"{self.development_revision}\n", encoding="utf-8"
        )

    def cli_args(self, mode):
        return [
            f"--{mode}",
            "--release-root",
            str(self.release_root),
            "--development-root",
            str(self.development_root),
        ]

    def write_generated(self):
        return g36_source.run(
            "write",
            self.release_root,
            self.development_root,
            repo_root=self.library_root,
        )

    @property
    def inventory_path(self):
        return self.library_root / g36_source.INVENTORY_PATH

    @property
    def retained_legal_path(self):
        return self.library_root / g36_source.RETAINED_LEGAL_PATH

    def read_inventory(self):
        return json.loads(self.inventory_path.read_text(encoding="utf-8"))

    def write_inventory(self, value):
        self.inventory_path.write_text(
            json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )

    def assert_failure(self, expected, mode="check"):
        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            result = g36_source.main(
                self.cli_args(mode), repo_root=self.library_root
            )
        self.assertEqual(result, 1)
        output = stderr.getvalue()
        self.assertIn(expected, output)
        self.assertNotIn("Traceback", output)
        self.assertEqual(stdout.getvalue(), "")
        return output

    def test_write_and_check_produce_expected_tree_only_artifacts(self):
        inventory = self.write_generated()
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = g36_source.main(
                self.cli_args("check"), repo_root=self.library_root
            )
        self.assertEqual(result, 0)
        self.assertEqual(
            output.getvalue(),
            "g36 source inventory: checked 4 release files and "
            "5 development files OK\n",
        )
        self.assertEqual(self.retained_legal_path.read_bytes(), LEGAL_BYTES)
        self.assertEqual(
            inventory["license"]["sha256"],
            f"sha256:{hashlib.sha256(LEGAL_BYTES).hexdigest()}",
        )
        self.assertEqual(
            self.inventory_path.read_bytes(), g36_source._canonical_json(inventory)
        )

    def test_repeated_write_and_check_are_deterministic_and_do_not_rewrite(self):
        self.write_generated()
        first_inventory = self.inventory_path.read_bytes()
        first_legal = self.retained_legal_path.read_bytes()
        first_inventory_mtime = self.inventory_path.stat().st_mtime_ns
        first_legal_mtime = self.retained_legal_path.stat().st_mtime_ns

        self.write_generated()
        g36_source.run(
            "check",
            self.release_root,
            self.development_root,
            repo_root=self.library_root,
        )
        g36_source.run(
            "check",
            self.release_root,
            self.development_root,
            repo_root=self.library_root,
        )
        self.assertEqual(self.inventory_path.read_bytes(), first_inventory)
        self.assertEqual(self.retained_legal_path.read_bytes(), first_legal)
        self.assertEqual(self.inventory_path.stat().st_mtime_ns, first_inventory_mtime)
        self.assertEqual(self.retained_legal_path.stat().st_mtime_ns, first_legal_mtime)

    def test_package_order_content_is_never_interpreted(self):
        inventory = self.write_generated()
        for snapshot in inventory["snapshots"]:
            rows = {row["path"]: row for row in snapshot["files"]}
            package_order_path = f"{SOURCE_ROOT}/package.order"
            self.assertIn(package_order_path, rows)
            self.assertEqual(snapshot["package_order_count"], 1)
            committed = self.git(
                self.release_root if snapshot["role"] == "release" else self.development_root,
                "show",
                f"HEAD:{package_order_path}",
            )
            self.assertEqual(
                rows[package_order_path]["sha256"],
                f"sha256:{hashlib.sha256(committed).hexdigest()}",
            )
        g36_source.run(
            "check",
            self.release_root,
            self.development_root,
            repo_root=self.library_root,
        )

    def test_dirty_checkout_bytes_do_not_change_git_object_inventory(self):
        inventory = self.write_generated()
        original_bytes = self.inventory_path.read_bytes()
        (self.release_root / SOURCE_ROOT / "package.order").write_bytes(b"changed\n")
        (self.development_root / LEGAL_PATH).write_bytes(b"changed legal\n")

        checked = g36_source.run(
            "check",
            self.release_root,
            self.development_root,
            repo_root=self.library_root,
        )
        self.assertEqual(checked, inventory)
        self.assertEqual(self.inventory_path.read_bytes(), original_bytes)

    def test_release_and_development_snapshots_remain_isolated(self):
        inventory = self.write_generated()
        release_paths = {row["path"] for row in inventory["snapshots"][0]["files"]}
        development_paths = {
            row["path"] for row in inventory["snapshots"][1]["files"]
        }
        plants_path = f"{SOURCE_ROOT}/Plants/package.mo"
        self.assertNotIn(plants_path, release_paths)
        self.assertIn(plants_path, development_paths)
        self.assertEqual(inventory["snapshots"][0]["role"], "release")
        self.assertEqual(inventory["snapshots"][1]["role"], "development")

    def test_checkout_head_must_match_its_pin(self):
        (self.library_root / g36_source.RELEASE_PIN_PATH).write_text(
            f"{'0' * 40}\n", encoding="utf-8"
        )
        self.assert_failure("release checkout: HEAD must be", mode="write")

    def test_checkout_origin_must_identify_upstream_repository(self):
        self.git(
            self.release_root,
            "remote",
            "set-url",
            "origin",
            "https://github.com/example/not-modelica-buildings.git",
        )
        self.assert_failure(
            "release checkout: origin must identify lbl-srg/modelica-buildings",
            mode="write",
        )

    def test_missing_source_root_and_legal_file_fail_cleanly(self):
        with self.subTest(case="source root"):
            self.release_root, self.release_revision = self.create_source_checkout(
                "missing-root", include_root=False
            )
            self.write_pins()
            self.assert_failure("release snapshot: source root", mode="write")

        self.release_root, self.release_revision = self.create_source_checkout(
            "missing-legal", legal=None
        )
        self.write_pins()
        self.assert_failure(
            f"release snapshot: legal file {LEGAL_PATH} is missing", mode="write"
        )

    def test_unsupported_git_mode_fails_closed(self):
        self.release_root, self.release_revision = self.create_source_checkout(
            "executable", executable_blob=True
        )
        self.write_pins()
        self.assert_failure(
            "unsupported Git entry", mode="write"
        )
        self.assert_failure("v1 supports only 100644 blobs", mode="write")

    def test_legal_notices_must_match_across_snapshots(self):
        self.development_root, self.development_revision = self.create_source_checkout(
            "different-legal", development=True, legal=b"different notice\n"
        )
        self.write_pins()
        self.assert_failure(
            "release and development legal notices differ", mode="write"
        )

    def test_retained_legal_notice_drift_and_absence_are_rejected(self):
        self.write_generated()
        self.retained_legal_path.write_bytes(b"changed\n")
        self.assert_failure("bytes do not match the pinned legal notice")

        self.write_generated()
        self.retained_legal_path.unlink()
        self.assert_failure("LICENSE-BUILDINGS.html: file is missing")

    def test_check_failure_never_rewrites_either_output(self):
        self.write_generated()
        self.inventory_path.write_bytes(b"{}\n")
        self.retained_legal_path.write_bytes(b"locally changed notice\n")
        inventory_before = self.inventory_path.read_bytes()
        legal_before = self.retained_legal_path.read_bytes()
        self.assert_failure("keys must appear exactly in this order")
        self.assertEqual(self.inventory_path.read_bytes(), inventory_before)
        self.assertEqual(self.retained_legal_path.read_bytes(), legal_before)

    def test_malformed_json_and_duplicate_object_keys_fail_cleanly(self):
        self.write_generated()
        self.inventory_path.write_bytes(b"{")
        self.assert_failure("invalid JSON at line 1, column 2")

        self.inventory_path.write_text(
            '{"schema": "one", "schema": "two"}\n', encoding="utf-8"
        )
        self.assert_failure("duplicate object key 'schema'")

    def test_manifest_static_shape_and_snapshot_order_are_exact(self):
        self.write_generated()
        valid = self.read_inventory()

        cases = (
            (
                "schema",
                lambda value: value.update(schema="wrong"),
                "schema must be 'cxf-library/g36-source-inventory/v1'",
            ),
            (
                "extra top key",
                lambda value: value.update(extra=True),
                "keys must appear exactly in this order",
            ),
            (
                "swapped snapshots",
                lambda value: value["snapshots"].reverse(),
                "snapshots[0].role: must be 'release'",
            ),
            (
                "wrong role",
                lambda value: value["snapshots"][0].update(role="development"),
                "snapshots[0].role: must be 'release'",
            ),
            (
                "wrong revision",
                lambda value: value["snapshots"][0].update(revision="0" * 40),
                "snapshots[0].revision: must equal release source pin",
            ),
            (
                "snapshot extra key",
                lambda value: value["snapshots"][0].update(extra=True),
                "keys must appear exactly in this order",
            ),
            (
                "license extra key",
                lambda value: value["license"].update(extra=True),
                "keys must appear exactly in this order",
            ),
        )
        for name, mutation, expected in cases:
            with self.subTest(name=name):
                changed = copy.deepcopy(valid)
                mutation(changed)
                self.write_inventory(changed)
                self.assert_failure(expected)

    def test_manifest_paths_order_modes_hashes_and_sizes_are_validated(self):
        self.write_generated()
        valid = self.read_inventory()

        def first_file(value):
            return value["snapshots"][0]["files"][0]

        cases = (
            (
                "unsafe path",
                lambda value: first_file(value).update(path="../outside"),
                "parent traversal is forbidden",
            ),
            (
                "wrong order",
                lambda value: value["snapshots"][0]["files"].reverse(),
                "paths must be lexicographically ordered",
            ),
            (
                "duplicate path",
                lambda value: value["snapshots"][0]["files"][1].update(
                    path=first_file(value)["path"]
                ),
                "duplicate",
            ),
            (
                "mode",
                lambda value: first_file(value).update(mode="100755"),
                "mode: must be '100644'",
            ),
            (
                "negative size",
                lambda value: first_file(value).update(bytes=-1),
                "bytes: must be a nonnegative integer",
            ),
            (
                "blob hash",
                lambda value: first_file(value).update(git_blob_sha1="0" * 40),
                "git_blob_sha1: must match sha1:<40 lowercase hex>",
            ),
            (
                "content hash",
                lambda value: first_file(value).update(sha256="sha256:bad"),
                "sha256: must match sha256:<64 lowercase hex>",
            ),
            (
                "tree hash",
                lambda value: value["snapshots"][0].update(root_tree_sha1="bad"),
                "root_tree_sha1: must match sha1:<40 lowercase hex>",
            ),
        )
        for name, mutation, expected in cases:
            with self.subTest(name=name):
                changed = copy.deepcopy(valid)
                mutation(changed)
                self.write_inventory(changed)
                self.assert_failure(expected)

    def test_manifest_counts_are_recomputed_from_file_rows(self):
        self.write_generated()
        valid = self.read_inventory()
        for key in (
            "file_count",
            "total_bytes",
            "modelica_file_count",
            "package_order_count",
        ):
            with self.subTest(key=key):
                changed = copy.deepcopy(valid)
                changed["snapshots"][0][key] += 1
                self.write_inventory(changed)
                self.assert_failure(f"snapshots[0].{key}: must equal")

    def test_missing_extra_and_modified_manifest_entries_are_rejected(self):
        self.write_generated()
        valid = self.read_inventory()

        missing = copy.deepcopy(valid)
        missing["snapshots"][0]["files"].pop()
        self.write_inventory(missing)
        self.assert_failure("snapshots[0].file_count: must equal")

        extra = copy.deepcopy(valid)
        extra["snapshots"][0]["files"].append(
            {
                "path": f"{SOURCE_ROOT}/zz-extra.bin",
                "mode": "100644",
                "bytes": 0,
                "git_blob_sha1": f"sha1:{'0' * 40}",
                "sha256": f"sha256:{'0' * 64}",
            }
        )
        self.write_inventory(extra)
        self.assert_failure("snapshots[0].file_count: must equal")

        modified = copy.deepcopy(valid)
        modified["snapshots"][0]["files"][0]["git_blob_sha1"] = (
            f"sha1:{'0' * 40}"
        )
        self.write_inventory(modified)
        self.assert_failure("value does not match the pinned Git objects")

    def test_valid_but_wrong_tree_and_file_size_fail_against_git_objects(self):
        self.write_generated()
        valid = self.read_inventory()

        changed_tree = copy.deepcopy(valid)
        changed_tree["snapshots"][0]["root_tree_sha1"] = f"sha1:{'0' * 40}"
        self.write_inventory(changed_tree)
        self.assert_failure("root_tree_sha1: value does not match the pinned Git objects")

        changed_size = copy.deepcopy(valid)
        changed_size["snapshots"][0]["files"][0]["bytes"] += 1
        changed_size["snapshots"][0]["files"][1]["bytes"] -= 1
        self.write_inventory(changed_size)
        self.assert_failure(".bytes: value does not match the pinned Git objects")


if __name__ == "__main__":
    unittest.main()
