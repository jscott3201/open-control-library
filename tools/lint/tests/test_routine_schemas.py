import contextlib
import copy
import io
import json
import shutil
import tempfile
import unittest
from pathlib import Path

from tools.lint import routine_schemas


PRODUCT_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_FILES = tuple(
    f"routines/schemas/{name}" for name in routine_schemas.SCHEMA_FILES
)
FIXTURE_FILES = tuple(
    f"tools/lint/tests/fixtures/routine_schemas/{name}"
    for name in routine_schemas.FIXTURE_SCHEMAS
)


class RoutineSchemaTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        for relative_path in SCHEMA_FILES + FIXTURE_FILES:
            self.restore(relative_path)
        (self.root / "routines/g36").mkdir(parents=True)

    def tearDown(self):
        self.temporary_directory.cleanup()

    def restore(self, relative_path):
        source = PRODUCT_ROOT / relative_path
        destination = self.root / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

    def read_json(self, relative_path):
        return json.loads((self.root / relative_path).read_text(encoding="utf-8"))

    def write_json(self, relative_path, value):
        path = self.root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )

    def mutate(self, relative_path, change):
        value = self.read_json(relative_path)
        change(value)
        self.write_json(relative_path, value)

    def assert_error(self, expected):
        errors = routine_schemas.validate(self.root)
        self.assertTrue(
            any(expected in error for error in errors),
            f"{expected!r} not found in {errors!r}",
        )
        self.assertEqual(errors, sorted(errors))
        self.assertFalse(any("Traceback" in error for error in errors))
        return errors

    def test_production_schemas_and_fixture_set_are_clean_and_repeatable(self):
        self.assertEqual(routine_schemas.validate(PRODUCT_ROOT), [])
        first = routine_schemas.validate(self.root)
        second = routine_schemas.validate(self.root)
        self.assertEqual(first, [])
        self.assertEqual(second, first)

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_schemas.main(self.root)
        self.assertEqual(result, 0)
        self.assertEqual(
            output.getvalue(),
            "routine schema lint: 4 schemas, 3 synthetic fixtures OK\n",
        )

    def test_json_loader_rejects_malformed_duplicates_and_nonfinite_numbers(self):
        path = self.root / FIXTURE_FILES[2]
        original = path.read_bytes()
        cases = (
            (b"{", "invalid JSON at line 1, column 2"),
            (
                b'{"schema":"one","schema":"two"}\n',
                "duplicate object key 'schema'",
            ),
            (
                original.replace(b'"value": 3', b'"value": NaN', 1),
                "non-finite number 'NaN' is forbidden",
            ),
            (
                original.replace(b'"value": 3', b'"value": Infinity', 1),
                "non-finite number 'Infinity' is forbidden",
            ),
        )
        for content, expected in cases:
            with self.subTest(expected=expected):
                path.write_bytes(content)
                self.assert_error(expected)
                path.write_bytes(original)

    def test_governed_schema_files_ids_dialect_and_refs_are_exact(self):
        common_path = "routines/schemas/common.schema.json"
        path = self.root / common_path
        original = path.read_bytes()

        path.unlink()
        self.assert_error("common.schema.json: governed schema is missing")
        path.write_bytes(original)

        extra = self.root / "routines/schemas/extra.schema.json"
        extra.write_text("{}\n", encoding="utf-8")
        self.assert_error("extra.schema.json: unexpected schema file")
        extra.unlink()

        self.mutate(common_path, lambda value: value.update({"$id": "wrong"}))
        self.assert_error(f"$id must be {routine_schemas.COMMON_ID!r}")
        path.write_bytes(original)

        self.mutate(common_path, lambda value: value.pop("$id"))
        self.assert_error(f"$id must be {routine_schemas.COMMON_ID!r}")
        path.write_bytes(original)

        self.mutate(common_path, lambda value: value.pop("$schema"))
        self.assert_error(f"$schema must be {routine_schemas.DIALECT!r}")
        path.write_bytes(original)

        self.mutate(common_path, lambda value: value.update({"$schema": "wrong"}))
        self.assert_error(f"$schema must be {routine_schemas.DIALECT!r}")
        path.write_bytes(original)

        def external_reference(value):
            value["$defs"]["localId"] = {"$ref": "https://example.com/network.json"}

        self.mutate(common_path, external_reference)
        self.assert_error("references forbidden resource 'https://example.com/network.json'")

    def test_schema_self_check_and_local_reference_resolution_fail_closed(self):
        common_path = "routines/schemas/common.schema.json"
        interface_path = "routines/schemas/interface.schema.json"

        self.mutate(common_path, lambda value: value.update(type="not-a-json-type"))
        self.assert_error("invalid Draft 2020-12 schema")
        self.restore(common_path)

        def unresolved(value):
            value["properties"]["canonical_id"]["$ref"] = (
                f"{routine_schemas.COMMON_ID}#/$defs/notPresent"
            )

        self.mutate(interface_path, unresolved)
        self.assert_error("cannot resolve")
        self.assert_error("#/$defs/notPresent")

    def test_fixture_closed_shapes_required_keys_and_discriminators(self):
        interface_path = FIXTURE_FILES[1]
        original = (self.root / interface_path).read_bytes()
        cases = (
            (lambda value: value.update(extra=True), "Additional properties are not allowed"),
            (lambda value: value.pop("connectors"), "'connectors' is a required property"),
            (
                lambda value: value["parameters"][0]["type"].update(kind="runtime"),
                "is not valid under any of the given schemas",
            ),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                self.mutate(interface_path, mutation)
                self.assert_error(expected)
                (self.root / interface_path).write_bytes(original)

    def test_invalid_canonical_identity_forms_are_rejected(self):
        manifest_path, interface_path, specialization_path = FIXTURE_FILES
        invalid_ids = (
            "G36-SCOPE-05-16",
            "G36-05-16-ZONE-COUNT-3",
            "G36-05-16-Buildings/Controls/OBC/ASHRAE/G36/Controller",
            "G36-05-16-SHA1-ABC123",
        )
        for canonical_id in invalid_ids:
            with self.subTest(canonical_id=canonical_id):
                self.mutate(manifest_path, lambda value: value.update(id=canonical_id))
                self.mutate(
                    interface_path, lambda value: value.update(canonical_id=canonical_id)
                )
                self.mutate(
                    specialization_path,
                    lambda value: value.update(canonical_id=canonical_id),
                )
                self.assert_error("does not match")
                for path in FIXTURE_FILES:
                    self.restore(path)

    def test_section_and_cross_document_identity_revision_must_agree(self):
        manifest_path, interface_path, specialization_path = FIXTURE_FILES
        self.mutate(manifest_path, lambda value: value.update(section="5.15"))
        self.assert_error("$.section must be '5.16'")
        self.restore(manifest_path)

        self.mutate(
            interface_path,
            lambda value: value.update(canonical_id="G36-05-16-OTHER-TEST"),
        )
        self.assert_error("$.canonical_id must equal")
        self.restore(interface_path)

        self.mutate(specialization_path, lambda value: value.update(revision=2))
        self.assert_error("$.revision must equal class manifest revision")

    def test_duplicate_nested_ids_and_symbols_are_rejected(self):
        interface_path = FIXTURE_FILES[1]
        specialization_path = FIXTURE_FILES[2]
        interface_original = (self.root / interface_path).read_bytes()
        specialization_original = (self.root / specialization_path).read_bytes()

        interface_cases = (
            (
                lambda value: value["types"].append(copy.deepcopy(value["types"][0])),
                "$.types[3].id: duplicate",
            ),
            (
                lambda value: value["types"][2]["members"][1].update(id="occupied"),
                ".members[1].id: duplicate",
            ),
            (
                lambda value: value["types"][2]["members"][1].update(symbol="OCCUPIED"),
                ".members[1].symbol: duplicate",
            ),
            (
                lambda value: value["dimensions"][1].update(id="fixed_pair"),
                "$.dimensions[1].id: duplicate",
            ),
            (
                lambda value: value["parameters"][1].update(id="sample_period_s"),
                "$.parameters[1].id: duplicate",
            ),
            (
                lambda value: value["connectors"][1].update(id="zone_temperatures"),
                "$.connectors[1].id: duplicate",
            ),
        )
        for mutation, expected in interface_cases:
            with self.subTest(expected=expected):
                self.mutate(interface_path, mutation)
                self.assert_error(expected)
                (self.root / interface_path).write_bytes(interface_original)

        def duplicate_assignment(value):
            value["parameters"].append(copy.deepcopy(value["parameters"][0]))

        self.mutate(specialization_path, duplicate_assignment)
        self.assert_error("$.parameters[5].parameter: duplicate")
        (self.root / specialization_path).write_bytes(specialization_original)

        def duplicate_member(value):
            value["members"][0]["members"][1] = value["members"][0]["members"][0]

        self.mutate(specialization_path, duplicate_member)
        self.assert_error("duplicate stable member")

    def test_unknown_type_dimension_parameter_and_enum_member_are_rejected(self):
        interface_path = FIXTURE_FILES[1]
        specialization_path = FIXTURE_FILES[2]
        interface_original = (self.root / interface_path).read_bytes()
        specialization_original = (self.root / specialization_path).read_bytes()

        self.mutate(
            interface_path,
            lambda value: value["connectors"][0]["type"].update(type="missing_type"),
        )
        self.assert_error("unknown named type 'missing_type'")
        (self.root / interface_path).write_bytes(interface_original)

        self.mutate(
            interface_path,
            lambda value: value["connectors"][0]["shape"]["dimensions"].__setitem__(
                0, "missing_dimension"
            ),
        )
        self.assert_error("unknown dimension 'missing_dimension'")
        (self.root / interface_path).write_bytes(interface_original)

        self.mutate(
            interface_path,
            lambda value: value["dimensions"][1]["extent"].update(
                parameter="missing_parameter"
            ),
        )
        self.assert_error("unknown parameter 'missing_parameter'")
        (self.root / interface_path).write_bytes(interface_original)

        self.mutate(
            specialization_path,
            lambda value: value["parameters"][2].update(value="missing-member"),
        )
        self.assert_error("value must be member of enum 'operating_mode'")
        (self.root / specialization_path).write_bytes(specialization_original)

        self.mutate(
            specialization_path,
            lambda value: value["members"][0].update(dimension="missing_dimension"),
        )
        self.assert_error("unknown dimension 'missing_dimension'")

    def test_dimension_contract_rejects_wrong_parameter_zero_extent_and_rank(self):
        interface_path = FIXTURE_FILES[1]
        original = (self.root / interface_path).read_bytes()

        self.mutate(
            interface_path,
            lambda value: value["parameters"][2]["type"].update(primitive="boolean"),
        )
        self.assert_error("dimension parameter 'zone_count' must resolve to Integer")
        (self.root / interface_path).write_bytes(original)

        self.mutate(
            interface_path,
            lambda value: value["parameters"][2].update(
                shape={"kind": "array", "dimensions": ["fixed_pair"]}
            ),
        )
        self.assert_error("dimension parameter 'zone_count' must be scalar")
        (self.root / interface_path).write_bytes(original)

        self.mutate(
            interface_path,
            lambda value: value["dimensions"][0]["extent"].update(value=0),
        )
        self.assert_error("is not valid under any of the given schemas")
        (self.root / interface_path).write_bytes(original)

        self.mutate(
            interface_path,
            lambda value: value["parameters"][6]["shape"].update(
                dimensions=["zones", "fixed_pair", "zones"]
            ),
        )
        self.assert_error("is not valid under any of the given schemas")

    def test_guards_reject_bad_operators_runtime_operands_and_type_errors(self):
        interface_path = FIXTURE_FILES[1]
        original = (self.root / interface_path).read_bytes()

        def first_comparison(value):
            return value["connectors"][2]["presence"]["guard"]["operands"][0]

        cases = (
            (
                lambda value: first_comparison(value).update(op="xor"),
                "is not valid under any of the given schemas",
            ),
            (
                lambda value: first_comparison(value).update(
                    left={"kind": "connector", "connector": "trim_request"}
                ),
                "is not valid under any of the given schemas",
            ),
            (
                lambda value: first_comparison(value)["left"].update(
                    parameter="missing_parameter"
                ),
                "unknown guard parameter 'missing_parameter'",
            ),
            (
                lambda value: first_comparison(value).update(
                    right={
                        "kind": "literal",
                        "type": {"kind": "primitive", "primitive": "integer"},
                        "value": 1,
                    }
                ),
                "guard operands have incompatible types",
            ),
            (
                lambda value: first_comparison(value).update(op="gt"),
                "ordering comparison requires a numeric operand",
            ),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                self.mutate(interface_path, mutation)
                self.assert_error(expected)
                (self.root / interface_path).write_bytes(original)

    def test_specialization_rejects_fixed_missing_and_wrong_typed_values(self):
        specialization_path = FIXTURE_FILES[2]
        original = (self.root / specialization_path).read_bytes()

        cases = (
            (
                lambda value: value["parameters"].append(
                    {"parameter": "sample_period_s", "value": 30.0}
                ),
                "fixed parameter 'sample_period_s' cannot be overridden",
            ),
            (
                lambda value: value["parameters"].__setitem__(
                    slice(None),
                    [
                        row
                        for row in value["parameters"]
                        if row["parameter"] != "zone_offsets"
                    ],
                ),
                "configurable parameter 'zone_offsets' requires a value",
            ),
            (
                lambda value: value["parameters"][0].update(value="three"),
                "value must be integer",
            ),
            (
                lambda value: value["parameters"][1].update(value=1),
                "value must be boolean",
            ),
            (
                lambda value: value["parameters"][2].update(value="invalid-mode"),
                "value must be member of enum 'operating_mode'",
            ),
            (
                lambda value: value["parameters"][0].update(value=9),
                "value 9 exceeds maximum 8",
            ),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                self.mutate(specialization_path, mutation)
                self.assert_error(expected)
                (self.root / specialization_path).write_bytes(original)

    def test_vector_matrix_and_stable_member_extents_are_enforced(self):
        interface_path = FIXTURE_FILES[1]
        specialization_path = FIXTURE_FILES[2]
        interface_original = (self.root / interface_path).read_bytes()
        specialization_original = (self.root / specialization_path).read_bytes()

        self.mutate(
            interface_path,
            lambda value: value["parameters"][1].update(default=[1.0]),
        )
        self.assert_error("dimension 0 length must be 2, found 1")
        (self.root / interface_path).write_bytes(interface_original)

        self.mutate(
            specialization_path,
            lambda value: value["parameters"][3].update(value=[0.0, 0.5]),
        )
        self.assert_error("dimension 0 length must be 3, found 2")
        (self.root / specialization_path).write_bytes(specialization_original)

        def ragged(value):
            value["parameters"][4]["value"][1] = [0.5]

        self.mutate(specialization_path, ragged)
        self.assert_error("dimension 1 length must be 2, found 1")
        (self.root / specialization_path).write_bytes(specialization_original)

        self.mutate(
            specialization_path,
            lambda value: value["parameters"][4].update(value=[[1.0, 0.0]]),
        )
        self.assert_error("dimension 0 length must be 3, found 1")
        (self.root / specialization_path).write_bytes(specialization_original)

        self.mutate(
            specialization_path,
            lambda value: value["members"][0].update(
                members=["north-zone", "south-zone"]
            ),
        )
        self.assert_error("expected 3 members, found 2")

    def test_manifest_paths_and_source_discriminator_are_closed(self):
        manifest_path = FIXTURE_FILES[0]
        original = (self.root / manifest_path).read_bytes()

        self.mutate(
            manifest_path,
            lambda value: value["source"].update(kind="hybrid"),
        )
        self.assert_error("is not valid under any of the given schemas")
        (self.root / manifest_path).write_bytes(original)

        self.mutate(
            manifest_path,
            lambda value: value["artifacts"].update(
                interface="test-only/../interface.json"
            ),
        )
        self.assert_error("parent traversal is forbidden")
        (self.root / manifest_path).write_bytes(original)

        self.mutate(
            manifest_path,
            lambda value: value["artifacts"].update(
                specialization_config="another-test-class/specialization.json"
            ),
        )
        self.assert_error("paths must share one non-root class directory")
        (self.root / manifest_path).write_bytes(original)

        def duplicate_source(value):
            value["source"]["files"].append(copy.deepcopy(value["source"]["files"][0]))

        self.mutate(manifest_path, duplicate_source)
        self.assert_error("$.source.files[1].path: duplicate")

    def test_manifest_provenance_union_accepts_development_and_independent_sources(self):
        manifest_path = FIXTURE_FILES[0]

        self.mutate(
            manifest_path,
            lambda value: value["source"].update(
                snapshot="development",
                revision="3333333333333333333333333333333333333333",
            ),
        )
        self.assertEqual(routine_schemas.validate(self.root), [])
        self.restore(manifest_path)

        self.mutate(
            manifest_path,
            lambda value: value.update(
                source={
                    "kind": "independent",
                    "paths": ["test-only/independent/schema-fixture.md"],
                }
            ),
        )
        self.assertEqual(routine_schemas.validate(self.root), [])

    def test_dense_numeric_stable_member_id_is_rejected(self):
        specialization_path = FIXTURE_FILES[2]
        self.mutate(
            specialization_path,
            lambda value: value["members"][0]["members"].__setitem__(0, "1"),
        )
        self.assert_error("does not match")

    def test_schema_only_fixture_artifacts_are_forbidden_in_production_g36(self):
        destination = self.root / "routines/g36/test-only/interface.json"
        destination.parent.mkdir(parents=True)
        shutil.copyfile(self.root / FIXTURE_FILES[1], destination)
        self.assert_error(
            "routines/g36/test-only/interface.json: schema-only fixture artifact is forbidden"
        )

    def test_cli_errors_are_sorted_and_have_no_traceback(self):
        interface_path = FIXTURE_FILES[1]

        def introduce_errors(value):
            value["parameters"][1]["id"] = value["parameters"][0]["id"]
            value["connectors"][0]["type"] = {
                "kind": "named",
                "type": "missing_type",
            }

        self.mutate(interface_path, introduce_errors)
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_schemas.main(self.root)
        self.assertEqual(result, 1)
        lines = output.getvalue().splitlines()
        self.assertEqual(lines, sorted(lines))
        self.assertNotIn("Traceback", output.getvalue())

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_schemas.main(self.root, ["--network"])
        self.assertEqual(result, 2)
        self.assertEqual(output.getvalue(), "usage: routine_schemas.py\n")


if __name__ == "__main__":
    unittest.main()
