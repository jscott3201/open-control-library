import contextlib
import copy
import io
import json
import shutil
import socket
import tempfile
import unittest
import urllib.request
from pathlib import Path
from unittest import mock

from tools.lint import point_semantics


PRODUCT_ROOT = Path(__file__).resolve().parents[3]
PIN_PATH = "routines/ontology/ontology-pins.json"
POINT_FILES = tuple(
    path.relative_to(PRODUCT_ROOT).as_posix()
    for path in sorted((PRODUCT_ROOT / "points").glob("*.points.json"))
)


class PointSemanticLintTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        for relative_path in (PIN_PATH,) + POINT_FILES:
            self.restore(relative_path)

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

    def point(self, document, name):
        return next(point for point in document["points"] if point["name"] == name)

    def mutate_point(self, relative_path, name, change):
        def apply(document):
            change(self.point(document, name))

        self.mutate(relative_path, apply)

    def assert_error(self, expected):
        errors = point_semantics.validate(self.root)
        self.assertTrue(
            any(expected in error for error in errors),
            f"{expected!r} not found in {errors!r}",
        )
        self.assertEqual(errors, sorted(errors))
        self.assertFalse(any("Traceback" in error for error in errors))
        return errors

    def test_production_corpus_is_clean_repeatable_and_reports_actual_counts(self):
        self.assertEqual(point_semantics.validate(PRODUCT_ROOT), [])
        first = point_semantics.validate(self.root)
        second = point_semantics.validate(self.root)
        self.assertEqual(first, [])
        self.assertEqual(second, first)

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = point_semantics.main(self.root)
        self.assertEqual(result, 0)
        self.assertEqual(
            output.getvalue(),
            f"point semantic lint: {len(POINT_FILES)} dictionaries, "
            f"{len(point_semantics.REVIEWED_MAPPING_EXPECTATIONS)} bounded reviewed "
            "mappings OK\n",
        )

    def test_inventory_is_globbed_without_global_point_name_uniqueness(self):
        copied = self.read_json("points/ahu.points.json")
        copied["equipment"] = "copy"
        self.write_json("points/copy.points.json", copied)
        self.assertEqual(point_semantics.validate(self.root), [])

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = point_semantics.main(self.root)
        self.assertEqual(result, 0)
        self.assertIn(f"{len(POINT_FILES) + 1} dictionaries", output.getvalue())

    def test_json_loader_rejects_malformed_duplicate_nonfinite_and_non_utf8_input(self):
        relative_path = "points/ahu.points.json"
        path = self.root / relative_path
        cases = (
            (b"{", "invalid JSON at line 1, column 2"),
            (b'{"schema":"one","schema":"two"}\n', "duplicate object key 'schema'"),
            (b'{"schema":NaN}\n', "non-finite number 'NaN' is forbidden"),
            (b'{"schema":Infinity}\n', "non-finite number 'Infinity' is forbidden"),
            (b'{"schema":-Infinity}\n', "non-finite number '-Infinity' is forbidden"),
            (b"\xff", "file is not UTF-8"),
            (b"[]\n", "must contain a JSON object"),
        )
        for content, expected in cases:
            with self.subTest(expected=expected):
                path.write_bytes(content)
                self.assert_error(f"{relative_path}: {expected}")
                self.restore(relative_path)

    def test_pin_authority_uses_the_same_strict_local_json_loader(self):
        path = self.root / PIN_PATH
        original = path.read_bytes()
        cases = (
            (b"{", "invalid JSON at line 1, column 2"),
            (b'{"schema":"one","schema":"two"}\n', "duplicate object key 'schema'"),
            (b'{"schema":NaN}\n', "non-finite number 'NaN' is forbidden"),
            (b"\xff", "file is not UTF-8"),
        )
        for content, expected in cases:
            with self.subTest(expected=expected):
                path.write_bytes(content)
                self.assert_error(f"{PIN_PATH}: {expected}")
                path.write_bytes(original)

        path.unlink()
        self.assert_error(f"{PIN_PATH}: file is missing")

    def test_top_level_contract_rejects_missing_unexpected_and_wrong_typed_values(self):
        relative_path = "points/ahu.points.json"

        self.mutate(relative_path, lambda value: value.pop("schema"))
        self.assert_error(f"{relative_path}: missing required key 'schema'")
        self.restore(relative_path)

        self.mutate(relative_path, lambda value: value.update(extra=True))
        self.assert_error(f"{relative_path}: unexpected key 'extra'")
        self.restore(relative_path)

        self.mutate(relative_path, lambda value: value.update(points={}))
        self.assert_error(f"{relative_path}: points must be an array")
        self.restore(relative_path)

        self.mutate(relative_path, lambda value: value.update(namespaces=[]))
        self.assert_error(f"{relative_path}: namespaces must be an object")

    def test_filename_equipment_schema_and_point_names_are_enforced(self):
        relative_path = "points/ahu.points.json"

        self.mutate(relative_path, lambda value: value.update(schema="wrong"))
        self.assert_error(
            f"{relative_path}: schema must be {point_semantics.POINT_SCHEMA!r}"
        )
        self.restore(relative_path)

        self.mutate(relative_path, lambda value: value.update(equipment="rtu"))
        self.assert_error("equipment must match filename stem 'ahu'")
        self.restore(relative_path)

        def duplicate(document):
            document["points"][1]["name"] = document["points"][0]["name"]

        self.mutate(relative_path, duplicate)
        self.assert_error("points[1].name: duplicate 'sat'; first used at index 0")

    def test_point_records_require_existing_v1_fields_and_types(self):
        relative_path = "points/ahu.points.json"

        self.mutate_point(relative_path, "sat", lambda point: point.pop("description"))
        self.assert_error("points[0]: missing required key 'description'")
        self.restore(relative_path)

        self.mutate_point(relative_path, "sat", lambda point: point.update(extra=True))
        self.assert_error("points[0]: unexpected key 'extra'")
        self.restore(relative_path)

        for bad_kind in ("number", []):
            with self.subTest(bad_kind=bad_kind):
                self.mutate_point(
                    relative_path,
                    "sat",
                    lambda point, value=bad_kind: point.update(kind=value),
                )
                self.assert_error("points[0].kind: must be 'real', 'int', or 'bool'")
                self.restore(relative_path)

        self.mutate_point(relative_path, "sat", lambda point: point.update(derived="yes"))
        self.assert_error("points[0].derived: must be a boolean")
        self.restore(relative_path)

        self.mutate(relative_path, lambda value: value["points"].__setitem__(0, "sat"))
        self.assert_error("points[0]: must be an object")

    def test_namespace_iris_and_version_echoes_follow_the_local_pin(self):
        relative_path = "points/ahu.points.json"
        cases = (
            ("brick", "iri", "wrong", "namespaces.brick.iri: must equal ontology pin"),
            (
                "s223",
                "verified_version",
                "stale",
                "namespaces.s223.verified_version: must equal ontology pin echo",
            ),
            (
                "s223_g36",
                "iri",
                "wrong",
                "namespaces.s223_g36.iri: must equal ontology pin",
            ),
            (
                "quantitykind",
                "verified_version",
                "stale",
                "namespaces.quantitykind.verified_version: must equal ontology pin echo",
            ),
            ("unit", "iri", "wrong", "namespaces.unit.iri: must equal ontology pin"),
        )
        for namespace, field, replacement, expected in cases:
            with self.subTest(namespace=namespace, field=field):
                def alter(document, ns=namespace, key=field, value=replacement):
                    document["namespaces"][ns][key] = value

                self.mutate(relative_path, alter)
                self.assert_error(expected)
                self.restore(relative_path)

    def test_g36_namespace_is_optional_and_sys_without_it_is_valid(self):
        sys_dictionary = self.read_json("points/sys.points.json")
        self.assertNotIn("s223_g36", sys_dictionary["namespaces"])
        self.assertEqual(point_semantics.validate(self.root), [])

        self.mutate(
            "points/vav.points.json",
            lambda value: value["namespaces"].pop("s223_g36"),
        )
        self.assertEqual(point_semantics.validate(self.root), [])

    def test_property_classes_accept_corpus_variants_but_reject_enumerated_property(self):
        relative_path = "points/sys.points.json"
        self.mutate_point(
            relative_path,
            "lighting_status",
            lambda point: point["s223"].update(property_class="EnumeratedProperty"),
        )
        self.assert_error("s223.property_class: EnumeratedProperty is forbidden")
        self.restore(relative_path)

        self.mutate_point(
            relative_path,
            "lighting_status",
            lambda point: point["s223"].update(property_class=[]),
        )
        self.assert_error("s223.property_class: must be one of")
        self.restore(relative_path)

        self.mutate_point(
            relative_path,
            "lighting_status",
            lambda point: point["s223"].update(property_class="EnumerableProperty"),
        )
        self.assertEqual(point_semantics.validate(self.root), [])

    def test_command_and_setpoint_tokens_require_matching_actuatable_classes(self):
        relative_path = "points/ahu.points.json"
        self.mutate_point(
            relative_path,
            "htg_vlv_cmd",
            lambda point: point["s223"].update(
                property_class="QuantifiableObservableProperty"
            ),
        )
        self.assert_error("'htg_vlv_cmd' must use QuantifiableActuatableProperty")
        self.restore(relative_path)

        self.mutate_point(
            relative_path,
            "sat_sp",
            lambda point: point["s223"].update(
                property_class="QuantifiableObservableProperty"
            ),
        )
        self.assert_error("'sat_sp' must use QuantifiableActuatableProperty")
        self.restore(relative_path)

        self.assertEqual(point_semantics.validate(self.root), [])
        self.assertEqual(
            self.point(self.read_json(relative_path), "sf_speed")["s223"]["property_class"],
            "QuantifiableObservableProperty",
        )

    def test_null_roles_and_derived_records_are_exempt_from_direction_inference(self):
        self.assertIsNone(
            self.point(self.read_json("points/ahu.points.json"), "actuator_cmd")["s223"]
        )
        self.mutate_point(
            "points/ahu.points.json",
            "clg_vlv_baseline",
            lambda point: point.update(name="derived_cmd"),
        )
        self.assertEqual(point_semantics.validate(self.root), [])

    def test_quantifiable_actuatable_mappings_require_quantity_and_matching_units(self):
        relative_path = "points/ahu.points.json"

        self.mutate_point(
            relative_path,
            "htg_vlv_cmd",
            lambda point: point["s223"].update(quantitykind=None),
        )
        self.assert_error(
            "QuantifiableActuatableProperty requires a nonempty quantitykind"
        )
        self.restore(relative_path)

        self.mutate_point(
            relative_path,
            "htg_vlv_cmd",
            lambda point: point["s223"].update(unit=None),
        )
        self.assert_error("QuantifiableActuatableProperty requires a nonempty unit")
        self.restore(relative_path)

        self.mutate_point(
            relative_path,
            "htg_vlv_cmd",
            lambda point: point["s223"].update(unit="DEG_C"),
        )
        self.assert_error("s223.unit: must equal the point qudt_unit")

    def test_every_bounded_reviewed_mapping_field_is_enforced(self):
        for (relative_path, name), expected in sorted(
            point_semantics.REVIEWED_MAPPING_EXPECTATIONS.items()
        ):
            for field in (
                "qudt_unit",
                "property_class",
                "quantitykind",
                "unit",
                "medium",
                "aspects",
            ):
                with self.subTest(relative_path=relative_path, name=name, field=field):
                    if field == "qudt_unit":
                        self.mutate_point(
                            relative_path,
                            name,
                            lambda point: point.update(qudt_unit="WRONG"),
                        )
                    else:
                        replacement = {
                            "property_class": "QuantifiableProperty",
                            "quantitykind": "WrongQuantityKind",
                            "unit": "WRONG-UNIT",
                            "medium": (
                                "Fluid-Water"
                                if expected["medium"] is None
                                else None
                            ),
                            "aspects": (
                                ["Aspect-Delta"]
                                if expected["aspects"] != ["Aspect-Delta"]
                                else []
                            ),
                        }[field]
                        self.mutate_point(
                            relative_path,
                            name,
                            lambda point, key=field, value=replacement: point[
                                "s223"
                            ].__setitem__(key, copy.deepcopy(value)),
                        )
                    expected_path = (
                        f"{relative_path}#{name}: qudt_unit must be"
                        if field == "qudt_unit"
                        else f"{relative_path}#{name}: s223.{field} must be"
                    )
                    self.assert_error(expected_path)
                    self.restore(relative_path)

    def test_cli_errors_are_sorted_without_traceback_and_usage_is_closed(self):
        self.mutate(
            "points/ahu.points.json",
            lambda value: value["namespaces"]["brick"].update(
                iri="wrong", verified_version="stale"
            ),
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = point_semantics.main(self.root)
        self.assertEqual(result, 1)
        lines = output.getvalue().splitlines()
        self.assertEqual(lines, sorted(lines))
        self.assertNotIn("Traceback", output.getvalue())

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = point_semantics.main(self.root, ["--network"])
        self.assertEqual(result, 2)
        self.assertEqual(output.getvalue(), "usage: point_semantics.py\n")

    def test_validation_does_not_open_network_sockets_or_urls(self):
        with mock.patch.object(
            socket, "socket", side_effect=AssertionError("network socket opened")
        ), mock.patch.object(
            urllib.request,
            "urlopen",
            side_effect=AssertionError("network URL opened"),
        ):
            self.assertEqual(point_semantics.validate(PRODUCT_ROOT), [])


if __name__ == "__main__":
    unittest.main()
