import copy
import json
import math
import unittest
from dataclasses import FrozenInstanceError, fields, is_dataclass
from pathlib import Path
from unittest import mock

from tools.lint import routine_resolution, routine_scalar_abi


FIXTURE_ROOT = Path(__file__).parent / "fixtures" / "routine_schemas"


class RoutineScalarAbiTests(unittest.TestCase):
    def setUp(self):
        self.interface = json.loads(
            (FIXTURE_ROOT / "interface.json").read_text(encoding="utf-8")
        )
        self.specialization = json.loads(
            (FIXTURE_ROOT / "specialization.json").read_text(encoding="utf-8")
        )

    @staticmethod
    def by_id(rows, field, value):
        return next(row for row in rows if getattr(row, field) == value)

    @staticmethod
    def document_by_id(rows, value):
        return next(row for row in rows if row["id"] == value)

    @staticmethod
    def assignment(specialization, parameter_id):
        return next(
            row
            for row in specialization["parameters"]
            if row["parameter"] == parameter_id
        )

    def resolve(self, interface, specialization):
        return routine_resolution.resolve_specialization(interface, specialization)

    def primitive_documents(self):
        interface = copy.deepcopy(self.interface)
        specialization = copy.deepcopy(self.specialization)
        initial_mode = self.document_by_id(interface["parameters"], "initial_mode")
        initial_mode["type"] = {"kind": "primitive", "primitive": "integer"}
        initial_mode["default"] = -4
        self.assignment(specialization, "initial_mode")["value"] = -3
        trim_request = self.document_by_id(interface["connectors"], "trim_request")
        trim_request["presence"] = {
            "kind": "when",
            "guard": {
                "op": "eq",
                "left": {"kind": "parameter", "parameter": "enable_trim"},
                "right": {
                    "kind": "literal",
                    "type": {"kind": "primitive", "primitive": "boolean"},
                    "value": True,
                },
            },
        }
        return interface, specialization

    @staticmethod
    def add_matrix_connector(interface):
        interface["connectors"].append(
            {
                "id": "matrix_feedback",
                "direction": "output",
                "type": {"kind": "primitive", "primitive": "real"},
                "shape": {
                    "kind": "array",
                    "dimensions": ["zones", "fixed_pair"],
                },
                "presence": {"kind": "always"},
            }
        )

    @staticmethod
    def add_enum_connector(interface, presence):
        interface["connectors"].append(
            {
                "id": "mode_signal",
                "direction": "input",
                "type": {"kind": "named", "type": "operating_mode"},
                "shape": {"kind": "array", "dimensions": ["fixed_pair"]},
                "presence": presence,
            }
        )

    def test_scalar_vector_and_matrix_leaves_keep_resolver_order(self):
        interface, specialization = self.primitive_documents()
        self.add_matrix_connector(interface)
        result = routine_scalar_abi.project_scalar_abi(
            self.resolve(interface, specialization)
        )

        self.assertEqual(result.canonical_id, "G36-05-16-SYNTHETIC-SCHEMA-TEST")
        self.assertEqual(result.revision, 1)
        self.assertEqual(
            [row.parameter_id for row in result.parameters],
            [
                "sample_period_s",
                "fixed_gains",
                "fixed_gains",
                "zone_count",
                "enable_trim",
                "initial_mode",
                "optional_gain",
                "zone_offsets",
                "zone_offsets",
                "zone_offsets",
                "matrix_weights",
                "matrix_weights",
                "matrix_weights",
                "matrix_weights",
                "matrix_weights",
                "matrix_weights",
            ],
        )
        sample_period = self.by_id(result.parameters, "parameter_id", "sample_period_s")
        self.assertEqual(sample_period.coordinates, ())
        fixed_gains = [
            row for row in result.parameters if row.parameter_id == "fixed_gains"
        ]
        self.assertEqual(
            [(row.coordinates[0].member_id, row.value) for row in fixed_gains],
            [("primary", 1.0), ("secondary", 0.5)],
        )
        matrix = [
            row for row in result.parameters if row.parameter_id == "matrix_weights"
        ]
        self.assertEqual(
            [
                (
                    tuple(coordinate.member_id for coordinate in row.coordinates),
                    row.value,
                )
                for row in matrix
            ],
            [
                (("north-zone", "primary"), 1.0),
                (("north-zone", "secondary"), 0.0),
                (("south-zone", "primary"), 0.5),
                (("south-zone", "secondary"), 0.5),
                (("core-zone", "primary"), 0.0),
                (("core-zone", "secondary"), 1.0),
            ],
        )

        self.assertEqual(
            [row.connector_id for row in result.connectors],
            [
                "zone_temperatures",
                "zone_temperatures",
                "zone_temperatures",
                "supply_air_flow",
                "zone_commands",
                "zone_commands",
                "zone_commands",
                "matrix_feedback",
                "matrix_feedback",
                "matrix_feedback",
                "matrix_feedback",
                "matrix_feedback",
                "matrix_feedback",
            ],
        )
        matrix_feedback = [
            row for row in result.connectors if row.connector_id == "matrix_feedback"
        ]
        self.assertEqual(
            [
                tuple(
                    (coordinate.dimension_id, coordinate.member_id, coordinate.ordinal)
                    for coordinate in row.coordinates
                )
                for row in matrix_feedback
            ],
            [
                (("zones", "north-zone", 0), ("fixed_pair", "primary", 0)),
                (("zones", "north-zone", 0), ("fixed_pair", "secondary", 1)),
                (("zones", "south-zone", 1), ("fixed_pair", "primary", 0)),
                (("zones", "south-zone", 1), ("fixed_pair", "secondary", 1)),
                (("zones", "core-zone", 2), ("fixed_pair", "primary", 0)),
                (("zones", "core-zone", 2), ("fixed_pair", "secondary", 1)),
            ],
        )

    def test_authored_member_reordering_keeps_row_major_association(self):
        interface, specialization = self.primitive_documents()
        interface["dimensions"][0]["members"] = ["secondary", "primary"]
        specialization["members"][0]["members"] = [
            "south-zone",
            "north-zone",
            "core-zone",
        ]
        result = routine_scalar_abi.project_scalar_abi(
            self.resolve(interface, specialization)
        )
        matrix = [
            row for row in result.parameters if row.parameter_id == "matrix_weights"
        ]
        self.assertEqual(
            [
                (
                    tuple(coordinate.member_id for coordinate in row.coordinates),
                    tuple(coordinate.ordinal for coordinate in row.coordinates),
                    row.value,
                )
                for row in matrix
            ],
            [
                (("south-zone", "secondary"), (0, 0), 1.0),
                (("south-zone", "primary"), (0, 1), 0.0),
                (("north-zone", "secondary"), (1, 0), 0.5),
                (("north-zone", "primary"), (1, 1), 0.5),
                (("core-zone", "secondary"), (2, 0), 0.0),
                (("core-zone", "primary"), (2, 1), 1.0),
            ],
        )

    def test_primitive_values_alias_metadata_sources_and_directions_are_exact(self):
        interface, specialization = self.primitive_documents()
        interface["types"].extend(
            [
                {"id": "signed_count", "kind": "alias", "primitive": "integer"},
                {"id": "latched_flag", "kind": "alias", "primitive": "boolean"},
            ]
        )
        interface["parameters"].extend(
            [
                {
                    "id": "count_bias",
                    "type": {"kind": "named", "type": "signed_count"},
                    "shape": {"kind": "scalar"},
                    "configurability": "fixed",
                    "default": -2,
                },
                {
                    "id": "latch_enabled",
                    "type": {"kind": "named", "type": "latched_flag"},
                    "shape": {"kind": "scalar"},
                    "configurability": "fixed",
                    "default": False,
                },
            ]
        )
        self.document_by_id(interface["parameters"], "optional_gain")["default"] = -0.0
        result = routine_scalar_abi.project_scalar_abi(
            self.resolve(interface, specialization)
        )

        sample_period = self.by_id(result.parameters, "parameter_id", "sample_period_s")
        self.assertEqual(sample_period.type, routine_scalar_abi.ScalarAbiType("real"))
        self.assertIs(type(sample_period.value), float)
        zone_count = self.by_id(result.parameters, "parameter_id", "zone_count")
        self.assertEqual(zone_count.type, routine_scalar_abi.ScalarAbiType("integer"))
        self.assertIs(type(zone_count.value), int)
        self.assertEqual(zone_count.source, "assignment")
        enable_trim = self.by_id(result.parameters, "parameter_id", "enable_trim")
        self.assertEqual(enable_trim.type, routine_scalar_abi.ScalarAbiType("boolean"))
        self.assertIs(type(enable_trim.value), bool)
        self.assertIs(enable_trim.value, False)
        initial_mode = self.by_id(result.parameters, "parameter_id", "initial_mode")
        self.assertEqual(initial_mode.value, -3)
        self.assertIs(type(initial_mode.value), int)
        fixed_gain = self.by_id(result.parameters, "parameter_id", "fixed_gains")
        self.assertEqual(fixed_gain.source, "default")

        zone_offset = self.by_id(result.parameters, "parameter_id", "zone_offsets")
        self.assertEqual(
            zone_offset.type,
            routine_scalar_abi.ScalarAbiType(
                primitive="real",
                alias_type_id="temperature",
                quantity="thermodynamic_temperature",
                unit="K",
                display_unit="degC",
            ),
        )
        supply_air_flow = self.by_id(
            result.connectors, "connector_id", "supply_air_flow"
        )
        self.assertEqual(
            supply_air_flow.type,
            routine_scalar_abi.ScalarAbiType(
                primitive="real",
                alias_type_id="air_flow",
                quantity="volume_flow_rate",
                unit="m3/s",
            ),
        )
        self.assertEqual(supply_air_flow.direction, "input")
        zone_command = self.by_id(result.connectors, "connector_id", "zone_commands")
        self.assertEqual(zone_command.direction, "output")

        count_bias = self.by_id(result.parameters, "parameter_id", "count_bias")
        self.assertEqual(
            count_bias.type,
            routine_scalar_abi.ScalarAbiType(
                primitive="integer", alias_type_id="signed_count"
            ),
        )
        self.assertEqual(count_bias.value, -2)
        self.assertIs(type(count_bias.value), int)
        latch_enabled = self.by_id(result.parameters, "parameter_id", "latch_enabled")
        self.assertEqual(
            latch_enabled.type,
            routine_scalar_abi.ScalarAbiType(
                primitive="boolean", alias_type_id="latched_flag"
            ),
        )
        self.assertIs(latch_enabled.value, False)
        optional_gain = self.by_id(result.parameters, "parameter_id", "optional_gain")
        self.assertEqual(math.copysign(1.0, optional_gain.value), -1.0)

    def test_inactive_connector_is_omitted_and_active_guard_result_is_consumed(self):
        interface, specialization = self.primitive_documents()
        inactive = routine_scalar_abi.project_scalar_abi(
            self.resolve(interface, specialization)
        )
        self.assertNotIn(
            "trim_request", [row.connector_id for row in inactive.connectors]
        )

        self.assignment(specialization, "enable_trim")["value"] = True
        resolved = self.resolve(interface, specialization)
        with mock.patch.object(
            routine_resolution,
            "_evaluate_guard",
            side_effect=AssertionError("guard re-evaluation"),
        ):
            active = routine_scalar_abi.project_scalar_abi(resolved)
        trim_rows = [
            row for row in active.connectors if row.connector_id == "trim_request"
        ]
        self.assertEqual(len(trim_rows), 1)
        self.assertEqual(trim_rows[0].coordinates, ())
        self.assertEqual(trim_rows[0].direction, "input")
        self.assertEqual(trim_rows[0].type, routine_scalar_abi.ScalarAbiType("boolean"))

    def test_enum_parameter_and_active_connector_have_sorted_diagnostics(self):
        interface = copy.deepcopy(self.interface)
        specialization = copy.deepcopy(self.specialization)
        self.add_enum_connector(interface, {"kind": "always"})
        resolved = self.resolve(interface, specialization)

        results = []
        for _ in range(2):
            with self.assertRaises(routine_scalar_abi.ScalarAbiError) as caught:
                routine_scalar_abi.project_scalar_abi(resolved)
            error = caught.exception
            self.assertEqual(error.diagnostics, tuple(sorted(error.diagnostics)))
            self.assertEqual(
                [
                    (
                        diagnostic.code,
                        diagnostic.owner_kind,
                        diagnostic.owner_id,
                        diagnostic.type_id,
                    )
                    for diagnostic in error.diagnostics
                ],
                [
                    (
                        "unsupported_enum",
                        "connector",
                        "mode_signal",
                        "operating_mode",
                    ),
                    (
                        "unsupported_enum",
                        "parameter",
                        "initial_mode",
                        "operating_mode",
                    ),
                ],
            )
            self.assertIn("connector mode_signal", str(error))
            self.assertIn("parameter initial_mode", str(error))
            self.assertIn("enum type 'operating_mode'", str(error))
            results.append(error.diagnostics)
        self.assertEqual(results[0], results[1])

    def test_inactive_enum_connector_produces_no_row_or_diagnostic(self):
        interface, specialization = self.primitive_documents()
        presence = copy.deepcopy(
            self.document_by_id(interface["connectors"], "trim_request")["presence"]
        )
        self.add_enum_connector(interface, presence)
        resolved = self.resolve(interface, specialization)
        mode_signal = self.by_id(resolved.connectors, "connector_id", "mode_signal")
        self.assertFalse(mode_signal.active)
        self.assertEqual(mode_signal.leaves, ())

        result = routine_scalar_abi.project_scalar_abi(resolved)
        self.assertNotIn("mode_signal", [row.connector_id for row in result.connectors])

    def test_projection_is_repeatable_frozen_detached_and_has_no_io(self):
        interface, specialization = self.primitive_documents()
        resolved = self.resolve(interface, specialization)
        resolved_before = copy.deepcopy(resolved)
        with mock.patch("builtins.open", side_effect=AssertionError("file access")), mock.patch(
            "pathlib.Path.open", side_effect=AssertionError("path access")
        ), mock.patch(
            "socket.socket", side_effect=AssertionError("network access")
        ), mock.patch(
            "urllib.request.urlopen", side_effect=AssertionError("URL access")
        ):
            first = routine_scalar_abi.project_scalar_abi(resolved)
            second = routine_scalar_abi.project_scalar_abi(resolved)
        self.assertEqual(first, second)
        self.assertEqual(resolved, resolved_before)
        self.assertIsInstance(first.parameters, tuple)
        self.assertIsInstance(first.connectors, tuple)
        with self.assertRaises(FrozenInstanceError):
            setattr(first, "revision", 2)
        with self.assertRaises(FrozenInstanceError):
            setattr(first.parameters[0], "source", "changed")
        zone_offset = self.by_id(first.parameters, "parameter_id", "zone_offsets")
        with self.assertRaises(FrozenInstanceError):
            setattr(zone_offset.coordinates[0], "member_id", "changed")

        resolved_zone_offsets = self.by_id(
            resolved.parameters, "parameter_id", "zone_offsets"
        )
        object.__setattr__(resolved_zone_offsets.type, "quantity", "changed")
        object.__setattr__(
            resolved_zone_offsets.leaves[0].coordinates[0], "member_id", "changed"
        )
        object.__setattr__(resolved_zone_offsets.leaves[0], "value", 99.0)
        self.assertEqual(zone_offset.type.quantity, "thermodynamic_temperature")
        self.assertEqual(zone_offset.coordinates[0].member_id, "north-zone")
        self.assertEqual(zone_offset.value, 0.0)

        def assert_no_mutable_values(value):
            self.assertNotIsInstance(value, (dict, list, set))
            if is_dataclass(value):
                for field in fields(value):
                    assert_no_mutable_values(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    assert_no_mutable_values(item)

        assert_no_mutable_values(first)

    def test_output_fields_exclude_scalar_names_and_deferred_concepts(self):
        interface, specialization = self.primitive_documents()
        result = routine_scalar_abi.project_scalar_abi(
            self.resolve(interface, specialization)
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_scalar_abi.ScalarAbiProjection)),
            ("canonical_id", "revision", "parameters", "connectors"),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_scalar_abi.ScalarParameterAbiRow)),
            ("parameter_id", "coordinates", "type", "source", "value"),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_scalar_abi.ScalarConnectorAbiRow)),
            ("connector_id", "coordinates", "type", "direction"),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_scalar_abi.ScalarCoordinate)),
            ("dimension_id", "member_id", "ordinal"),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_scalar_abi.ScalarAbiType)),
            ("primitive", "alias_type_id", "quantity", "unit", "display_unit"),
        )

        names = set()

        def collect_field_names(value):
            if is_dataclass(value):
                for field in fields(value):
                    names.add(field.name)
                    collect_field_names(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    collect_field_names(item)

        collect_field_names(result)
        forbidden = {
            "name",
            "scalar_name",
            "path",
            "iri",
            "deployment_id",
            "source_path",
            "source_map",
            "graph_node",
            "connection",
            "cxf_id",
            "content",
            "provenance",
            "timestamp",
            "hash",
            "runtime_state",
            "engine_profile",
            "point_binding",
            "semantic_binding",
        }
        self.assertTrue(names.isdisjoint(forbidden))

    def test_non_resolved_input_is_a_clean_typed_error(self):
        results = []
        for value in (None, {}, object()):
            with self.subTest(value=type(value).__name__):
                with self.assertRaises(routine_scalar_abi.ScalarAbiError) as caught:
                    routine_scalar_abi.project_scalar_abi(value)
                error = caught.exception
                self.assertEqual(len(error.diagnostics), 1)
                self.assertEqual(error.diagnostics[0].code, "invalid_input")
                self.assertEqual(error.diagnostics[0].owner_kind, "projection")
                self.assertEqual(error.diagnostics[0].owner_id, "$")
                self.assertEqual(error.diagnostics[0].type_id, "")
                self.assertIn("input must be a ResolvedSpecialization", str(error))
                self.assertNotIn("Traceback", str(error))
                results.append(error.diagnostics)
        self.assertEqual(results[0], results[1])
        self.assertEqual(results[1], results[2])


if __name__ == "__main__":
    unittest.main()
