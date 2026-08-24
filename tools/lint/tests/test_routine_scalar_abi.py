import copy
import json
import math
import unittest
from dataclasses import FrozenInstanceError, fields, is_dataclass, replace
from pathlib import Path
from unittest import mock

from tools.lint import routine_resolution, routine_scalar_abi


FIXTURE_ROOT = Path(__file__).parent / "fixtures" / "routine_schemas"
HEATING_COIL_CLASS_PATH = (
    "Buildings.Controls.OBC.ASHRAE.G36.Types.HeatingCoil"
)
HEATING_COIL_SOURCE_MEMBERS = ("None", "WaterBased", "Electric")


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
    def add_enum_connector(
        interface,
        presence,
        *,
        connector_id="mode_signal",
        type_id="operating_mode",
        direction="input",
        scalar=False,
    ):
        interface["connectors"].append(
            {
                "id": connector_id,
                "direction": direction,
                "type": {"kind": "named", "type": type_id},
                "shape": (
                    {"kind": "scalar"}
                    if scalar
                    else {"kind": "array", "dimensions": ["fixed_pair"]}
                ),
                "presence": presence,
            }
        )

    def heating_coil_documents(
        self,
        member_order=("electric", "none", "water-based"),
        symbols=None,
    ):
        interface = copy.deepcopy(self.interface)
        specialization = copy.deepcopy(self.specialization)
        symbols = symbols or {
            "none": "LOCAL_ZERO",
            "water-based": "LOCAL_WATER",
            "electric": "LOCAL_ELECTRIC",
        }
        enum_type = self.document_by_id(interface["types"], "operating_mode")
        enum_type["id"] = "heating_coil"
        enum_type["members"] = [
            {"id": member_id, "symbol": symbols[member_id]}
            for member_id in member_order
        ]

        replacements = {
            "occupied": "none",
            "warm-up": "water-based",
            "unoccupied": "electric",
        }

        def replace_enum_references(value):
            if isinstance(value, dict):
                for key, item in value.items():
                    if key == "type" and item == "operating_mode":
                        value[key] = "heating_coil"
                    elif (
                        key in {"default", "value"}
                        and isinstance(item, str)
                        and item in replacements
                    ):
                        value[key] = replacements[item]
                    else:
                        replace_enum_references(item)
            elif isinstance(value, list):
                for item in value:
                    replace_enum_references(item)

        replace_enum_references(interface)
        replace_enum_references(specialization)
        return interface, specialization

    @staticmethod
    def heating_coil_mapping():
        return routine_scalar_abi.EnumAbiMapping(
            type_id="heating_coil",
            canonical_class_path=HEATING_COIL_CLASS_PATH,
            source_members=HEATING_COIL_SOURCE_MEMBERS,
            member_mappings=(
                routine_scalar_abi.EnumAbiMemberMapping(
                    "water-based", "WaterBased"
                ),
                routine_scalar_abi.EnumAbiMemberMapping("electric", "Electric"),
                routine_scalar_abi.EnumAbiMemberMapping("none", "None"),
            ),
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

    def test_heating_coil_parameter_and_connectors_project_class_and_ordinal(self):
        interface, specialization = self.heating_coil_documents()
        interface["parameters"].append(
            {
                "id": "coil_sequence",
                "type": {"kind": "named", "type": "heating_coil"},
                "shape": {"kind": "array", "dimensions": ["fixed_pair"]},
                "configurability": "fixed",
                "default": ["electric", "none"],
            }
        )
        self.add_enum_connector(
            interface,
            {"kind": "always"},
            connector_id="coil_command",
            type_id="heating_coil",
            direction="output",
            scalar=True,
        )
        self.add_enum_connector(
            interface,
            {"kind": "always"},
            type_id="heating_coil",
        )
        mapping = self.heating_coil_mapping()
        result = routine_scalar_abi.project_scalar_abi(
            self.resolve(interface, specialization), enum_mappings=(mapping,)
        )

        expected_type = routine_scalar_abi.ScalarEnumAbiType(
            HEATING_COIL_CLASS_PATH
        )
        initial_mode = self.by_id(result.parameters, "parameter_id", "initial_mode")
        self.assertEqual(initial_mode.type, expected_type)
        self.assertEqual(
            initial_mode.value, routine_scalar_abi.ScalarEnumAbiValue(2)
        )
        self.assertEqual(initial_mode.coordinates, ())
        self.assertEqual(initial_mode.source, "assignment")
        self.assertNotEqual(
            initial_mode.type, routine_scalar_abi.ScalarAbiType("integer")
        )
        self.assertNotEqual(initial_mode.value, 2)

        sequence = [
            row for row in result.parameters if row.parameter_id == "coil_sequence"
        ]
        self.assertEqual(
            [
                (row.coordinates[0].member_id, row.coordinates[0].ordinal, row.value)
                for row in sequence
            ],
            [
                ("primary", 0, routine_scalar_abi.ScalarEnumAbiValue(3)),
                ("secondary", 1, routine_scalar_abi.ScalarEnumAbiValue(1)),
            ],
        )
        self.assertEqual([row.source for row in sequence], ["default", "default"])
        self.assertTrue(all(row.type == expected_type for row in sequence))

        coil_command = self.by_id(
            result.connectors, "connector_id", "coil_command"
        )
        self.assertEqual(coil_command.type, expected_type)
        self.assertEqual(coil_command.direction, "output")
        self.assertEqual(coil_command.coordinates, ())
        mode_signal = [
            row for row in result.connectors if row.connector_id == "mode_signal"
        ]
        self.assertEqual(
            [
                (row.coordinates[0].member_id, row.coordinates[0].ordinal)
                for row in mode_signal
            ],
            [("primary", 0), ("secondary", 1)],
        )
        self.assertTrue(all(row.type == expected_type for row in mode_signal))
        self.assertTrue(all(row.direction == "input" for row in mode_signal))

    def test_local_enum_order_and_symbols_do_not_set_source_ordinal(self):
        variants = (
            (
                ("electric", "none", "water-based"),
                {
                    "none": "SYMBOL_9",
                    "water-based": "SYMBOL_1",
                    "electric": "SYMBOL_5",
                },
            ),
            (
                ("water-based", "electric", "none"),
                {
                    "none": "ARBITRARY_C",
                    "water-based": "ARBITRARY_A",
                    "electric": "ARBITRARY_B",
                },
            ),
        )
        results = []
        for member_order, symbols in variants:
            interface, specialization = self.heating_coil_documents(
                member_order, symbols
            )
            results.append(
                routine_scalar_abi.project_scalar_abi(
                    self.resolve(interface, specialization),
                    enum_mappings=(self.heating_coil_mapping(),),
                )
            )

        self.assertEqual(results[0], results[1])
        initial_mode = self.by_id(
            results[0].parameters, "parameter_id", "initial_mode"
        )
        self.assertEqual(
            initial_mode.value, routine_scalar_abi.ScalarEnumAbiValue(2)
        )

    def test_source_member_list_can_cover_a_local_enum_subset(self):
        interface, specialization = self.heating_coil_documents(
            ("none", "water-based")
        )
        self.document_by_id(interface["connectors"], "trim_request")["presence"] = {
            "kind": "always"
        }
        mapping = replace(
            self.heating_coil_mapping(),
            member_mappings=tuple(
                item
                for item in self.heating_coil_mapping().member_mappings
                if item.member_id != "electric"
            ),
        )
        result = routine_scalar_abi.project_scalar_abi(
            self.resolve(interface, specialization), enum_mappings=(mapping,)
        )
        initial_mode = self.by_id(result.parameters, "parameter_id", "initial_mode")
        self.assertEqual(mapping.source_members, HEATING_COIL_SOURCE_MEMBERS)
        self.assertEqual(
            initial_mode.value, routine_scalar_abi.ScalarEnumAbiValue(2)
        )

    def test_enum_mapping_and_output_are_frozen_detached_and_have_no_io(self):
        interface, specialization = self.heating_coil_documents()
        resolved = self.resolve(interface, specialization)
        resolved_before = copy.deepcopy(resolved)
        mapping = self.heating_coil_mapping()
        mapping_before = copy.deepcopy(mapping)
        with mock.patch("builtins.open", side_effect=AssertionError("file access")), mock.patch(
            "pathlib.Path.open", side_effect=AssertionError("path access")
        ), mock.patch(
            "socket.socket", side_effect=AssertionError("network access")
        ), mock.patch(
            "urllib.request.urlopen", side_effect=AssertionError("URL access")
        ):
            first = routine_scalar_abi.project_scalar_abi(
                resolved, enum_mappings=(mapping,)
            )
            second = routine_scalar_abi.project_scalar_abi(
                resolved, enum_mappings=(mapping,)
            )
        self.assertEqual(first, second)
        self.assertEqual(resolved, resolved_before)
        self.assertEqual(mapping, mapping_before)
        self.assertIsInstance(mapping.source_members, tuple)
        self.assertIsInstance(mapping.member_mappings, tuple)

        initial_mode = self.by_id(first.parameters, "parameter_id", "initial_mode")
        with self.assertRaises(FrozenInstanceError):
            setattr(mapping, "canonical_class_path", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(mapping.member_mappings[0], "source_literal", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(initial_mode.type, "canonical_class_path", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(initial_mode.value, "ordinal", 99)

        object.__setattr__(mapping, "canonical_class_path", "changed")
        object.__setattr__(mapping.member_mappings[0], "source_literal", "changed")
        self.assertEqual(
            initial_mode.type,
            routine_scalar_abi.ScalarEnumAbiType(HEATING_COIL_CLASS_PATH),
        )
        self.assertEqual(
            initial_mode.value, routine_scalar_abi.ScalarEnumAbiValue(2)
        )

    def test_mapping_record_order_does_not_change_projection(self):
        interface, specialization = self.heating_coil_documents()
        interface["types"].append(
            {
                "id": "standby_mode",
                "kind": "enum",
                "members": [
                    {"id": "off", "symbol": "OFF"},
                    {"id": "ready", "symbol": "READY"},
                ],
            }
        )
        presence = copy.deepcopy(
            self.document_by_id(interface["connectors"], "trim_request")["presence"]
        )
        self.add_enum_connector(
            interface,
            presence,
            connector_id="standby_signal",
            type_id="standby_mode",
            scalar=True,
        )
        standby_mapping = routine_scalar_abi.EnumAbiMapping(
            "standby_mode",
            "Example.Controls.Types.StandbyMode",
            ("Off", "Ready"),
            (
                routine_scalar_abi.EnumAbiMemberMapping("off", "Off"),
                routine_scalar_abi.EnumAbiMemberMapping("ready", "Ready"),
            ),
        )
        heating_mapping = self.heating_coil_mapping()
        reordered_heating_mapping = replace(
            heating_mapping,
            member_mappings=tuple(reversed(heating_mapping.member_mappings)),
        )
        resolved = self.resolve(interface, specialization)

        first = routine_scalar_abi.project_scalar_abi(
            resolved, enum_mappings=(heating_mapping, standby_mapping)
        )
        second = routine_scalar_abi.project_scalar_abi(
            resolved, enum_mappings=(standby_mapping, reordered_heating_mapping)
        )
        self.assertEqual(first, second)
        self.assertNotIn(
            "standby_signal", [row.connector_id for row in first.connectors]
        )

    def test_invalid_enum_mappings_fail_atomically_with_typed_diagnostics(self):
        interface, specialization = self.heating_coil_documents()
        resolved = self.resolve(interface, specialization)
        valid = self.heating_coil_mapping()
        member = routine_scalar_abi.EnumAbiMemberMapping
        members = valid.member_mappings
        cases = {
            "duplicate type": (
                (valid, valid),
                {"duplicate_enum_mapping"},
            ),
            "unknown type": (
                (valid, replace(valid, type_id="missing_type")),
                {"unknown_enum_mapping_type"},
            ),
            "non-enum type": (
                (valid, replace(valid, type_id="temperature")),
                {"non_enum_mapping_type"},
            ),
            "missing local member": (
                (replace(valid, member_mappings=members[:-1]),),
                {"missing_enum_local_member"},
            ),
            "extra local member": (
                (
                    replace(
                        valid,
                        source_members=valid.source_members + ("Spare",),
                        member_mappings=members + (member("spare", "Spare"),),
                    ),
                ),
                {"extra_enum_local_member"},
            ),
            "duplicate local member": (
                (
                    replace(
                        valid,
                        source_members=valid.source_members + ("Spare",),
                        member_mappings=members + (member("none", "Spare"),),
                    ),
                ),
                {"duplicate_enum_local_member"},
            ),
            "duplicate destination literal": (
                (
                    replace(
                        valid,
                        member_mappings=tuple(
                            replace(item, source_literal="WaterBased")
                            if item.member_id == "electric"
                            else item
                            for item in members
                        ),
                    ),
                ),
                {"duplicate_enum_source_literal"},
            ),
            "empty source members": (
                (replace(valid, source_members=()),),
                {"invalid_enum_mapping"},
            ),
            "duplicate source member": (
                (replace(valid, source_members=valid.source_members + ("None",)),),
                {"duplicate_enum_source_member"},
            ),
            "unknown source literal": (
                (
                    replace(
                        valid,
                        member_mappings=tuple(
                            replace(item, source_literal="Missing")
                            if item.member_id == "water-based"
                            else item
                            for item in members
                        ),
                    ),
                ),
                {"unknown_enum_source_literal"},
            ),
            "empty class path": (
                (replace(valid, canonical_class_path=""),),
                {"invalid_enum_mapping"},
            ),
            "empty type id": (
                (valid, replace(valid, type_id="")),
                {"invalid_enum_mapping"},
            ),
            "empty local member id": (
                (
                    replace(
                        valid,
                        member_mappings=(
                            replace(members[0], member_id=""),
                            *members[1:],
                        ),
                    ),
                ),
                {"invalid_enum_mapping", "missing_enum_local_member"},
            ),
            "empty mapping source literal": (
                (
                    replace(
                        valid,
                        member_mappings=(
                            replace(members[0], source_literal=""),
                            *members[1:],
                        ),
                    ),
                ),
                {"invalid_enum_mapping"},
            ),
            "empty source member literal": (
                (replace(valid, source_members=("None", "", "Electric")),),
                {"invalid_enum_mapping"},
            ),
            "non-tuple source members": (
                (replace(valid, source_members=list(valid.source_members)),),
                {"invalid_enum_mapping"},
            ),
            "non-tuple member mappings": (
                (replace(valid, member_mappings=list(members)),),
                {"invalid_enum_mapping"},
            ),
            "malformed member mapping": (
                (replace(valid, member_mappings=(object(), *members[1:])),),
                {"invalid_enum_mapping", "missing_enum_local_member"},
            ),
        }

        for name, (mappings, expected_codes) in cases.items():
            with self.subTest(name=name):
                attempts = []
                for _ in range(2):
                    with mock.patch.object(
                        routine_scalar_abi,
                        "ScalarParameterAbiRow",
                        side_effect=AssertionError("parameter row allocated"),
                    ), mock.patch.object(
                        routine_scalar_abi,
                        "ScalarConnectorAbiRow",
                        side_effect=AssertionError("connector row allocated"),
                    ):
                        with self.assertRaises(
                            routine_scalar_abi.ScalarAbiError
                        ) as caught:
                            routine_scalar_abi.project_scalar_abi(
                                resolved, enum_mappings=mappings
                            )
                    diagnostics = caught.exception.diagnostics
                    self.assertEqual(diagnostics, tuple(sorted(diagnostics)))
                    self.assertTrue(
                        expected_codes.issubset(
                            {diagnostic.code for diagnostic in diagnostics}
                        )
                    )
                    attempts.append(diagnostics)
                self.assertEqual(attempts[0], attempts[1])

    def test_malformed_mapping_container_is_rejected_before_output(self):
        interface, specialization = self.heating_coil_documents()
        resolved = self.resolve(interface, specialization)
        valid = self.heating_coil_mapping()
        for value in (None, [], (object(),), (valid, object())):
            with self.subTest(value=type(value).__name__):
                with mock.patch.object(
                    routine_scalar_abi,
                    "ScalarParameterAbiRow",
                    side_effect=AssertionError("parameter row allocated"),
                ):
                    with self.assertRaises(routine_scalar_abi.ScalarAbiError) as caught:
                        routine_scalar_abi.project_scalar_abi(
                            resolved, enum_mappings=value
                        )
                self.assertIn(
                    "invalid_enum_mappings",
                    {diagnostic.code for diagnostic in caught.exception.diagnostics},
                )

    def test_mapping_reorder_does_not_change_diagnostic_order(self):
        interface, specialization = self.heating_coil_documents()
        resolved = self.resolve(interface, specialization)
        valid = self.heating_coil_mapping()
        bad_a = replace(valid, type_id="aaa_missing")
        bad_z = replace(valid, type_id="zzz_missing")
        diagnostics = []
        for mappings in (
            (bad_z, valid, bad_a),
            (bad_a, bad_z, valid),
        ):
            with self.assertRaises(routine_scalar_abi.ScalarAbiError) as caught:
                routine_scalar_abi.project_scalar_abi(
                    resolved, enum_mappings=mappings
                )
            diagnostics.append(caught.exception.diagnostics)
        self.assertEqual(diagnostics[0], diagnostics[1])

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
        self.assertEqual(
            tuple(
                field.name for field in fields(routine_scalar_abi.ScalarEnumAbiType)
            ),
            ("canonical_class_path",),
        )
        self.assertEqual(
            tuple(
                field.name for field in fields(routine_scalar_abi.ScalarEnumAbiValue)
            ),
            ("ordinal",),
        )
        self.assertEqual(
            tuple(
                field.name for field in fields(routine_scalar_abi.EnumAbiMemberMapping)
            ),
            ("member_id", "source_literal"),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_scalar_abi.EnumAbiMapping)),
            (
                "type_id",
                "canonical_class_path",
                "source_members",
                "member_mappings",
            ),
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
