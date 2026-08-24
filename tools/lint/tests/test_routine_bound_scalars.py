import copy
import json
import os
import unittest
from dataclasses import FrozenInstanceError, fields, is_dataclass, replace
from pathlib import Path
from typing import Any, cast
from unittest import mock

from tools.lint import (
    routine_bound_scalars,
    routine_resolution,
    routine_scalar_abi,
    routine_scalar_names,
    routine_scalar_source_claims,
)


REPO_ROOT = Path(__file__).parents[3]
FIXTURE_ROOT = Path(__file__).parent / "fixtures" / "routine_schemas"
INVENTORY_PATH = REPO_ROOT / "routines" / "g36" / "source-inventory.json"
RELEASE_PIN_PATH = REPO_ROOT / "routines" / "g36" / "SOURCE_RELEASE_PIN"
DEVELOPMENT_PIN_PATH = (
    REPO_ROOT / "routines" / "g36" / "SOURCE_DEVELOPMENT_PIN"
)
RELEASE_REVISION = "55abf579598ca81cae0a82f337350375958e6722"
DEVELOPMENT_REVISION = "eccb40b3974bb10eef120c5670a6454e43ca36e3"
TRIM_CLASS = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond"
TRIM_PATH = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo"
TRIM_BLOB = "sha1:028439a4fb478fc041d703a092d5186f5861eb03"
ENUM_CLASS = "Buildings.Controls.OBC.ASHRAE.G36.Types.HeatingCoil"


