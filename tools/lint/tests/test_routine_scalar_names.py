import copy
import json
import os
import unittest
from dataclasses import FrozenInstanceError, fields, is_dataclass, replace
from pathlib import Path
from typing import Any, cast
from unittest import mock

from tools.lint import routine_resolution, routine_scalar_abi, routine_scalar_names


FIXTURE_ROOT = Path(__file__).parent / "fixtures" / "routine_schemas"
HEATING_COIL_CLASS_PATH = "Buildings.Controls.OBC.ASHRAE.G36.Types.HeatingCoil"


class RoutineScalarNameTests(unittest.TestCase):
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
    def heating_coil_mapping():
        return routine_scalar_abi.EnumAbiMapping(
            type_id="operating_mode",
            canonical_class_path=HEATING_COIL_CLASS_PATH,
            source_members=("None", "WaterBased", "Electric"),
            member_mappings=(
                routine_scalar_abi.EnumAbiMemberMapping("occupied", "None"),
                routine_scalar_abi.EnumAbiMemberMapping("warm-up", "WaterBased"),
                routine_scalar_abi.EnumAbiMemberMapping("unoccupied", "Electric"),
            ),
        )

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

    def scalar_abi(self, *, matrix_connector=False):
        interface = copy.deepcopy(self.interface)
        if matrix_connector:
            self.add_matrix_connector(interface)
        resolved = routine_resolution.resolve_specialization(
            interface, copy.deepcopy(self.specialization)
        )
        return routine_scalar_abi.project_scalar_abi(
            resolved, enum_mappings=(self.heating_coil_mapping(),)
        )

    @staticmethod
    def coordinate(dimension_id="zones", member_id="north-zone", ordinal=0):
        return routine_scalar_abi.ScalarCoordinate(
            dimension_id, member_id, ordinal
        )

    @classmethod
    def parameter(
        cls,
        parameter_id="gain",
        coordinates=(),
        *,
        type_info: Any = None,
        source="default",
        value: Any = 1.0,
    ):
        return routine_scalar_abi.ScalarParameterAbiRow(
            parameter_id,
            coordinates,
            type_info or routine_scalar_abi.ScalarAbiType("real"),
            source,
            value,
        )

    @classmethod
    def connector(
        cls,
        connector_id="signal",
        coordinates=(),
        *,
        type_info=None,
        direction="input",
    ):
        return routine_scalar_abi.ScalarConnectorAbiRow(
            connector_id,
            coordinates,
            type_info or routine_scalar_abi.ScalarAbiType("real"),
            direction,
        )

    @classmethod
    def direct_projection(cls, parameters=None, connectors=None):
        return routine_scalar_abi.ScalarAbiProjection(
            "G36-05-16-DIRECT-NAME-TEST",
            1,
            (cls.parameter(),) if parameters is None else parameters,
            (cls.connector(),) if connectors is None else connectors,
        )

    def test_synthetic_scalar_vector_and_matrix_names_keep_abi_order(self):
        projection = self.scalar_abi(matrix_connector=True)
        result = routine_scalar_names.allocate_scalar_names(projection)

        self.assertEqual(result.canonical_id, projection.canonical_id)
        self.assertEqual(result.revision, projection.revision)
        self.assertEqual(
            [(row.parameter_id, row.coordinates) for row in result.parameters],
            [(row.parameter_id, row.coordinates) for row in projection.parameters],
        )
        self.assertEqual(
            [(row.connector_id, row.coordinates) for row in result.connectors],
            [(row.connector_id, row.coordinates) for row in projection.connectors],
        )

        sample_period = self.by_id(result.parameters, "parameter_id", "sample_period_s")
        self.assertEqual(
            sample_period.scalar_name,
            "p_73616d706c655f706572696f645f73",
        )
        fixed_gains = [
            row for row in result.parameters if row.parameter_id == "fixed_gains"
        ]
        self.assertEqual(
            [row.scalar_name for row in fixed_gains],
            [
                "p_66697865645f6761696e73_66697865645f70616972_7072696d617279",
                "p_66697865645f6761696e73_66697865645f70616972_7365636f6e64617279",
            ],
        )
        matrix_weights = [
            row for row in result.parameters if row.parameter_id == "matrix_weights"
        ]
        self.assertEqual(
            [row.scalar_name for row in matrix_weights],
            [
                "p_6d61747269785f77656967687473_7a6f6e6573_6e6f7274682d7a6f6e65_66697865645f70616972_7072696d617279",
                "p_6d61747269785f77656967687473_7a6f6e6573_6e6f7274682d7a6f6e65_66697865645f70616972_7365636f6e64617279",
                "p_6d61747269785f77656967687473_7a6f6e6573_736f7574682d7a6f6e65_66697865645f70616972_7072696d617279",
                "p_6d61747269785f77656967687473_7a6f6e6573_736f7574682d7a6f6e65_66697865645f70616972_7365636f6e64617279",
                "p_6d61747269785f77656967687473_7a6f6e6573_636f72652d7a6f6e65_66697865645f70616972_7072696d617279",
                "p_6d61747269785f77656967687473_7a6f6e6573_636f72652d7a6f6e65_66697865645f70616972_7365636f6e64617279",
            ],
        )

        supply_air_flow = self.by_id(
            result.connectors, "connector_id", "supply_air_flow"
        )
        self.assertEqual(
            supply_air_flow.scalar_name,
            "c_737570706c795f6169725f666c6f77",
        )
        zone_temperatures = [
            row
            for row in result.connectors
            if row.connector_id == "zone_temperatures"
        ]
        self.assertEqual(
            zone_temperatures[0].scalar_name,
            "c_7a6f6e655f74656d706572617475726573_7a6f6e6573_6e6f7274682d7a6f6e65",
        )
        matrix_feedback = [
            row for row in result.connectors if row.connector_id == "matrix_feedback"
        ]
        self.assertEqual(
            matrix_feedback[0].scalar_name,
            "c_6d61747269785f666565646261636b_7a6f6e6573_6e6f7274682d7a6f6e65_66697865645f70616972_7072696d617279",
        )
        self.assertEqual(len(matrix_feedback), 6)

        for named, original in zip(
            result.parameters, projection.parameters, strict=True
        ):
            self.assertEqual(
                (named.type, named.source, named.value),
                (original.type, original.source, original.value),
            )
        for named, original in zip(
            result.connectors, projection.connectors, strict=True
        ):
            self.assertEqual(
                (named.type, named.direction),
                (original.type, original.direction),
            )

    def test_mapped_heating_coil_enum_payload_survives_naming(self):
        projection = self.scalar_abi()
        result = routine_scalar_names.allocate_scalar_names(projection)
        original = self.by_id(projection.parameters, "parameter_id", "initial_mode")
        named = self.by_id(result.parameters, "parameter_id", "initial_mode")

        self.assertEqual(
            named.scalar_name,
            "p_696e697469616c5f6d6f6465",
        )
        self.assertEqual(
            named.type,
            routine_scalar_abi.ScalarEnumAbiType(HEATING_COIL_CLASS_PATH),
        )
        self.assertEqual(named.value, routine_scalar_abi.ScalarEnumAbiValue(2))
        self.assertEqual(named.source, "assignment")
        self.assertIsNot(named.type, original.type)
        self.assertIsNot(named.value, original.value)

    def test_parameter_and_connector_namespaces_are_disjoint(self):
        coordinates = (self.coordinate("zones", "north-zone", 7),)
        projection = self.direct_projection(
            parameters=(self.parameter("shared", coordinates),),
            connectors=(self.connector("shared", coordinates),),
        )
        result = routine_scalar_names.allocate_scalar_names(projection)

        parameter_name = result.parameters[0].scalar_name
        connector_name = result.connectors[0].scalar_name
        self.assertEqual(
            parameter_name,
            "p_736861726564_7a6f6e6573_6e6f7274682d7a6f6e65",
        )
        self.assertEqual(
            connector_name,
            "c_736861726564_7a6f6e6573_6e6f7274682d7a6f6e65",
        )
        self.assertNotEqual(parameter_name, connector_name)

    def test_names_follow_stable_ids_not_ordinals_or_row_position(self):
        north_a = self.parameter(
            "gain", (self.coordinate("zones", "north-zone", 0),)
        )
        south_a = self.parameter(
            "gain", (self.coordinate("zones", "south-zone", 1),)
        )
        north_b = self.parameter(
            "gain", (self.coordinate("zones", "north-zone", 31),)
        )
        south_b = self.parameter(
            "gain", (self.coordinate("zones", "south-zone", 17),)
        )
        first = routine_scalar_names.allocate_scalar_names(
            self.direct_projection(parameters=(north_a, south_a), connectors=())
        )
        reordered = routine_scalar_names.allocate_scalar_names(
            self.direct_projection(parameters=(south_b, north_b), connectors=())
        )

        def by_member(result):
            return {
                row.coordinates[0].member_id: row.scalar_name
                for row in result.parameters
            }

        self.assertEqual(by_member(first), by_member(reordered))
        self.assertEqual(
            [row.coordinates[0].member_id for row in reordered.parameters],
            ["south-zone", "north-zone"],
        )
        self.assertNotEqual(
            first.parameters[0].scalar_name,
            first.parameters[1].scalar_name,
        )
        self.assertEqual(first.parameters[0].coordinates[0].ordinal, 0)
        self.assertEqual(reordered.parameters[1].coordinates[0].ordinal, 31)

    def test_delimiter_like_and_unicode_ids_encode_injectively(self):
        first = self.parameter(
            "a_b",
            (self.coordinate("d_e", "μ_雪", 0),),
        )
        second = self.parameter(
            "a",
            (self.coordinate("b_d", "e_μ_雪", 1),),
        )
        result = routine_scalar_names.allocate_scalar_names(
            self.direct_projection(parameters=(first, second), connectors=())
        )

        self.assertEqual(
            [row.scalar_name for row in result.parameters],
            [
                "p_615f62_645f65_cebc5fe99baa",
                "p_61_625f64_655fcebc5fe99baa",
            ],
        )
        self.assertEqual(len({row.scalar_name for row in result.parameters}), 2)
        for row in result.parameters:
            components = row.scalar_name[2:].split("_")
            self.assertTrue(all(components))
            self.assertTrue(
                all(character in "0123456789abcdef" for item in components for character in item)
            )

    def test_inactive_connector_absence_is_preserved(self):
        projection = self.scalar_abi()
        self.assertNotIn(
            "trim_request", [row.connector_id for row in projection.connectors]
        )
        result = routine_scalar_names.allocate_scalar_names(projection)
        self.assertNotIn(
            "trim_request", [row.connector_id for row in result.connectors]
        )
        self.assertEqual(len(result.connectors), len(projection.connectors))

    def test_allocation_is_repeatable_and_ignores_incidental_mapping_order(self):
        first_input = self.scalar_abi()
        second_input = copy.deepcopy(first_input)
        object.__setattr__(first_input, "incidental", {"z": 1, "a": 2})
        object.__setattr__(second_input, "incidental", {"a": 2, "z": 1})

        first = routine_scalar_names.allocate_scalar_names(first_input)
        second = routine_scalar_names.allocate_scalar_names(first_input)
        reordered_mapping = routine_scalar_names.allocate_scalar_names(second_input)
        self.assertEqual(first, second)
        self.assertEqual(first, reordered_mapping)

    def test_output_is_frozen_detached_recursive_and_allocation_has_no_side_effects(self):
        projection = self.scalar_abi()
        with mock.patch(
            "builtins.open", side_effect=AssertionError("file access")
        ), mock.patch(
            "builtins.hash", side_effect=AssertionError("hash access")
        ), mock.patch(
            "pathlib.Path.open", side_effect=AssertionError("path access")
        ), mock.patch(
            "socket.socket", side_effect=AssertionError("network access")
        ), mock.patch(
            "urllib.request.urlopen", side_effect=AssertionError("URL access")
        ), mock.patch(
            "os.getenv", side_effect=AssertionError("environment access")
        ), mock.patch.object(
            os.environ, "get", side_effect=AssertionError("environment access")
        ), mock.patch(
            "time.time", side_effect=AssertionError("clock access")
        ), mock.patch(
            "time.monotonic", side_effect=AssertionError("clock access")
        ), mock.patch(
            "time.perf_counter", side_effect=AssertionError("clock access")
        ), mock.patch(
            "random.random", side_effect=AssertionError("random access")
        ), mock.patch(
            "secrets.token_bytes", side_effect=AssertionError("random access")
        ), mock.patch(
            "os.urandom", side_effect=AssertionError("random access")
        ), mock.patch(
            "os.getpid", side_effect=AssertionError("process access")
        ):
            result = routine_scalar_names.allocate_scalar_names(projection)

        with self.assertRaises(FrozenInstanceError):
            setattr(result, "revision", 2)
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0], "scalar_name", "changed")
        zone_offset = self.by_id(result.parameters, "parameter_id", "zone_offsets")
        with self.assertRaises(FrozenInstanceError):
            setattr(zone_offset.coordinates[0], "member_id", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(zone_offset.type, "quantity", "changed")

        original_zone_offset = self.by_id(
            projection.parameters, "parameter_id", "zone_offsets"
        )
        original_initial_mode = self.by_id(
            projection.parameters, "parameter_id", "initial_mode"
        )
        object.__setattr__(projection, "canonical_id", "changed")
        object.__setattr__(original_zone_offset, "parameter_id", "changed")
        object.__setattr__(
            original_zone_offset.coordinates[0], "member_id", "changed"
        )
        object.__setattr__(original_zone_offset.type, "quantity", "changed")
        object.__setattr__(original_initial_mode.value, "ordinal", 99)

        self.assertEqual(result.canonical_id, "G36-05-16-SYNTHETIC-SCHEMA-TEST")
        self.assertEqual(zone_offset.parameter_id, "zone_offsets")
        self.assertEqual(zone_offset.coordinates[0].member_id, "north-zone")
        self.assertEqual(zone_offset.type.quantity, "thermodynamic_temperature")
        named_initial_mode = self.by_id(
            result.parameters, "parameter_id", "initial_mode"
        )
        self.assertEqual(
            named_initial_mode.value, routine_scalar_abi.ScalarEnumAbiValue(2)
        )

        def assert_no_mutable_values(value):
            self.assertNotIsInstance(value, (dict, list, set))
            if is_dataclass(value):
                for field in fields(value):
                    assert_no_mutable_values(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    assert_no_mutable_values(item)

        assert_no_mutable_values(result)

    def test_scalar_subclasses_are_detached_to_builtin_values(self):
        class MutableInt(int):
            pass

        class MutableString(str):
            pass

        revision = MutableInt(1)
        ordinal = MutableInt(9)
        enum_ordinal = MutableInt(2)
        canonical_id = MutableString("G36-05-16-DIRECT-NAME-TEST")
        owner_id = MutableString("gain")
        dimension_id = MutableString("zones")
        member_id = MutableString("north-zone")
        for value in (
            revision,
            ordinal,
            enum_ordinal,
            canonical_id,
            owner_id,
            dimension_id,
            member_id,
        ):
            object.__setattr__(value, "mutable", [])

        parameter = self.parameter(
            owner_id,
            (
                routine_scalar_abi.ScalarCoordinate(
                    dimension_id, member_id, ordinal
                ),
            ),
            type_info=routine_scalar_abi.ScalarEnumAbiType(
                HEATING_COIL_CLASS_PATH
            ),
            value=routine_scalar_abi.ScalarEnumAbiValue(enum_ordinal),
        )
        projection = routine_scalar_abi.ScalarAbiProjection(
            canonical_id, revision, (parameter,), ()
        )
        result = routine_scalar_names.allocate_scalar_names(projection)

        self.assertIs(type(result.canonical_id), str)
        self.assertIs(type(result.revision), int)
        self.assertIs(type(result.parameters[0].parameter_id), str)
        self.assertIs(type(result.parameters[0].coordinates[0].dimension_id), str)
        self.assertIs(type(result.parameters[0].coordinates[0].member_id), str)
        self.assertIs(type(result.parameters[0].coordinates[0].ordinal), int)
        output_value = cast(
            routine_scalar_abi.ScalarEnumAbiValue,
            result.parameters[0].value,
        )
        self.assertIs(type(output_value.ordinal), int)

    def test_malformed_and_duplicate_inputs_fail_atomically_and_repeatably(self):
        valid = self.direct_projection()
        parameter = valid.parameters[0]
        connector = valid.connectors[0]
        coordinate = self.coordinate()
        cases = {
            "empty canonical id": (
                replace(valid, canonical_id=""),
                "invalid_metadata",
            ),
            "invalid revision": (
                replace(valid, revision=True),
                "invalid_metadata",
            ),
            "parameter container": (
                replace(valid, parameters=list(valid.parameters)),
                "invalid_container",
            ),
            "connector container": (
                replace(valid, connectors=list(valid.connectors)),
                "invalid_container",
            ),
            "parameter row": (
                replace(valid, parameters=(object(),)),
                "invalid_row",
            ),
            "connector row": (
                replace(valid, connectors=(object(),)),
                "invalid_row",
            ),
            "empty owner": (
                replace(valid, parameters=(replace(parameter, parameter_id=""),)),
                "invalid_owner_id",
            ),
            "non-string owner": (
                replace(valid, parameters=(replace(parameter, parameter_id=7),)),
                "invalid_owner_id",
            ),
            "coordinate container": (
                replace(valid, parameters=(replace(parameter, coordinates=[]),)),
                "invalid_coordinates",
            ),
            "coordinate entry": (
                replace(valid, parameters=(replace(parameter, coordinates=(object(),)),)),
                "invalid_coordinate",
            ),
            "empty dimension": (
                replace(
                    valid,
                    parameters=(
                        replace(
                            parameter,
                            coordinates=(replace(coordinate, dimension_id=""),),
                        ),
                    ),
                ),
                "invalid_dimension_id",
            ),
            "non-string member": (
                replace(
                    valid,
                    parameters=(
                        replace(
                            parameter,
                            coordinates=(replace(coordinate, member_id=[]),),
                        ),
                    ),
                ),
                "invalid_member_id",
            ),
            "Boolean ordinal": (
                replace(
                    valid,
                    parameters=(
                        replace(
                            parameter,
                            coordinates=(replace(coordinate, ordinal=True),),
                        ),
                    ),
                ),
                "invalid_ordinal",
            ),
            "negative ordinal": (
                replace(
                    valid,
                    parameters=(
                        replace(
                            parameter,
                            coordinates=(replace(coordinate, ordinal=-1),),
                        ),
                    ),
                ),
                "invalid_ordinal",
            ),
            "owner UTF-8": (
                replace(
                    valid,
                    parameters=(replace(parameter, parameter_id="bad\ud800"),),
                ),
                "utf8_encoding",
            ),
            "canonical UTF-8": (
                replace(valid, canonical_id="bad\ud800"),
                "utf8_encoding",
            ),
            "member UTF-8": (
                replace(
                    valid,
                    parameters=(
                        replace(
                            parameter,
                            coordinates=(replace(coordinate, member_id="bad\ud800"),),
                        ),
                    ),
                ),
                "utf8_encoding",
            ),
            "mutable ABI payload": (
                replace(valid, parameters=(replace(parameter, value=[]),)),
                "invalid_abi_payload",
            ),
            "duplicate parameter": (
                replace(valid, parameters=(parameter, parameter)),
                "duplicate_scalar_name",
            ),
            "duplicate connector": (
                replace(valid, connectors=(connector, connector)),
                "duplicate_scalar_name",
            ),
        }

        for name, (projection, expected_code) in cases.items():
            with self.subTest(name=name):
                attempts = []
                for _ in range(2):
                    with mock.patch.object(
                        routine_scalar_names,
                        "NamedScalarParameterRow",
                        side_effect=AssertionError("parameter row allocated"),
                    ), mock.patch.object(
                        routine_scalar_names,
                        "NamedScalarConnectorRow",
                        side_effect=AssertionError("connector row allocated"),
                    ), mock.patch.object(
                        routine_scalar_names,
                        "NamedScalarProjection",
                        side_effect=AssertionError("projection allocated"),
                    ):
                        with self.assertRaises(
                            routine_scalar_names.ScalarNameError
                        ) as caught:
                            routine_scalar_names.allocate_scalar_names(projection)
                    diagnostics = caught.exception.diagnostics
                    self.assertTrue(diagnostics)
                    self.assertEqual(diagnostics, tuple(sorted(diagnostics)))
                    self.assertIn(
                        expected_code,
                        {diagnostic.code for diagnostic in diagnostics},
                    )
                    self.assertNotIn("Traceback", str(caught.exception))
                    attempts.append(diagnostics)
                self.assertEqual(attempts[0], attempts[1])
                with self.assertRaises(FrozenInstanceError):
                    setattr(attempts[0][0], "code", "changed")

    def test_non_projection_inputs_are_clean_typed_errors(self):
        results = []
        for value in (None, {}, object()):
            with self.subTest(value=type(value).__name__):
                with self.assertRaises(routine_scalar_names.ScalarNameError) as caught:
                    routine_scalar_names.allocate_scalar_names(cast(Any, value))
                error = caught.exception
                self.assertEqual(len(error.diagnostics), 1)
                self.assertEqual(error.diagnostics[0].code, "invalid_input")
                self.assertEqual(error.diagnostics[0].owner_kind, "projection")
                self.assertEqual(error.diagnostics[0].owner_id, "$")
                self.assertIn("input must be a ScalarAbiProjection", str(error))
                self.assertNotIn("Traceback", str(error))
                results.append(error.diagnostics)
        self.assertEqual(results[0], results[1])
        self.assertEqual(results[1], results[2])

    def test_named_output_shape_excludes_deferred_contracts(self):
        result = routine_scalar_names.allocate_scalar_names(self.scalar_abi())
        self.assertEqual(
            tuple(field.name for field in fields(routine_scalar_names.NamedScalarProjection)),
            ("canonical_id", "revision", "parameters", "connectors"),
        )
        self.assertEqual(
            tuple(
                field.name
                for field in fields(routine_scalar_names.NamedScalarParameterRow)
            ),
            (
                "scalar_name",
                "parameter_id",
                "coordinates",
                "type",
                "source",
                "value",
            ),
        )
        self.assertEqual(
            tuple(
                field.name
                for field in fields(routine_scalar_names.NamedScalarConnectorRow)
            ),
            ("scalar_name", "connector_id", "coordinates", "type", "direction"),
        )
        self.assertNotIn(
            "scalar_name",
            {field.name for field in fields(routine_scalar_abi.ScalarAbiProjection)},
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
        self.assertIn("scalar_name", names)
        forbidden = {
            "iri",
            "path",
            "source_map",
            "deployment_id",
            "graph_node",
            "cxf_id",
            "provenance",
            "semantic_binding",
            "point_binding",
            "runtime",
            "runtime_state",
            "persistence",
            "persistence_id",
        }
        self.assertTrue(names.isdisjoint(forbidden))


if __name__ == "__main__":
    unittest.main()
