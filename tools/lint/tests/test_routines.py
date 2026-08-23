import contextlib
import copy
import io
import json
import shutil
import tempfile
import unittest
from pathlib import Path

from tools.lint import routines as routine_lint


PRODUCT_ROOT = Path(__file__).resolve().parents[3]
JSON_ARTIFACTS = (
    "routines/registry.json",
    "routines/generated-registry.json",
    "routines/g36/scope.json",
    "routines/g36/coverage.json",
)
PIN_ARTIFACTS = (
    "routines/g36/SOURCE_RELEASE_PIN",
    "routines/g36/SOURCE_DEVELOPMENT_PIN",
)


class RoutineLintTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        for relative_path in JSON_ARTIFACTS + PIN_ARTIFACTS:
            self.restore_product_file(relative_path)

    def tearDown(self):
        self.temporary_directory.cleanup()

    def restore_product_file(self, relative_path):
        source = PRODUCT_ROOT / relative_path
        destination = self.root / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

    def write_text(self, relative_path, value):
        path = self.root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value, encoding="utf-8")

    def read_json(self, relative_path):
        return json.loads((self.root / relative_path).read_text(encoding="utf-8"))

    def write_json(self, relative_path, value):
        self.write_text(relative_path, json.dumps(value) + "\n")

    def mutate_json(self, relative_path, change):
        value = self.read_json(relative_path)
        change(value)
        self.write_json(relative_path, value)

    def assert_error(self, expected):
        errors = routine_lint.validate(self.root)
        self.assertTrue(
            any(expected in error for error in errors),
            f"{expected!r} not found in {errors!r}",
        )
        self.assertFalse(any("Traceback" in error for error in errors))
        return errors

    def test_production_catalog_and_repeated_validation_are_clean(self):
        self.assertEqual(routine_lint.validate(PRODUCT_ROOT), [])
        first = routine_lint.validate(self.root)
        second = routine_lint.validate(self.root)
        self.assertEqual(first, [])
        self.assertEqual(second, first)

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_lint.main(self.root)
        self.assertEqual(result, 0)
        self.assertEqual(
            output.getvalue(),
            "routine catalog lint: 22 planned scope anchors, 0 canonical routines, "
            "0 generated deployments OK\n",
        )

    def test_required_pins_must_exist_use_exact_format_and_differ(self):
        for relative_path in PIN_ARTIFACTS:
            with self.subTest(relative_path=relative_path, case="missing"):
                (self.root / relative_path).unlink()
                self.assert_error(f"{relative_path}: file is missing")
                self.restore_product_file(relative_path)

            for bad_value in ("ABCDEF\n", "a" * 40, "A" * 40 + "\n"):
                with self.subTest(relative_path=relative_path, bad_value=bad_value):
                    self.write_text(relative_path, bad_value)
                    self.assert_error(
                        f"{relative_path}: must contain one lowercase 40-hex Git commit "
                        "followed by a newline"
                    )
                    self.restore_product_file(relative_path)

        release = (self.root / PIN_ARTIFACTS[0]).read_text(encoding="utf-8")
        self.write_text(PIN_ARTIFACTS[1], release)
        self.assert_error("source release and development pins must be distinct")

    def test_legacy_pins_are_rejected(self):
        for name in ("DONOR_PIN", "SOURCE_PIN"):
            with self.subTest(name=name):
                relative_path = f"routines/g36/{name}"
                self.write_text(relative_path, "a" * 40 + "\n")
                self.assert_error(f"{relative_path}: legacy pin must be absent")
                (self.root / relative_path).unlink()

    def test_json_artifacts_reject_missing_malformed_and_non_object_values(self):
        for relative_path in JSON_ARTIFACTS:
            with self.subTest(relative_path=relative_path, case="missing"):
                (self.root / relative_path).unlink()
                self.assert_error(f"{relative_path}: file is missing")
                self.restore_product_file(relative_path)

            with self.subTest(relative_path=relative_path, case="malformed"):
                self.write_text(relative_path, "{")
                self.assert_error(f"{relative_path}: invalid JSON at line 1, column 2")
                self.restore_product_file(relative_path)

            with self.subTest(relative_path=relative_path, case="array"):
                self.write_json(relative_path, [])
                self.assert_error(f"{relative_path}: must contain a JSON object")
                self.restore_product_file(relative_path)

    def test_all_top_level_shapes_and_schema_identifiers_are_exact(self):
        expected_schemas = {
            "routines/registry.json": "cxf-library/routine-registry/v2",
            "routines/generated-registry.json": (
                "cxf-library/generated-routine-registry/v1"
            ),
            "routines/g36/scope.json": "cxf-library/g36-scope/v1",
            "routines/g36/coverage.json": "cxf-library/g36-coverage/v2",
        }
        for relative_path, schema in expected_schemas.items():
            with self.subTest(relative_path=relative_path, case="schema"):
                self.mutate_json(relative_path, lambda value: value.update(schema="wrong"))
                self.assert_error(f"{relative_path}: schema must be {schema!r}")
                self.restore_product_file(relative_path)

            with self.subTest(relative_path=relative_path, case="extra"):
                self.mutate_json(relative_path, lambda value: value.update(extra=True))
                self.assert_error(f"{relative_path}: keys must be exactly")
                self.assert_error("unexpected extra")
                self.restore_product_file(relative_path)

            with self.subTest(relative_path=relative_path, case="missing"):
                self.mutate_json(relative_path, lambda value: value.pop("schema"))
                self.assert_error("missing schema")
                self.restore_product_file(relative_path)

    def test_l0_registries_require_empty_arrays(self):
        cases = (
            ("routines/registry.json", "routines"),
            ("routines/generated-registry.json", "deployments"),
        )
        for relative_path, key in cases:
            with self.subTest(relative_path=relative_path, case="shape"):
                self.mutate_json(relative_path, lambda value: value.update({key: {}}))
                self.assert_error(f"{relative_path}: {key} must be an array")
                self.restore_product_file(relative_path)

            with self.subTest(relative_path=relative_path, case="nonempty"):
                self.mutate_json(relative_path, lambda value: value.update({key: [{}]}))
                self.assert_error(f"{relative_path}: {key} must remain empty in L0")
                self.restore_product_file(relative_path)

    def test_scope_requires_exactly_22_object_rows_with_exact_keys(self):
        for count in (21, 23):
            with self.subTest(count=count):
                scope = self.read_json("routines/g36/scope.json")
                if count == 21:
                    scope["sections"].pop()
                else:
                    scope["sections"].append(copy.deepcopy(scope["sections"][-1]))
                self.write_json("routines/g36/scope.json", scope)
                self.assert_error("scope.json: sections must contain exactly 22 rows")
                self.restore_product_file("routines/g36/scope.json")

        self.mutate_json("routines/g36/scope.json", lambda value: value.update(sections={}))
        self.assert_error("scope.json: sections must be an array")
        self.restore_product_file("routines/g36/scope.json")

        def replace_first(value):
            value["sections"][0] = "not an object"

        self.mutate_json("routines/g36/scope.json", replace_first)
        self.assert_error("scope.json: sections[0] must be an object")
        self.restore_product_file("routines/g36/scope.json")

        def alter_keys(value):
            value["sections"][0].pop("name")
            value["sections"][0]["extra"] = True

        self.mutate_json("routines/g36/scope.json", alter_keys)
        errors = self.assert_error("scope.json: sections[0]: keys must be exactly")
        self.assertTrue(any("missing name; unexpected extra" in error for error in errors))

    def test_scope_profile_and_catalog_status_are_exact(self):
        cases = {
            "profile": (
                "other",
                "profile must be 'ASHRAE Guideline 36-2021 Section 5'",
            ),
            "status": ("implemented", "status must be 'planned'"),
        }
        for key, (replacement, expected) in cases.items():
            with self.subTest(key=key):
                self.mutate_json(
                    "routines/g36/scope.json",
                    lambda value, field=key, new_value=replacement: value.update(
                        {field: new_value}
                    ),
                )
                self.assert_error(expected)
                self.restore_product_file("routines/g36/scope.json")

    def test_scope_ids_sections_and_destinations_are_unique(self):
        for key in ("id", "section", "destination"):
            with self.subTest(key=key):
                def duplicate(value, field=key):
                    value["sections"][1][field] = value["sections"][0][field]

                self.mutate_json("routines/g36/scope.json", duplicate)
                self.assert_error(f"scope.json: sections[1].{key}: duplicate")
                self.restore_product_file("routines/g36/scope.json")

    def test_scope_set_order_status_and_stable_fields_are_enforced(self):
        def reverse(value):
            value["sections"].reverse()

        self.mutate_json("routines/g36/scope.json", reverse)
        self.assert_error("scope.json: sections must be ordered from 5.1 through 5.22")
        self.restore_product_file("routines/g36/scope.json")

        cases = {
            "section": ("5.99", "sections must contain exactly sections 5.1 through 5.22"),
            "id": ("G36-SCOPE-05-99", "id must be 'G36-SCOPE-05-01'"),
            "name": (" ", "name must be a nonempty trimmed string"),
            "status": ("implemented", "status must be 'planned'"),
            "source_disposition": ("unknown", "source_disposition must be 'mixed'"),
            "destination": ("g36/shared/other", "destination must be 'g36/shared/general'"),
        }
        for key, (replacement, expected) in cases.items():
            with self.subTest(key=key):
                def replace(value, field=key, new_value=replacement):
                    value["sections"][0][field] = new_value

                self.mutate_json("routines/g36/scope.json", replace)
                self.assert_error(expected)
                self.restore_product_file("routines/g36/scope.json")

    def test_scope_destinations_reject_unsafe_paths(self):
        cases = {
            "/g36/example": "absolute paths are forbidden",
            "g36\\example": "backslashes are forbidden",
            "g36/./example": "dot path segments are forbidden",
            "g36/example/../other": "parent traversal is forbidden",
            "g36//example": "empty path segments are forbidden",
            "": "empty path segments are forbidden",
            "faults/example": "path must be below g36/",
        }
        for destination, expected in cases.items():
            with self.subTest(destination=destination):
                def replace(value, new_destination=destination):
                    value["sections"][0]["destination"] = new_destination

                self.mutate_json("routines/g36/scope.json", replace)
                self.assert_error(f"sections[0].destination: {expected}")
                self.restore_product_file("routines/g36/scope.json")

    def test_coverage_must_agree_with_scope_and_make_no_claims(self):
        cases = {
            "profile": ("other", "profile must equal scope.json profile"),
            "status": ("implemented", "status must equal scope.json status"),
            "scope": ("other.json", "scope must be 'scope.json'"),
            "claims": ([{}], "claims must remain empty in L0"),
        }
        for key, (replacement, expected) in cases.items():
            with self.subTest(key=key):
                self.mutate_json(
                    "routines/g36/coverage.json",
                    lambda value, field=key, new_value=replacement: value.update(
                        {field: new_value}
                    ),
                )
                self.assert_error(expected)
                self.restore_product_file("routines/g36/coverage.json")

        self.mutate_json(
            "routines/g36/coverage.json", lambda value: value.update(claims={})
        )
        self.assert_error("coverage.json: claims must be an array")

    def test_stale_executable_and_fixed_variant_artifacts_are_rejected(self):
        self.write_text("routines/g36/future/routine.cxf.jsonld", "{}\n")
        self.assert_error("routine.cxf.jsonld: executable routine artifacts are forbidden in L0")
        shutil.rmtree(self.root / "routines/g36/future")

        self.write_text(
            "routines/g36/generic/air-economizer-high-limits/stale/card.md",
            "stale\n",
        )
        self.assert_error(
            "routines/g36/generic/air-economizer-high-limits: legacy fixed-variant path "
            "must be absent"
        )

    def test_expected_failures_have_sorted_output_and_no_traceback(self):
        self.write_text(PIN_ARTIFACTS[0], "bad\n")
        self.write_text(PIN_ARTIFACTS[1], "also-bad\n")
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_lint.main(self.root)
        self.assertEqual(result, 1)
        lines = output.getvalue().splitlines()
        self.assertEqual(lines, sorted(lines))
        self.assertEqual(len(lines), 2)
        self.assertNotIn("Traceback", output.getvalue())

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_lint.main(self.root, ["--donor-root", "elsewhere"])
        self.assertEqual(result, 2)
        self.assertEqual(output.getvalue(), "usage: routines.py\n")


if __name__ == "__main__":
    unittest.main()