class RoutineBoundScalarTests(unittest.TestCase):
    def setUp(self):
        self.inventory = json.loads(INVENTORY_PATH.read_text(encoding="utf-8"))
        self.release_revision = RELEASE_PIN_PATH.read_text(encoding="utf-8").strip()
        self.development_revision = DEVELOPMENT_PIN_PATH.read_text(
            encoding="utf-8"
        ).strip()

    @staticmethod
    def enum_mapping():
        return routine_scalar_abi.EnumAbiMapping(
            type_id="operating_mode",
            canonical_class_path=ENUM_CLASS,
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

    @staticmethod
    def by_id(rows, field, value):
        return next(row for row in rows if getattr(row, field) == value)

    @staticmethod
    def scalar_name(prefix, owner_id, coordinates=()):
        components = [owner_id.encode("utf-8").hex()]
        for coordinate in coordinates:
            components.extend(
                (
                    coordinate.dimension_id.encode("utf-8").hex(),
                    coordinate.member_id.encode("utf-8").hex(),
                )
            )
        return prefix + "_".join(components)

    @staticmethod
    def coordinate(dimension_id="zones", member_id="north", ordinal=0):
        return routine_scalar_abi.ScalarCoordinate(
            dimension_id, member_id, ordinal
        )

    def pins(self):
        return (
            routine_scalar_source_claims.SourcePin(
                "release", self.release_revision
            ),
            routine_scalar_source_claims.SourcePin(
                "development", self.development_revision
            ),
        )

    @staticmethod
    def trim_locator():
        return routine_scalar_source_claims.SourceFileLocator(
            TRIM_PATH, TRIM_BLOB
        )

    def through_existing_stages(self, *, matrix_connector=False):
        interface = json.loads(
            (FIXTURE_ROOT / "interface.json").read_text(encoding="utf-8")
        )
        specialization = json.loads(
            (FIXTURE_ROOT / "specialization.json").read_text(encoding="utf-8")
        )
        if matrix_connector:
            self.add_matrix_connector(interface)
        resolved = routine_resolution.resolve_specialization(
            interface, specialization
        )
        abi = routine_scalar_abi.project_scalar_abi(
            resolved, enum_mappings=(self.enum_mapping(),)
        )
        named = routine_scalar_names.allocate_scalar_names(abi)
        owners = []
        for row in named.parameters:
            key = ("parameter", row.parameter_id)
            if key not in owners:
                owners.append(key)
        for row in named.connectors:
            key = ("connector", row.connector_id)
            if key not in owners:
                owners.append(key)
        source_claims = routine_scalar_source_claims.project_scalar_source_claims(
            named,
            source_inventory=copy.deepcopy(self.inventory),
            source_pins=self.pins(),
            class_claims=(
                routine_scalar_source_claims.SourceClassClaim(
                    TRIM_CLASS,
                    "release",
                    self.release_revision,
                    self.trim_locator(),
                ),
            ),
            member_bindings=tuple(
                routine_scalar_source_claims.SourceMemberBinding(
                    owner_kind, owner_id, TRIM_CLASS, owner_id
                )
                for owner_kind, owner_id in owners
            ),
        )
        return named, source_claims

    def direct_pair(self):
        coordinate = self.coordinate()
        abi = routine_scalar_abi.ScalarAbiProjection(
            "G36-05-16-BOUND-SCALAR-TEST",
            1,
            (
                routine_scalar_abi.ScalarParameterAbiRow(
                    "gain",
                    (coordinate,),
                    routine_scalar_abi.ScalarAbiType(
                        "real",
                        alias_type_id="gain_type",
                        quantity="dimensionless",
                        unit="1",
                    ),
                    "assignment",
                    1.5,
                ),
            ),
            (
                routine_scalar_abi.ScalarConnectorAbiRow(
                    "signal",
                    (),
                    routine_scalar_abi.ScalarAbiType("integer"),
                    "input",
                ),
            ),
        )
        named = routine_scalar_names.allocate_scalar_names(abi)
        parameter = named.parameters[0]
        connector = named.connectors[0]
        source_claims = routine_scalar_source_claims.ScalarSourceClaimProjection(
            named.canonical_id,
            named.revision,
            (
                routine_scalar_source_claims.ScalarParameterSourceClaim(
                    parameter.scalar_name,
                    parameter.parameter_id,
                    parameter.coordinates,
                    TRIM_CLASS,
                    "gain",
                    "release",
                    self.release_revision,
                    self.trim_locator(),
                ),
            ),
            (
                routine_scalar_source_claims.ScalarConnectorSourceClaim(
                    connector.scalar_name,
                    connector.connector_id,
                    connector.coordinates,
                    TRIM_CLASS,
                    "signal",
                    "release",
                    self.release_revision,
                    self.trim_locator(),
                ),
            ),
        )
        return named, source_claims

    def extra_parameter_claim(self, owner_id="extra"):
        return routine_scalar_source_claims.ScalarParameterSourceClaim(
            self.scalar_name("p_", owner_id),
            owner_id,
            (),
            TRIM_CLASS,
            owner_id,
            "release",
            self.release_revision,
            self.trim_locator(),
        )

    def assert_bind_error(self, named, source_claims, expected_code):
        attempts = []
        for _ in range(2):
            with mock.patch.object(
                routine_bound_scalars,
                "BoundSourceClaim",
                side_effect=AssertionError("source claim allocated"),
            ), mock.patch.object(
                routine_bound_scalars,
                "BoundScalarParameterRow",
                side_effect=AssertionError("parameter row allocated"),
            ), mock.patch.object(
                routine_bound_scalars,
                "BoundScalarConnectorRow",
                side_effect=AssertionError("connector row allocated"),
            ), mock.patch.object(
                routine_bound_scalars,
                "BoundScalarProjection",
                side_effect=AssertionError("projection allocated"),
            ):
                with self.assertRaises(
                    routine_bound_scalars.BoundScalarError
                ) as caught:
                    routine_bound_scalars.bind_scalar_source_claims(
                        named, source_claims
                    )
            diagnostics = caught.exception.diagnostics
            self.assertTrue(diagnostics)
            self.assertEqual(diagnostics, tuple(sorted(diagnostics)))
            self.assertIn(expected_code, {item.code for item in diagnostics})
            self.assertNotIn("Traceback", str(caught.exception))
            attempts.append(diagnostics)
        self.assertEqual(attempts[0], attempts[1])
        with self.assertRaises(FrozenInstanceError):
            setattr(attempts[0][0], "code", "changed")
        return attempts[0]

    def test_end_to_end_release_join_preserves_full_payload_and_named_order(self):
        self.assertEqual(self.release_revision, RELEASE_REVISION)
        self.assertEqual(self.development_revision, DEVELOPMENT_REVISION)
        inventory_release = next(
            item
            for item in self.inventory["snapshots"]
            if item["role"] == "release"
        )
        inventory_trim = next(
            item for item in inventory_release["files"] if item["path"] == TRIM_PATH
        )
        self.assertEqual(inventory_trim["git_blob_sha1"], TRIM_BLOB)

        named, source_claims = self.through_existing_stages(matrix_connector=True)
        result = routine_bound_scalars.bind_scalar_source_claims(
            named, source_claims
        )

        self.assertEqual(result.canonical_id, named.canonical_id)
        self.assertEqual(result.revision, named.revision)
        self.assertEqual(
            [row.scalar_name for row in result.parameters],
            [row.scalar_name for row in named.parameters],
        )
        self.assertEqual(
            [row.scalar_name for row in result.connectors],
            [row.scalar_name for row in named.connectors],
        )
        for bound, original in zip(result.parameters, named.parameters, strict=True):
            self.assertEqual(
                (
                    bound.scalar_name,
                    bound.parameter_id,
                    bound.coordinates,
                    bound.type,
                    bound.source,
                    bound.value,
                ),
                (
                    original.scalar_name,
                    original.parameter_id,
                    original.coordinates,
                    original.type,
                    original.source,
                    original.value,
                ),
            )
        for bound, original in zip(result.connectors, named.connectors, strict=True):
            self.assertEqual(
                (
                    bound.scalar_name,
                    bound.connector_id,
                    bound.coordinates,
                    bound.type,
                    bound.direction,
                ),
                (
                    original.scalar_name,
                    original.connector_id,
                    original.coordinates,
                    original.type,
                    original.direction,
                ),
            )

        sample_period = self.by_id(
            result.parameters, "parameter_id", "sample_period_s"
        )
        self.assertEqual(sample_period.type, routine_scalar_abi.ScalarAbiType("real"))
        self.assertEqual((sample_period.source, sample_period.value), ("default", 60.0))
        zone_count = self.by_id(result.parameters, "parameter_id", "zone_count")
        self.assertEqual((zone_count.source, zone_count.value), ("assignment", 3))
        zone_offset = self.by_id(result.parameters, "parameter_id", "zone_offsets")
        self.assertEqual(
            zone_offset.type,
            routine_scalar_abi.ScalarAbiType(
                "real",
                alias_type_id="temperature",
                quantity="thermodynamic_temperature",
                unit="K",
                display_unit="degC",
            ),
        )
        supply_air_flow = self.by_id(
            result.connectors, "connector_id", "supply_air_flow"
        )
        self.assertEqual(supply_air_flow.direction, "input")
        zone_command = self.by_id(
            result.connectors, "connector_id", "zone_commands"
        )
        self.assertEqual(zone_command.direction, "output")

        for row in (*result.parameters, *result.connectors):
            self.assertEqual(row.source_claim.canonical_class_path, TRIM_CLASS)
            self.assertEqual(row.source_claim.snapshot, "release")
            self.assertEqual(row.source_claim.revision, RELEASE_REVISION)
            self.assertEqual(row.source_claim.file.path, TRIM_PATH)
            self.assertEqual(row.source_claim.file.git_blob_sha1, TRIM_BLOB)
            self.assertIs(result.row_for_scalar(row.scalar_name), row)

    def test_array_matrix_order_and_owner_level_claim_repetition_survive(self):
        named, source_claims = self.through_existing_stages(matrix_connector=True)
        result = routine_bound_scalars.bind_scalar_source_claims(
            named, source_claims
        )
        matrices = [
            row for row in result.parameters if row.parameter_id == "matrix_weights"
        ]
        self.assertEqual(
            [tuple(item.member_id for item in row.coordinates) for row in matrices],
            [
                ("north-zone", "primary"),
                ("north-zone", "secondary"),
                ("south-zone", "primary"),
                ("south-zone", "secondary"),
                ("core-zone", "primary"),
                ("core-zone", "secondary"),
            ],
        )
        feedback = [
            row for row in result.connectors if row.connector_id == "matrix_feedback"
        ]
        self.assertEqual(len(feedback), 6)
        self.assertEqual(
            [row.scalar_name for row in matrices],
            [
                row.scalar_name
                for row in named.parameters
                if row.parameter_id == "matrix_weights"
            ],
        )
        self.assertTrue(
            all(row.source_claim.source_member == "matrix_weights" for row in matrices)
        )
        self.assertTrue(
            all(row.source_claim.source_member == "matrix_feedback" for row in feedback)
        )

    def test_enum_abi_identity_is_distinct_from_owner_source_claim(self):
        named, source_claims = self.through_existing_stages()
        result = routine_bound_scalars.bind_scalar_source_claims(
            named, source_claims
        )
        initial_mode = self.by_id(
            result.parameters, "parameter_id", "initial_mode"
        )
        self.assertEqual(
            initial_mode.type, routine_scalar_abi.ScalarEnumAbiType(ENUM_CLASS)
        )
        self.assertEqual(
            initial_mode.value, routine_scalar_abi.ScalarEnumAbiValue(2)
        )
        self.assertEqual(initial_mode.source_claim.canonical_class_path, TRIM_CLASS)
        self.assertEqual(initial_mode.source_claim.source_member, "initial_mode")
        self.assertNotEqual(
            initial_mode.type.canonical_class_path,
            initial_mode.source_claim.canonical_class_path,
        )
        named_initial_mode = self.by_id(
            named.parameters, "parameter_id", "initial_mode"
        )
        object.__setattr__(named_initial_mode.type, "canonical_class_path", "changed")
        object.__setattr__(named_initial_mode.value, "ordinal", 99)
        self.assertEqual(
            initial_mode.type, routine_scalar_abi.ScalarEnumAbiType(ENUM_CLASS)
        )
        self.assertEqual(
            initial_mode.value, routine_scalar_abi.ScalarEnumAbiValue(2)
        )

    def test_source_claim_row_order_does_not_control_output(self):
        named, source_claims = self.through_existing_stages(matrix_connector=True)
        expected = routine_bound_scalars.bind_scalar_source_claims(
            named, source_claims
        )
        reversed_claims = replace(
            source_claims,
            parameters=tuple(reversed(source_claims.parameters)),
            connectors=tuple(reversed(source_claims.connectors)),
        )
        self.assertEqual(
            routine_bound_scalars.bind_scalar_source_claims(named, reversed_claims),
            expected,
        )

    def test_projection_identity_mismatches_fail_atomically(self):
        named, source_claims = self.direct_pair()
        self.assert_bind_error(
            named,
            replace(source_claims, canonical_id="G36-05-16-OTHER"),
            "canonical_id_mismatch",
        )
        self.assert_bind_error(
            named,
            replace(source_claims, revision=2),
            "revision_mismatch",
        )
        self.assert_bind_error(
            replace(named, revision=True), source_claims, "invalid_metadata"
        )

    def test_missing_extra_duplicate_wrong_kind_and_namespace_claims_fail(self):
        named, source_claims = self.direct_pair()
        parameter_claim = source_claims.parameters[0]
        connector_claim = source_claims.connectors[0]
        cases = (
            (
                "missing",
                named,
                replace(source_claims, parameters=()),
                "missing_source_claim",
            ),
            (
                "extra",
                named,
                replace(
                    source_claims,
                    parameters=(parameter_claim, self.extra_parameter_claim()),
                ),
                "extra_source_claim",
            ),
            (
                "duplicate claim",
                named,
                replace(
                    source_claims,
                    parameters=(parameter_claim, parameter_claim),
                ),
                "duplicate_scalar_name",
            ),
            (
                "duplicate named row",
                replace(named, parameters=(named.parameters[0], named.parameters[0])),
                source_claims,
                "duplicate_scalar_name",
            ),
            (
                "wrong kind claim row",
                named,
                replace(source_claims, parameters=(connector_claim,)),
                "invalid_source_claim_row",
            ),
            (
                "wrong kind named row",
                replace(named, parameters=(named.connectors[0],)),
                source_claims,
                "invalid_named_row",
            ),
        )
        for label, case_named, case_claims, code in cases:
            with self.subTest(label=label):
                self.assert_bind_error(case_named, case_claims, code)

        confused = routine_scalar_source_claims.ScalarConnectorSourceClaim(
            parameter_claim.scalar_name,
            parameter_claim.parameter_id,
            parameter_claim.coordinates,
            parameter_claim.canonical_class_path,
            parameter_claim.source_member,
            parameter_claim.snapshot,
            parameter_claim.revision,
            parameter_claim.file,
        )
        self.assert_bind_error(
            named,
            replace(
                source_claims,
                parameters=(),
                connectors=(connector_claim, confused),
            ),
            "namespace_confusion",
        )

        colliding_connector = replace(
            connector_claim,
            scalar_name=parameter_claim.scalar_name,
            connector_id=parameter_claim.parameter_id,
            coordinates=parameter_claim.coordinates,
        )
        self.assert_bind_error(
            named,
            replace(source_claims, connectors=(colliding_connector,)),
            "cross_kind_collision",
        )

    def test_owner_and_complete_coordinate_mismatches_fail(self):
        named, source_claims = self.direct_pair()
        claim = source_claims.parameters[0]
        coordinate = claim.coordinates[0]
        cases = (
            (replace(claim, parameter_id="other"), "owner_mismatch"),
            (
                replace(
                    claim,
                    coordinates=(replace(coordinate, dimension_id="other_dimension"),),
                ),
                "dimension_mismatch",
            ),
            (
                replace(
                    claim,
                    coordinates=(replace(coordinate, member_id="other_member"),),
                ),
                "member_mismatch",
            ),
            (
                replace(
                    claim,
                    coordinates=(replace(coordinate, ordinal=99),),
                ),
                "ordinal_mismatch",
            ),
            (replace(claim, coordinates=()), "coordinate_count_mismatch"),
        )
        for changed, code in cases:
            with self.subTest(code=code):
                self.assert_bind_error(
                    named,
                    replace(source_claims, parameters=(changed,)),
                    code,
                )

    def test_malformed_record_container_abi_and_source_values_fail_cleanly(self):
        named, source_claims = self.direct_pair()
        parameter = named.parameters[0]
        connector = named.connectors[0]
        claim = source_claims.parameters[0]

        class NamedProjectionSubclass(routine_scalar_names.NamedScalarProjection):
            pass

        class ClaimProjectionSubclass(
            routine_scalar_source_claims.ScalarSourceClaimProjection
        ):
            pass

        named_subclass = NamedProjectionSubclass(
            named.canonical_id,
            named.revision,
            named.parameters,
            named.connectors,
        )
        claim_subclass = ClaimProjectionSubclass(
            source_claims.canonical_id,
            source_claims.revision,
            source_claims.parameters,
            source_claims.connectors,
        )
        malformed_enum_parameter = replace(
            parameter,
            type=routine_scalar_abi.ScalarEnumAbiType(ENUM_CLASS),
            value=routine_scalar_abi.ScalarEnumAbiValue(0),
        )
        cases = (
            (cast(Any, object()), source_claims, "invalid_named_projection"),
            (named, cast(Any, object()), "invalid_source_claim_projection"),
            (named_subclass, source_claims, "invalid_named_projection"),
            (named, claim_subclass, "invalid_source_claim_projection"),
            (
                replace(named, parameters=list(named.parameters)),
                source_claims,
                "invalid_named_container",
            ),
            (
                named,
                replace(source_claims, connectors=list(source_claims.connectors)),
                "invalid_source_claim_container",
            ),
            (
                replace(named, parameters=(replace(parameter, coordinates=[]),)),
                source_claims,
                "invalid_coordinates",
            ),
            (
                replace(named, parameters=(replace(parameter, type=object()),)),
                source_claims,
                "invalid_abi_type",
            ),
            (
                replace(
                    named,
                    parameters=(
                        replace(
                            parameter,
                            type=routine_scalar_abi.ScalarAbiType("string"),
                        ),
                    ),
                ),
                source_claims,
                "invalid_abi_type",
            ),
            (
                replace(
                    named,
                    parameters=(
                        replace(
                            parameter,
                            type=routine_scalar_abi.ScalarAbiType(
                                cast(Any, [])
                            ),
                        ),
                    ),
                ),
                source_claims,
                "invalid_abi_type",
            ),
            (
                replace(named, parameters=(malformed_enum_parameter,)),
                source_claims,
                "invalid_abi_value",
            ),
            (
                replace(named, parameters=(replace(parameter, source="caller"),)),
                source_claims,
                "invalid_parameter_source",
            ),
            (
                replace(
                    named,
                    parameters=(
                        replace(parameter, source=cast(Any, [])),
                    ),
                ),
                source_claims,
                "invalid_parameter_source",
            ),
            (
                replace(named, connectors=(replace(connector, direction="sideways"),)),
                source_claims,
                "invalid_direction",
            ),
            (
                named,
                replace(
                    source_claims,
                    parameters=(replace(claim, canonical_class_path="Bad.Class"),),
                ),
                "invalid_source_class",
            ),
            (
                named,
                replace(
                    source_claims,
                    parameters=(replace(claim, source_member="bad.member"),),
                ),
                "invalid_source_member",
            ),
            (
                named,
                replace(
                    source_claims,
                    parameters=(replace(claim, snapshot="candidate"),),
                ),
                "invalid_source_snapshot",
            ),
            (
                named,
                replace(
                    source_claims,
                    parameters=(replace(claim, revision="ABC"),),
                ),
                "invalid_source_revision",
            ),
            (
                named,
                replace(
                    source_claims,
                    parameters=(replace(claim, file=object()),),
                ),
                "invalid_file_locator",
            ),
            (
                named,
                replace(
                    source_claims,
                    parameters=(
                        replace(
                            claim,
                            file=routine_scalar_source_claims.SourceFileLocator(
                                "Buildings/Controls/OBC/ASHRAE/G36/../escape.mo",
                                TRIM_BLOB,
                            ),
                        ),
                    ),
                ),
                "invalid_source_path",
            ),
            (
                named,
                replace(
                    source_claims,
                    parameters=(
                        replace(
                            claim,
                            file=routine_scalar_source_claims.SourceFileLocator(
                                TRIM_PATH, "sha1:ABC"
                            ),
                        ),
                    ),
                ),
                "invalid_source_blob",
            ),
        )
        for case_named, case_claims, code in cases:
            with self.subTest(code=code):
                self.assert_bind_error(case_named, case_claims, code)

    def test_output_is_frozen_recursive_detached_and_binding_is_pure(self):
        named, source_claims = self.direct_pair()
        with mock.patch(
            "builtins.open", side_effect=AssertionError("file access")
        ), mock.patch(
            "pathlib.Path.open", side_effect=AssertionError("path access")
        ), mock.patch(
            "pathlib.Path.read_text", side_effect=AssertionError("path access")
        ), mock.patch(
            "socket.socket", side_effect=AssertionError("network access")
        ), mock.patch(
            "urllib.request.urlopen", side_effect=AssertionError("URL access")
        ), mock.patch(
            "subprocess.run", side_effect=AssertionError("subprocess access")
        ), mock.patch(
            "subprocess.Popen", side_effect=AssertionError("subprocess access")
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
            result = routine_bound_scalars.bind_scalar_source_claims(
                named, source_claims
            )

        with self.assertRaises(FrozenInstanceError):
            setattr(result, "revision", 2)
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0], "parameter_id", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0].coordinates[0], "member_id", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0].type, "quantity", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0].source_claim.file, "path", "changed")

        named_parameter = named.parameters[0]
        source_parameter = source_claims.parameters[0]
        object.__setattr__(named, "canonical_id", "changed")
        object.__setattr__(named_parameter, "parameter_id", "changed")
        object.__setattr__(named_parameter.coordinates[0], "member_id", "changed")
        object.__setattr__(named_parameter.type, "quantity", "changed")
        object.__setattr__(named_parameter, "value", 99.0)
        object.__setattr__(source_parameter, "canonical_class_path", "changed")
        object.__setattr__(source_parameter, "source_member", "changed")
        object.__setattr__(source_parameter.file, "path", "changed")
        object.__setattr__(source_parameter.file, "git_blob_sha1", "sha1:" + "0" * 40)

        bound = result.parameters[0]
        self.assertEqual(result.canonical_id, "G36-05-16-BOUND-SCALAR-TEST")
        self.assertEqual(bound.parameter_id, "gain")
        self.assertEqual(bound.coordinates[0].member_id, "north")
        self.assertEqual(
            cast(routine_scalar_abi.ScalarAbiType, bound.type).quantity,
            "dimensionless",
        )
        self.assertEqual(bound.value, 1.5)
        self.assertEqual(bound.source_claim.canonical_class_path, TRIM_CLASS)
        self.assertEqual(bound.source_claim.source_member, "gain")
        self.assertEqual(bound.source_claim.file.path, TRIM_PATH)
        self.assertEqual(bound.source_claim.file.git_blob_sha1, TRIM_BLOB)

        def assert_no_mutable_values(value):
            self.assertNotIsInstance(value, (dict, list, set))
            if is_dataclass(value):
                for field in fields(value):
                    assert_no_mutable_values(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    assert_no_mutable_values(item)

        assert_no_mutable_values(result)

    def test_unknown_lookup_is_bounded(self):
        named, source_claims = self.direct_pair()
        result = routine_bound_scalars.bind_scalar_source_claims(
            named, source_claims
        )
        with self.assertRaisesRegex(KeyError, "unknown scalar name") as caught:
            result.row_for_scalar("c_" + "a" * 10_000)
        self.assertLess(len(str(caught.exception)), 240)

    def test_output_shape_excludes_deferred_contracts_and_inputs_stay_unchanged(self):
        named, source_claims = self.direct_pair()
        result = routine_bound_scalars.bind_scalar_source_claims(
            named, source_claims
        )
        self.assertEqual(
            tuple(field.name for field in fields(routine_bound_scalars.BoundSourceClaim)),
            (
                "canonical_class_path",
                "source_member",
                "snapshot",
                "revision",
                "file",
            ),
        )
        self.assertEqual(
            tuple(
                field.name
                for field in fields(routine_bound_scalars.BoundScalarParameterRow)
            ),
            (
                "scalar_name",
                "parameter_id",
                "coordinates",
                "type",
                "source",
                "value",
                "source_claim",
            ),
        )
        self.assertEqual(
            tuple(
                field.name
                for field in fields(routine_bound_scalars.BoundScalarConnectorRow)
            ),
            (
                "scalar_name",
                "connector_id",
                "coordinates",
                "type",
                "direction",
                "source_claim",
            ),
        )
        self.assertEqual(
            tuple(
                field.name
                for field in fields(routine_bound_scalars.BoundScalarProjection)
            ),
            ("canonical_id", "revision", "parameters", "connectors"),
        )
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
        self.assertEqual(
            tuple(
                field.name
                for field in fields(
                    routine_scalar_source_claims.ScalarSourceClaimProjection
                )
            ),
            ("canonical_id", "revision", "parameters", "connectors"),
        )
        self.assertEqual(
            tuple(
                field.name
                for field in fields(
                    routine_scalar_source_claims.ScalarParameterSourceClaim
                )
            ),
            (
                "scalar_name",
                "parameter_id",
                "coordinates",
                "canonical_class_path",
                "source_member",
                "snapshot",
                "revision",
                "file",
            ),
        )
        self.assertEqual(
            tuple(
                field.name
                for field in fields(
                    routine_scalar_source_claims.ScalarConnectorSourceClaim
                )
            ),
            (
                "scalar_name",
                "connector_id",
                "coordinates",
                "canonical_class_path",
                "source_member",
                "snapshot",
                "revision",
                "file",
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
            "schema",
            "schema_id",
            "serialized_bytes",
            "serialized",
            "json",
            "json_format",
            "content_id",
            "content_hash",
            "projection_hash",
            "deployment_id",
            "engine_path",
            "iri",
            "graph_node",
            "block",
            "port",
            "edge",
            "source_span",
            "line",
            "column",
            "dependency_closure",
            "semantic_binding",
            "point_binding",
            "runtime",
            "persistence",
            "production_status",
            "registry_reference",
            "generated_artifact_path",
            "artifact_path",
            "manifest",
            "bundle",
            "declaration",
            "source_parser",
        }
        self.assertTrue(names.isdisjoint(forbidden))
        normalized_doc = " ".join((routine_bound_scalars.__doc__ or "").split())
        function_doc = " ".join(
            (routine_bound_scalars.bind_scalar_source_claims.__doc__ or "").split()
        )
        self.assertIn("validated internal join", normalized_doc)
        self.assertIn("caller claims", normalized_doc)
        self.assertIn("rather than verified declarations", function_doc)


if __name__ == "__main__":
    unittest.main()
