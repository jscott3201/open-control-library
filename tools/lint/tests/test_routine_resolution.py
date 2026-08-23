import copy
import json
import unittest
from dataclasses import FrozenInstanceError, fields, is_dataclass
from pathlib import Path
from unittest import mock

from tools.lint import routine_resolution


FIXTURE_ROOT = Path(__file__).parent / "fixtures" / "routine_schemas"


class RoutineResolutionTests(unittest.TestCase):
    def setUp(self):
        self.interface = json.loads(
            (FIXTURE_ROOT / "interface.json").read_text(encoding="utf-8")
        )
        self.specialization = json.loads(
            (FIXTURE_ROOT / "specialization.json").read_text(encoding="utf-8")
        )

    def resolve(self, interface=None, specialization=None, **kwargs):
        return routine_resolution.resolve_specialization(
            self.interface if interface is None else interface,
            self.specialization if specialization is None else specialization,
            **kwargs,
        )

    @staticmethod
    def by_id(rows, field, value):
        return next(row for row in rows if getattr(row, field) == value)

    @staticmethod
    def assignment(specialization, parameter_id):
        return next(
            row
            for row in specialization["parameters"]
            if row["parameter"] == parameter_id
        )

    @staticmethod
    def connector(interface, connector_id):
        return next(
            row for row in interface["connectors"] if row["id"] == connector_id
        )

    def assert_resolution_error(self, interface, specialization, code, expected):
        results = []
        for _ in range(2):
            with self.assertRaises(routine_resolution.ResolutionError) as caught:
                routine_resolution.resolve_specialization(interface, specialization)
            error = caught.exception
            self.assertTrue(error.diagnostics)
            self.assertEqual(error.diagnostics, tuple(sorted(error.diagnostics)))
            self.assertTrue(all(item.code == code for item in error.diagnostics))
            self.assertIn(expected, str(error))
            self.assertNotIn("Traceback", str(error))
            results.append(error.diagnostics)
        self.assertEqual(results[0], results[1])

    def test_baseline_dimensions_parameters_and_enum_are_exact(self):
        result = self.resolve()
        self.assertEqual(
            result.canonical_id, "G36-05-16-SYNTHETIC-SCHEMA-TEST"
        )
        self.assertEqual(result.revision, 1)
        self.assertEqual(
            [
                (row.dimension_id, row.kind, row.extent, row.members)
                for row in result.dimensions
            ],
            [
                ("fixed_pair", "fixed", 2, ("primary", "secondary")),
                (
                    "zones",
                    "parameter",
                    3,
                    ("north-zone", "south-zone", "core-zone"),
                ),
            ],
        )

        self.assertEqual(
            [(row.parameter_id, row.source) for row in result.parameters],
            [
                ("sample_period_s", "default"),
                ("fixed_gains", "default"),
                ("zone_count", "assignment"),
                ("enable_trim", "assignment"),
                ("initial_mode", "assignment"),
                ("optional_gain", "default"),
                ("zone_offsets", "assignment"),
                ("matrix_weights", "assignment"),
            ],
        )
        sample_period = self.by_id(
            result.parameters, "parameter_id", "sample_period_s"
        )
        self.assertEqual(
            sample_period.type,
            routine_resolution.ResolvedType(kind="primitive", primitive="real"),
        )
        self.assertEqual(sample_period.leaves[0].coordinates, ())
        self.assertEqual(sample_period.leaves[0].value, 60.0)
        self.assertEqual(sample_period.dimension_ids, ())
        fixed_gains = self.by_id(result.parameters, "parameter_id", "fixed_gains")
        self.assertEqual(fixed_gains.dimension_ids, ("fixed_pair",))
        matrix_weights = self.by_id(
            result.parameters, "parameter_id", "matrix_weights"
        )
        self.assertEqual(matrix_weights.dimension_ids, ("zones", "fixed_pair"))
        zone_offsets = self.by_id(
            result.parameters, "parameter_id", "zone_offsets"
        )
        self.assertEqual(
            zone_offsets.type,
            routine_resolution.ResolvedType(
                kind="alias",
                primitive="real",
                type_id="temperature",
                quantity="thermodynamic_temperature",
                unit="K",
                display_unit="degC",
            ),
        )
        initial_mode = self.by_id(result.parameters, "parameter_id", "initial_mode")
        self.assertEqual(
            initial_mode.type,
            routine_resolution.ResolvedType(
                kind="enum",
                type_id="operating_mode",
                enum_members=(
                    routine_resolution.ResolvedEnumMember("occupied", "OCCUPIED"),
                    routine_resolution.ResolvedEnumMember("warm-up", "WARM_UP"),
                    routine_resolution.ResolvedEnumMember(
                        "unoccupied", "UNOCCUPIED"
                    ),
                ),
            ),
        )
        self.assertEqual(
            initial_mode.leaves[0].value,
            routine_resolution.EnumValue("operating_mode", "warm-up", "WARM_UP"),
        )

    def test_parameter_leaves_preserve_coordinates_and_row_major_values(self):
        result = self.resolve()
        fixed_gains = self.by_id(result.parameters, "parameter_id", "fixed_gains")
        self.assertEqual(
            [
                (leaf.coordinates[0].dimension_id, leaf.coordinates[0].member_id,
                 leaf.coordinates[0].ordinal, leaf.value)
                for leaf in fixed_gains.leaves
            ],
            [
                ("fixed_pair", "primary", 0, 1.0),
                ("fixed_pair", "secondary", 1, 0.5),
            ],
        )

        matrix = self.by_id(result.parameters, "parameter_id", "matrix_weights")
        self.assertEqual(
            [
                (tuple(coordinate.member_id for coordinate in leaf.coordinates), leaf.value)
                for leaf in matrix.leaves
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

    def test_connectors_retain_exclusions_and_use_stable_coordinates(self):
        result = self.resolve()
        self.assertEqual(
            [row.connector_id for row in result.connectors],
            [
                "zone_temperatures",
                "supply_air_flow",
                "trim_request",
                "zone_commands",
            ],
        )
        trim = self.by_id(result.connectors, "connector_id", "trim_request")
        self.assertFalse(trim.active)
        self.assertFalse(trim.guard_result)
        self.assertEqual(trim.leaves, ())

        zone_temperatures = self.by_id(
            result.connectors, "connector_id", "zone_temperatures"
        )
        self.assertTrue(zone_temperatures.active)
        self.assertIsNone(zone_temperatures.guard_result)
        self.assertEqual(zone_temperatures.dimension_ids, ("zones",))
        self.assertEqual(
            zone_temperatures.type,
            routine_resolution.ResolvedType(
                kind="alias",
                primitive="real",
                type_id="temperature",
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
            routine_resolution.ResolvedType(
                kind="alias",
                primitive="real",
                type_id="air_flow",
                quantity="volume_flow_rate",
                unit="m3/s",
            ),
        )
        self.assertEqual(
            [
                (
                    leaf.coordinates[0].dimension_id,
                    leaf.coordinates[0].member_id,
                    leaf.coordinates[0].ordinal,
                )
                for leaf in zone_temperatures.leaves
            ],
            [
                ("zones", "north-zone", 0),
                ("zones", "south-zone", 1),
                ("zones", "core-zone", 2),
            ],
        )
        self.assertFalse(
            any(
                coordinate.member_id.isdigit()
                for connector in result.connectors
                for leaf in connector.leaves
                for coordinate in leaf.coordinates
            )
        )

    def test_excluded_conditional_array_connector_retains_declared_dimensions(self):
        interface = copy.deepcopy(self.interface)
        zone_commands = self.connector(interface, "zone_commands")
        zone_commands["presence"] = copy.deepcopy(
            self.connector(interface, "trim_request")["presence"]
        )

        resolved = self.by_id(
            self.resolve(interface=interface).connectors,
            "connector_id",
            "zone_commands",
        )
        self.assertFalse(resolved.active)
        self.assertFalse(resolved.guard_result)
        self.assertEqual(resolved.dimension_ids, ("zones",))
        self.assertEqual(resolved.leaves, ())

    def test_conditional_connector_can_become_active_with_a_scalar_leaf(self):
        specialization = copy.deepcopy(self.specialization)
        self.assignment(specialization, "enable_trim")["value"] = True
        trim = self.by_id(
            self.resolve(specialization=specialization).connectors,
            "connector_id",
            "trim_request",
        )
        self.assertTrue(trim.active)
        self.assertTrue(trim.guard_result)
        self.assertEqual(trim.leaves, (routine_resolution.ScalarConnectorLeaf(()),))

    def test_all_governed_comparisons_and_enum_equality_are_evaluated(self):
        cases = (
            ("eq", 3.0, True),
            ("ne", 4.0, True),
            ("lt", 4.0, True),
            ("lte", 3.0, True),
            ("gt", 2.0, True),
            ("gte", 3.0, True),
        )
        for operator, literal, expected in cases:
            with self.subTest(operator=operator):
                interface = copy.deepcopy(self.interface)
                self.connector(interface, "trim_request")["presence"]["guard"] = {
                    "op": operator,
                    "left": {"kind": "parameter", "parameter": "zone_count"},
                    "right": {
                        "kind": "literal",
                        "type": {"kind": "primitive", "primitive": "real"},
                        "value": literal,
                    },
                }
                trim = self.by_id(
                    self.resolve(interface=interface).connectors,
                    "connector_id",
                    "trim_request",
                )
                self.assertEqual(trim.guard_result, expected)

        interface = copy.deepcopy(self.interface)
        self.connector(interface, "trim_request")["presence"]["guard"] = {
            "op": "eq",
            "left": {"kind": "parameter", "parameter": "initial_mode"},
            "right": {
                "kind": "literal",
                "type": {"kind": "named", "type": "operating_mode"},
                "value": "warm-up",
            },
        }
        trim = self.by_id(
            self.resolve(interface=interface).connectors,
            "connector_id",
            "trim_request",
        )
        self.assertTrue(trim.guard_result)

    def test_assignment_member_record_and_object_key_order_do_not_matter(self):
        interface = copy.deepcopy(self.interface)
        specialization = copy.deepcopy(self.specialization)
        interface["dimensions"].append(
            {
                "id": "loops",
                "extent": {"kind": "parameter", "parameter": "loop_count"},
            }
        )
        interface["parameters"].append(
            {
                "id": "loop_count",
                "type": {"kind": "primitive", "primitive": "integer"},
                "shape": {"kind": "scalar"},
                "configurability": "configurable",
                "default": 2,
            }
        )
        specialization["members"].append(
            {"dimension": "loops", "members": ["loop-a", "loop-b"]}
        )
        baseline = self.resolve(interface=interface, specialization=specialization)

        specialization["parameters"].reverse()
        specialization["members"].reverse()

        def reverse_keys(value):
            if isinstance(value, dict):
                return {
                    key: reverse_keys(item)
                    for key, item in reversed(tuple(value.items()))
                }
            if isinstance(value, list):
                return [reverse_keys(item) for item in value]
            return value

        self.assertEqual(
            self.resolve(
                interface=reverse_keys(interface),
                specialization=reverse_keys(specialization),
            ),
            baseline,
        )

    def test_authored_member_order_changes_value_association_without_sorting(self):
        interface = copy.deepcopy(self.interface)
        interface["dimensions"][0]["members"] = ["secondary", "primary"]
        result = self.resolve(interface=interface)
        fixed_gains = self.by_id(result.parameters, "parameter_id", "fixed_gains")
        self.assertEqual(
            [(leaf.coordinates[0].member_id, leaf.value) for leaf in fixed_gains.leaves],
            [("secondary", 1.0), ("primary", 0.5)],
        )

        specialization = copy.deepcopy(self.specialization)
        specialization["members"][0]["members"] = [
            "south-zone",
            "north-zone",
            "core-zone",
        ]
        zone_offsets = self.by_id(
            self.resolve(specialization=specialization).parameters,
            "parameter_id",
            "zone_offsets",
        )
        self.assertEqual(
            [(leaf.coordinates[0].member_id, leaf.value) for leaf in zone_offsets.leaves],
            [
                ("south-zone", 0.0),
                ("north-zone", 0.5),
                ("core-zone", -0.5),
            ],
        )

    def test_resolution_is_repeatable_immutable_and_does_not_mutate_inputs(self):
        interface_before = copy.deepcopy(self.interface)
        specialization_before = copy.deepcopy(self.specialization)
        first = self.resolve()
        second = self.resolve()
        self.assertEqual(first, second)
        self.assertEqual(self.interface, interface_before)
        self.assertEqual(self.specialization, specialization_before)
        with self.assertRaises(FrozenInstanceError):
            setattr(first, "revision", 2)
        initial_mode = self.by_id(first.parameters, "parameter_id", "initial_mode")
        self.assertIsInstance(initial_mode.type.enum_members, tuple)
        with self.assertRaises(FrozenInstanceError):
            setattr(initial_mode.type.enum_members[0], "symbol", "CHANGED")
        self.interface["types"][2]["members"][0]["symbol"] = "CHANGED"
        self.assertEqual(initial_mode.type.enum_members[0].symbol, "OCCUPIED")
        fixed_gains = self.by_id(first.parameters, "parameter_id", "fixed_gains")
        zone_temperatures = self.by_id(
            first.connectors, "connector_id", "zone_temperatures"
        )
        self.assertIsInstance(fixed_gains.dimension_ids, tuple)
        self.assertIsInstance(zone_temperatures.dimension_ids, tuple)
        self.interface["parameters"][1]["shape"]["dimensions"][0] = "changed"
        self.interface["connectors"][0]["shape"]["dimensions"][0] = "changed"
        self.assertEqual(fixed_gains.dimension_ids, ("fixed_pair",))
        self.assertEqual(zone_temperatures.dimension_ids, ("zones",))

        def assert_no_mutable_values(value):
            self.assertNotIsInstance(value, (dict, list, set))
            if is_dataclass(value):
                for field in fields(value):
                    assert_no_mutable_values(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    assert_no_mutable_values(item)

        assert_no_mutable_values(first)

    def test_resolution_never_opens_network_resources(self):
        with mock.patch(
            "socket.socket", side_effect=AssertionError("network access")
        ), mock.patch(
            "urllib.request.urlopen", side_effect=AssertionError("URL access")
        ):
            self.resolve()

    def test_schema_invalid_values_have_sorted_typed_diagnostics(self):
        interface = copy.deepcopy(self.interface)
        interface["connectors"] = "not-an-array"
        self.assert_resolution_error(
            interface, self.specialization, "schema", "not of type 'array'"
        )
        self.assert_resolution_error(
            [], self.specialization, "schema", "not of type 'object'"
        )

    def test_semantic_failures_reuse_pair_validation_policy(self):
        mutations = (
            (
                "fixed override",
                lambda interface, specialization: specialization["parameters"].append(
                    {"parameter": "sample_period_s", "value": 30.0}
                ),
                "cannot be overridden",
            ),
            (
                "missing configurable",
                lambda interface, specialization: specialization.update(
                    parameters=[
                        row
                        for row in specialization["parameters"]
                        if row["parameter"] != "zone_offsets"
                    ]
                ),
                "requires a value",
            ),
            (
                "type",
                lambda interface, specialization: self.assignment(
                    specialization, "zone_count"
                ).update(value="three"),
                "value must be integer",
            ),
            (
                "bounds",
                lambda interface, specialization: self.assignment(
                    specialization, "zone_count"
                ).update(value=9),
                "exceeds maximum 8",
            ),
            (
                "ragged matrix",
                lambda interface, specialization: self.assignment(
                    specialization, "matrix_weights"
                )["value"].__setitem__(1, [0.5]),
                "dimension 1 length must be 2",
            ),
            (
                "members",
                lambda interface, specialization: specialization["members"][0][
                    "members"
                ].__setitem__(1, "north-zone"),
                "duplicate stable member",
            ),
            (
                "guard",
                lambda interface, specialization: self.connector(
                    interface, "trim_request"
                )["presence"].update(
                    guard={
                        "op": "eq",
                        "left": {
                            "kind": "parameter",
                            "parameter": "missing_parameter",
                        },
                        "right": {
                            "kind": "literal",
                            "type": {
                                "kind": "primitive",
                                "primitive": "boolean",
                            },
                            "value": True,
                        },
                    }
                ),
                "unknown guard parameter",
            ),
        )
        for label, mutation, expected in mutations:
            with self.subTest(label=label):
                interface = copy.deepcopy(self.interface)
                specialization = copy.deepcopy(self.specialization)
                mutation(interface, specialization)
                self.assert_resolution_error(
                    interface, specialization, "semantic", expected
                )

    def test_huge_integer_accepted_as_real_is_constraint_checked(self):
        interface = copy.deepcopy(self.interface)
        interface["parameters"][0]["default"] = 10**400
        self.assert_resolution_error(
            interface,
            self.specialization,
            "semantic",
            "exceeds maximum 3600.0",
        )

    def test_resolver_calls_shared_schema_and_pair_validators(self):
        schemas = routine_resolution.routine_schemas
        with mock.patch.object(
            schemas,
            "_check_schema_instance",
            wraps=schemas._check_schema_instance,
        ) as schema_check, mock.patch.object(
            schemas,
            "_check_interface_and_specialization",
            wraps=schemas._check_interface_and_specialization,
        ) as pair_check:
            self.resolve()
        self.assertEqual(schema_check.call_count, 2)
        pair_check.assert_called_once()

    def test_interface_and_specialization_identity_must_agree(self):
        specialization = copy.deepcopy(self.specialization)
        specialization["canonical_id"] = "G36-05-16-OTHER-TEST"
        specialization["revision"] = 2
        with self.assertRaises(routine_resolution.ResolutionError) as caught:
            self.resolve(specialization=specialization)
        self.assertEqual(
            [(item.code, item.path) for item in caught.exception.diagnostics],
            [("semantic", "$.canonical_id"), ("semantic", "$.revision")],
        )

    def test_guard_depth_and_node_limits_preflight_before_validation(self):
        interface = copy.deepcopy(self.interface)
        comparison = {
            "op": "eq",
            "left": {"kind": "parameter", "parameter": "enable_trim"},
            "right": {
                "kind": "literal",
                "type": {"kind": "primitive", "primitive": "boolean"},
                "value": True,
            },
        }
        guard = comparison
        for _ in range(100):
            guard = {"op": "not", "operand": guard}
        self.connector(interface, "trim_request")["presence"]["guard"] = guard
        limits = routine_resolution.ResolutionLimits(max_guard_depth=4)
        with mock.patch.object(
            routine_resolution, "_validated_model"
        ) as validated_model:
            with self.assertRaises(routine_resolution.ResolutionError) as caught:
                self.resolve(interface=interface, limits=limits)
            validated_model.assert_not_called()
        self.assertEqual(caught.exception.diagnostics[0].code, "resource_limit")
        self.assertIn("guard depth 5 exceeds limit 4", str(caught.exception))
        self.assertNotIn("RecursionError", str(caught.exception))

        interface = copy.deepcopy(self.interface)
        self.connector(interface, "trim_request")["presence"]["guard"] = {
            "op": "and",
            "operands": [copy.deepcopy(comparison) for _ in range(1000)],
        }
        limits = routine_resolution.ResolutionLimits(max_guard_nodes=5)
        with mock.patch.object(
            routine_resolution, "_validated_model"
        ) as validated_model:
            with self.assertRaises(routine_resolution.ResolutionError) as caught:
                self.resolve(interface=interface, limits=limits)
            validated_model.assert_not_called()
        self.assertEqual(caught.exception.diagnostics[0].code, "resource_limit")
        self.assertIn("guard node count exceeds limit 5", str(caught.exception))

    def test_scalar_leaf_limit_preflights_before_output_allocation(self):
        limits = routine_resolution.ResolutionLimits(max_scalar_leaves=22)
        with mock.patch.object(
            routine_resolution, "_parameter_leaves"
        ) as parameter_leaves:
            with self.assertRaises(routine_resolution.ResolutionError) as caught:
                self.resolve(limits=limits)
            parameter_leaves.assert_not_called()
        self.assertEqual(caught.exception.diagnostics[0].code, "resource_limit")
        self.assertIn("scalar leaf expansion 23 exceeds limit 22", str(caught.exception))

    def test_invalid_limit_values_are_typed_errors(self):
        with self.assertRaises(routine_resolution.ResolutionError) as caught:
            self.resolve(
                limits=routine_resolution.ResolutionLimits(
                    max_guard_depth=-1,
                    max_guard_nodes=True,
                    max_scalar_leaves=-2,
                )
            )
        self.assertEqual(len(caught.exception.diagnostics), 3)
        self.assertTrue(
            all(item.code == "invalid_limit" for item in caught.exception.diagnostics)
        )

    def test_result_model_excludes_deferred_artifact_and_runtime_concepts(self):
        result = self.resolve()
        self.assertEqual(
            tuple(field.name for field in fields(result)),
            ("canonical_id", "revision", "dimensions", "parameters", "connectors"),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_resolution.ResolvedParameter)),
            ("parameter_id", "type", "dimension_ids", "source", "leaves"),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_resolution.ResolvedConnector)),
            (
                "connector_id",
                "direction",
                "type",
                "dimension_ids",
                "active",
                "guard_result",
                "leaves",
            ),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_resolution.ResolvedType)),
            (
                "kind",
                "primitive",
                "type_id",
                "quantity",
                "unit",
                "display_unit",
                "enum_members",
            ),
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_resolution.ResolvedEnumMember)),
            ("member_id", "symbol"),
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
            "deployment_id",
            "source_path",
            "source_revision",
            "source_map",
            "semantic_binding",
            "point_binding",
            "graph_node",
            "connection",
            "cxf_id",
            "engine_id",
            "content_hash",
            "provenance",
            "timestamp",
            "random_id",
            "schema",
            "$id",
        }
        self.assertTrue(names.isdisjoint(forbidden))


if __name__ == "__main__":
    unittest.main()
