import copy
import json
import os
import unittest
from dataclasses import FrozenInstanceError, fields, is_dataclass, replace
from pathlib import Path
from typing import Any, cast
from unittest import mock

from tools.lint import (
    routine_resolution,
    routine_scalar_abi,
    routine_scalar_names,
    routine_scalar_source_claims,
)


REPO_ROOT = Path(__file__).parents[3]
INVENTORY_PATH = REPO_ROOT / "routines" / "g36" / "source-inventory.json"
RELEASE_PIN_PATH = REPO_ROOT / "routines" / "g36" / "SOURCE_RELEASE_PIN"
DEVELOPMENT_PIN_PATH = (
    REPO_ROOT / "routines" / "g36" / "SOURCE_DEVELOPMENT_PIN"
)
TRIM_CLASS = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TrimAndRespond"
TRIM_PATH = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TrimAndRespond.mo"
TRIM_BLOB = "sha1:028439a4fb478fc041d703a092d5186f5861eb03"
TIME_CLASS = "Buildings.Controls.OBC.ASHRAE.G36.Generic.TimeSuppression"
TIME_PATH = "Buildings/Controls/OBC/ASHRAE/G36/Generic/TimeSuppression.mo"
DEVELOPMENT_CLASS = "Buildings.Controls.OBC.ASHRAE.G36.Plants.Chillers.Controller"
DEVELOPMENT_PATH = (
    "Buildings/Controls/OBC/ASHRAE/G36/Plants/Chillers/Controller.mo"
)


class RoutineScalarSourceClaimTests(unittest.TestCase):
    def setUp(self):
        self.inventory = json.loads(INVENTORY_PATH.read_text(encoding="utf-8"))
        self.release_revision = RELEASE_PIN_PATH.read_text(encoding="utf-8").strip()
        self.development_revision = DEVELOPMENT_PIN_PATH.read_text(
            encoding="utf-8"
        ).strip()
        self.pins = (
            routine_scalar_source_claims.SourcePin(
                "release", self.release_revision
            ),
            routine_scalar_source_claims.SourcePin(
                "development", self.development_revision
            ),
        )

    @staticmethod
    def interface():
        return {
            "schema": "cxf-library/routine-interface/v3",
            "canonical_id": "G36-05-16-SOURCE-CLAIM-TEST",
            "revision": 1,
            "types": [],
            "dimensions": [
                {
                    "id": "request_pair",
                    "extent": {"kind": "fixed", "value": 2},
                    "members": ["first", "second"],
                }
            ],
            "parameters": [
                {
                    "id": "sample_period_s",
                    "type": {"kind": "primitive", "primitive": "real"},
                    "shape": {"kind": "scalar"},
                    "configurability": "fixed",
                    "default": 60.0,
                }
            ],
            "connectors": [
                {
                    "id": "requests",
                    "direction": "input",
                    "type": {"kind": "primitive", "primitive": "integer"},
                    "shape": {
                        "kind": "array",
                        "dimensions": ["request_pair"],
                    },
                    "presence": {"kind": "always"},
                }
            ],
        }

    @staticmethod
    def specialization():
        return {
            "schema": "cxf-library/routine-specialization/v1",
            "canonical_id": "G36-05-16-SOURCE-CLAIM-TEST",
            "revision": 1,
            "parameters": [],
            "members": [],
        }

    def named_through_stages(self, interface=None, specialization=None):
        resolved = routine_resolution.resolve_specialization(
            self.interface() if interface is None else interface,
            self.specialization() if specialization is None else specialization,
        )
        abi = routine_scalar_abi.project_scalar_abi(resolved)
        return routine_scalar_names.allocate_scalar_names(abi)

    @staticmethod
    def coordinate(member_id="first", ordinal=0, dimension_id="request_pair"):
        return routine_scalar_abi.ScalarCoordinate(
            dimension_id, member_id, ordinal
        )

    @classmethod
    def named_parameter(
        cls,
        parameter_id="setting",
        scalar_name="p_73657474696e67",
        coordinates=(),
    ):
        return routine_scalar_names.NamedScalarParameterRow(
            scalar_name,
            parameter_id,
            coordinates,
            routine_scalar_abi.ScalarAbiType("real"),
            "default",
            1.0,
        )

    @classmethod
    def named_connector(
        cls,
        connector_id="signal",
        scalar_name="c_7369676e616c",
        coordinates=(),
    ):
        return routine_scalar_names.NamedScalarConnectorRow(
            scalar_name,
            connector_id,
            coordinates,
            routine_scalar_abi.ScalarAbiType("real"),
            "input",
        )

    @classmethod
    def direct_named(cls, parameters=None, connectors=None):
        return routine_scalar_names.NamedScalarProjection(
            "G36-05-16-DIRECT-SOURCE-CLAIM-TEST",
            1,
            (cls.named_parameter(),) if parameters is None else parameters,
            (cls.named_connector(),) if connectors is None else connectors,
        )

    def locator(self, role, path):
        snapshot = next(
            snapshot
            for snapshot in self.inventory["snapshots"]
            if snapshot["role"] == role
        )
        row = next(row for row in snapshot["files"] if row["path"] == path)
        return routine_scalar_source_claims.SourceFileLocator(
            path, row["git_blob_sha1"]
        )

    def class_claim(
        self,
        *,
        class_path=TRIM_CLASS,
        role="release",
        revision=None,
        locator=None,
    ):
        revision = revision or (
            self.release_revision
            if role == "release"
            else self.development_revision
        )
        locator = locator or self.locator(role, TRIM_PATH)
        return routine_scalar_source_claims.SourceClassClaim(
            class_path, role, revision, locator
        )

    @staticmethod
    def binding(owner_kind, owner_id, source_member, class_path=TRIM_CLASS):
        return routine_scalar_source_claims.SourceMemberBinding(
            owner_kind, owner_id, class_path, source_member
        )

    def valid_arguments(self, named=None):
        named = self.named_through_stages() if named is None else named
        return {
            "named_projection": named,
            "source_inventory": copy.deepcopy(self.inventory),
            "source_pins": self.pins,
            "class_claims": (self.class_claim(),),
            "member_bindings": (
                self.binding("parameter", "sample_period_s", "samplePeriod"),
                self.binding("connector", "requests", "numOfReq"),
            ),
        }

    def project(self, arguments):
        return routine_scalar_source_claims.project_scalar_source_claims(
            arguments["named_projection"],
            source_inventory=arguments["source_inventory"],
            source_pins=arguments["source_pins"],
            class_claims=arguments["class_claims"],
            member_bindings=arguments["member_bindings"],
        )

    def assert_projection_error(self, arguments, expected_code):
        attempts = []
        for _ in range(2):
            with mock.patch.object(
                routine_scalar_source_claims,
                "ScalarParameterSourceClaim",
                side_effect=AssertionError("parameter output row allocated"),
            ), mock.patch.object(
                routine_scalar_source_claims,
                "ScalarConnectorSourceClaim",
                side_effect=AssertionError("connector output row allocated"),
            ), mock.patch.object(
                routine_scalar_source_claims,
                "ScalarSourceClaimProjection",
                side_effect=AssertionError("output projection allocated"),
            ):
                with self.assertRaises(
                    routine_scalar_source_claims.SourceClaimError
                ) as caught:
                    self.project(arguments)
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

    def test_release_trim_and_respond_projection_and_lookups_are_exact(self):
        arguments = self.valid_arguments()
        result = self.project(arguments)
        named = arguments["named_projection"]

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
        parameter = result.parameters[0]
        self.assertEqual(parameter.parameter_id, "sample_period_s")
        self.assertEqual(parameter.source_member, "samplePeriod")
        self.assertEqual(parameter.canonical_class_path, TRIM_CLASS)
        self.assertEqual(parameter.snapshot, "release")
        self.assertEqual(parameter.revision, self.release_revision)
        self.assertEqual(parameter.file.path, TRIM_PATH)
        self.assertEqual(parameter.file.git_blob_sha1, TRIM_BLOB)

        connector_names = tuple(row.scalar_name for row in result.connectors)
        self.assertEqual([row.source_member for row in result.connectors], ["numOfReq"] * 2)
        self.assertEqual(
            [row.coordinates[0].member_id for row in result.connectors],
            ["first", "second"],
        )
        for row in (*result.parameters, *result.connectors):
            self.assertIs(result.claim_for_scalar(row.scalar_name), row)
        self.assertEqual(
            result.scalar_names_for_source(
                "parameter", TRIM_CLASS, "samplePeriod"
            ),
            (parameter.scalar_name,),
        )
        self.assertEqual(
            result.scalar_names_for_source("connector", TRIM_CLASS, "numOfReq"),
            connector_names,
        )

    def test_vector_and_matrix_leaves_share_owner_claim_without_source_indices(self):
        rows = (
            self.named_connector(
                "matrix",
                "c_6d6174726978_726f7773_6e6f727468_636f6c73_6669727374",
                (
                    self.coordinate("north", 0, "rows"),
                    self.coordinate("first", 0, "cols"),
                ),
            ),
            self.named_connector(
                "matrix",
                "c_6d6174726978_726f7773_6e6f727468_636f6c73_7365636f6e64",
                (
                    self.coordinate("north", 0, "rows"),
                    self.coordinate("second", 1, "cols"),
                ),
            ),
            self.named_connector(
                "matrix",
                "c_6d6174726978_726f7773_736f757468_636f6c73_6669727374",
                (
                    self.coordinate("south", 1, "rows"),
                    self.coordinate("first", 0, "cols"),
                ),
            ),
        )
        named = self.direct_named(parameters=(), connectors=rows)
        arguments = self.valid_arguments(named)
        arguments["member_bindings"] = (
            self.binding("connector", "matrix", "uHol"),
        )
        result = self.project(arguments)

        expected = tuple(row.scalar_name for row in rows)
        self.assertEqual(
            result.scalar_names_for_source("connector", TRIM_CLASS, "uHol"),
            expected,
        )
        self.assertEqual(
            tuple(row.scalar_name for row in result.connectors), expected
        )
        field_names = {field.name for field in fields(result.connectors[0])}
        self.assertNotIn("source_index", field_names)
        self.assertNotIn("indices", field_names)

    def test_development_snapshot_isolated_from_release(self):
        named = self.direct_named(connectors=())
        development_claim = self.class_claim(
            class_path=DEVELOPMENT_CLASS,
            role="development",
            locator=self.locator("development", DEVELOPMENT_PATH),
        )
        arguments = self.valid_arguments(named)
        arguments["class_claims"] = (development_claim,)
        arguments["member_bindings"] = (
            self.binding(
                "parameter",
                "setting",
                "setpoint",
                class_path=DEVELOPMENT_CLASS,
            ),
        )
        result = self.project(arguments)
        self.assertEqual(result.parameters[0].snapshot, "development")
        self.assertEqual(result.parameters[0].file.path, DEVELOPMENT_PATH)

        release_claim = replace(
            development_claim,
            snapshot="release",
            revision=self.release_revision,
        )
        arguments["class_claims"] = (release_claim,)
        self.assert_projection_error(arguments, "absent_file_locator")

    def test_source_pin_shape_roles_and_revisions_fail_closed(self):
        cases = {}
        arguments = self.valid_arguments()
        cases["container"] = (list(arguments["source_pins"]), "invalid_source_pins")
        cases["missing"] = ((arguments["source_pins"][0],), "missing_source_pin")
        cases["duplicate role"] = (
            (
                arguments["source_pins"][0],
                routine_scalar_source_claims.SourcePin(
                    "release", self.development_revision
                ),
            ),
            "duplicate_source_pin",
        )
        cases["duplicate revision"] = (
            (
                arguments["source_pins"][0],
                routine_scalar_source_claims.SourcePin(
                    "development", self.release_revision
                ),
            ),
            "duplicate_source_revision",
        )
        cases["wrong role"] = (
            (
                routine_scalar_source_claims.SourcePin(
                    "candidate", self.release_revision
                ),
                arguments["source_pins"][1],
            ),
            "invalid_source_role",
        )
        cases["uppercase revision"] = (
            (
                routine_scalar_source_claims.SourcePin(
                    "release", self.release_revision.upper()
                ),
                arguments["source_pins"][1],
            ),
            "invalid_source_revision",
        )
        cases["pin mismatch"] = (
            (
                routine_scalar_source_claims.SourcePin("release", "0" * 40),
                arguments["source_pins"][1],
            ),
            "inventory_snapshot_revision",
        )

        for name, (pins, code) in cases.items():
            with self.subTest(name=name):
                case = self.valid_arguments()
                case["source_pins"] = pins
                self.assert_projection_error(case, code)

    def test_malformed_inventory_constants_snapshots_and_files_are_rejected(self):
        cases = []

        inventory = copy.deepcopy(self.inventory)
        inventory["schema"] = "wrong"
        cases.append(("constant", inventory, "inventory_constant"))

        inventory = copy.deepcopy(self.inventory)
        inventory["snapshots"].reverse()
        cases.append(("snapshot order", inventory, "inventory_snapshot_role"))

        inventory = copy.deepcopy(self.inventory)
        inventory["snapshots"][0]["revision"] = "0" * 40
        cases.append(
            ("snapshot revision", inventory, "inventory_snapshot_revision")
        )

        inventory = copy.deepcopy(self.inventory)
        inventory["snapshots"][0]["files"] = tuple(
            inventory["snapshots"][0]["files"]
        )
        cases.append(("file container", inventory, "invalid_inventory_files"))

        inventory = copy.deepcopy(self.inventory)
        inventory["snapshots"][0]["files"][0].pop("sha256")
        cases.append(("file shape", inventory, "invalid_inventory_file"))

        inventory = copy.deepcopy(self.inventory)
        inventory["snapshots"][0]["files"][0]["path"] = (
            "Buildings/Controls/OBC/ASHRAE/G36/../escape.mo"
        )
        cases.append(("unsafe path", inventory, "unsafe_inventory_path"))

        inventory = copy.deepcopy(self.inventory)
        duplicate = copy.deepcopy(inventory["snapshots"][0]["files"][0])
        inventory["snapshots"][0]["files"].insert(1, duplicate)
        cases.append(("duplicate path", inventory, "duplicate_inventory_path"))

        inventory = copy.deepcopy(self.inventory)
        inventory["snapshots"][0]["files"][0]["git_blob_sha1"] = "SHA1:bad"
        cases.append(("blob", inventory, "invalid_inventory_blob"))

        inventory = copy.deepcopy(self.inventory)
        inventory["snapshots"][0]["file_count"] += 1
        cases.append(("count", inventory, "inventory_count_mismatch"))

        inventory = copy.deepcopy(self.inventory)
        inventory["snapshots"][0]["files"][0:2] = reversed(
            inventory["snapshots"][0]["files"][0:2]
        )
        cases.append(("file order", inventory, "inventory_file_order"))

        for name, value, code in cases:
            with self.subTest(name=name):
                arguments = self.valid_arguments()
                arguments["source_inventory"] = value
                self.assert_projection_error(arguments, code)

    def test_class_claim_revision_locator_and_shape_failures(self):
        package_order = self.locator(
            "release", "Buildings/Controls/OBC/ASHRAE/G36/Generic/package.order"
        )
        cases = {
            "wrong snapshot": (
                replace(self.class_claim(), snapshot="candidate"),
                "invalid_class_snapshot",
            ),
            "wrong revision": (
                replace(self.class_claim(), revision="0" * 40),
                "class_revision_mismatch",
            ),
            "unsafe path": (
                replace(
                    self.class_claim(),
                    file=routine_scalar_source_claims.SourceFileLocator(
                        "Buildings/Controls/OBC/ASHRAE/G36/../escape.mo",
                        TRIM_BLOB,
                    ),
                ),
                "unsafe_class_path",
            ),
            "absent path": (
                replace(
                    self.class_claim(),
                    file=routine_scalar_source_claims.SourceFileLocator(
                        "Buildings/Controls/OBC/ASHRAE/G36/Generic/Absent.mo",
                        "sha1:" + "0" * 40,
                    ),
                ),
                "absent_file_locator",
            ),
            "wrong blob": (
                replace(
                    self.class_claim(),
                    file=routine_scalar_source_claims.SourceFileLocator(
                        TRIM_PATH, "sha1:" + "0" * 40
                    ),
                ),
                "file_blob_mismatch",
            ),
            "non Modelica": (
                replace(self.class_claim(), file=package_order),
                "non_modelica_locator",
            ),
            "malformed class": (
                replace(
                    self.class_claim(),
                    canonical_class_path="Buildings.Controls.OBC.ASHRAE.G36..Bad",
                ),
                "invalid_class_path",
            ),
            "outside class": (
                replace(
                    self.class_claim(),
                    canonical_class_path="Buildings.Controls.OBC.CDL.Reals.Add",
                ),
                "invalid_class_path",
            ),
            "long class segment": (
                replace(
                    self.class_claim(),
                    canonical_class_path=(
                        "Buildings.Controls.OBC.ASHRAE.G36." + "A" * 256
                    ),
                ),
                "invalid_class_path",
            ),
        }
        for name, (claim, code) in cases.items():
            with self.subTest(name=name):
                arguments = self.valid_arguments()
                arguments["class_claims"] = (claim,)
                self.assert_projection_error(arguments, code)

    def test_class_claims_are_unique_exactly_used_and_have_unique_locators(self):
        arguments = self.valid_arguments()
        arguments["class_claims"] = ()
        self.assert_projection_error(arguments, "missing_class_claim")

        arguments = self.valid_arguments()
        arguments["class_claims"] = (
            arguments["class_claims"][0],
            copy.deepcopy(arguments["class_claims"][0]),
        )
        diagnostics = self.assert_projection_error(
            arguments, "duplicate_class_claim"
        )
        self.assertIn("duplicate_file_locator", {item.code for item in diagnostics})
        self.assertIn("ambiguous_class_claim", {item.code for item in diagnostics})

        arguments = self.valid_arguments()
        extra = self.class_claim(
            class_path=TIME_CLASS,
            locator=self.locator("release", TIME_PATH),
        )
        arguments["class_claims"] = (*arguments["class_claims"], extra)
        diagnostics = self.assert_projection_error(arguments, "unused_class_claim")
        self.assertIn("extra_file_locator", {item.code for item in diagnostics})

        named = self.direct_named()
        arguments = self.valid_arguments(named)
        alias_class = "Buildings.Controls.OBC.ASHRAE.G36.Generic.CallerAlias"
        arguments["class_claims"] = (
            self.class_claim(),
            self.class_claim(class_path=alias_class),
        )
        arguments["member_bindings"] = (
            self.binding("parameter", "setting", "samplePeriod"),
            self.binding(
                "connector", "signal", "numOfReq", class_path=alias_class
            ),
        )
        self.assert_projection_error(arguments, "duplicate_file_locator")

    def test_member_bindings_are_complete_owner_level_and_namespace_safe(self):
        cases = []
        arguments = self.valid_arguments()
        cases.append(
            (
                "missing",
                arguments["member_bindings"][:-1],
                "missing_member_binding",
            )
        )
        cases.append(
            (
                "extra",
                (*arguments["member_bindings"], self.binding("connector", "extra", "y")),
                "extra_member_binding",
            )
        )
        cases.append(
            (
                "duplicate",
                (*arguments["member_bindings"], arguments["member_bindings"][0]),
                "duplicate_member_binding",
            )
        )
        cases.append(
            (
                "cross namespace",
                (
                    arguments["member_bindings"][0],
                    self.binding("parameter", "requests", "numOfReq"),
                ),
                "cross_namespace_binding",
            )
        )
        cases.append(
            (
                "bad owner kind",
                (
                    arguments["member_bindings"][0],
                    self.binding("signal", "requests", "numOfReq"),
                ),
                "invalid_owner_kind",
            )
        )
        cases.append(
            (
                "bad member",
                (
                    arguments["member_bindings"][0],
                    self.binding("connector", "requests", "u.Hol"),
                ),
                "invalid_source_member",
            )
        )
        cases.append(
            (
                "long member",
                (
                    arguments["member_bindings"][0],
                    self.binding("connector", "requests", "u" * 256),
                ),
                "invalid_source_member",
            )
        )
        cases.append(
            (
                "missing class",
                (
                    arguments["member_bindings"][0],
                    self.binding(
                        "connector",
                        "requests",
                        "numOfReq",
                        class_path="Buildings.Controls.OBC.ASHRAE.G36.Generic.Other",
                    ),
                ),
                "missing_class_claim",
            )
        )
        for name, bindings, code in cases:
            with self.subTest(name=name):
                case = self.valid_arguments()
                case["member_bindings"] = bindings
                self.assert_projection_error(case, code)

        same_namespace = self.direct_named(
            parameters=(),
            connectors=(
                self.named_connector("first_owner", "c_6669727374"),
                self.named_connector("second_owner", "c_7365636f6e64"),
            ),
        )
        case = self.valid_arguments(same_namespace)
        case["member_bindings"] = (
            self.binding("connector", "first_owner", "uHol"),
            self.binding("connector", "second_owner", "uHol"),
        )
        self.assert_projection_error(case, "duplicate_source_key")

        separate_namespaces = self.direct_named()
        case = self.valid_arguments(separate_namespaces)
        case["member_bindings"] = (
            self.binding("parameter", "setting", "uHol"),
            self.binding("connector", "signal", "uHol"),
        )
        result = self.project(case)
        self.assertEqual(
            result.scalar_names_for_source("parameter", TRIM_CLASS, "uHol"),
            (separate_namespaces.parameters[0].scalar_name,),
        )
        self.assertEqual(
            result.scalar_names_for_source("connector", TRIM_CLASS, "uHol"),
            (separate_namespaces.connectors[0].scalar_name,),
        )

    def test_mapping_order_row_order_and_ordinals_have_bounded_effects(self):
        arguments = self.valid_arguments()
        time_claim = self.class_claim(
            class_path=TIME_CLASS,
            locator=self.locator("release", TIME_PATH),
        )
        arguments["class_claims"] = (arguments["class_claims"][0], time_claim)
        arguments["member_bindings"] = (
            arguments["member_bindings"][0],
            self.binding(
                "connector", "requests", "uSet", class_path=TIME_CLASS
            ),
        )
        first = self.project(arguments)
        reordered = copy.deepcopy(arguments)
        reordered["class_claims"] = tuple(reversed(reordered["class_claims"]))
        reordered["member_bindings"] = tuple(
            reversed(reordered["member_bindings"])
        )
        reordered["source_pins"] = tuple(reversed(reordered["source_pins"]))
        self.assertEqual(first, self.project(reordered))

        alias_class = "Buildings.Controls.OBC.ASHRAE.G36.Generic.CallerAlias"
        invalid = self.valid_arguments(self.direct_named())
        invalid["class_claims"] = (
            self.class_claim(),
            self.class_claim(class_path=alias_class),
        )
        invalid["member_bindings"] = (
            self.binding("parameter", "setting", "samplePeriod"),
            self.binding(
                "connector", "signal", "numOfReq", class_path=alias_class
            ),
        )
        first_diagnostics = self.assert_projection_error(
            invalid, "duplicate_file_locator"
        )
        invalid["class_claims"] = tuple(reversed(invalid["class_claims"]))
        invalid["member_bindings"] = tuple(
            reversed(invalid["member_bindings"])
        )
        second_diagnostics = self.assert_projection_error(
            invalid, "duplicate_file_locator"
        )
        self.assertEqual(first_diagnostics, second_diagnostics)

        north = self.named_parameter(
            "gains", "p_6761696e73_7a6f6e6573_6e6f727468", (
                self.coordinate("north", 0, "zones"),
            )
        )
        south = self.named_parameter(
            "gains", "p_6761696e73_7a6f6e6573_736f757468", (
                self.coordinate("south", 1, "zones"),
            )
        )
        named = self.direct_named(parameters=(north, south), connectors=())
        case = self.valid_arguments(named)
        case["member_bindings"] = (
            self.binding("parameter", "gains", "triAmo"),
        )
        ordered = self.project(case)
        case["named_projection"] = replace(
            named, parameters=(south, north)
        )
        reversed_rows = self.project(case)
        self.assertEqual(
            [row.scalar_name for row in reversed_rows.parameters],
            [south.scalar_name, north.scalar_name],
        )
        self.assertEqual(
            reversed_rows.scalar_names_for_source("parameter", TRIM_CLASS, "triAmo"),
            (south.scalar_name, north.scalar_name),
        )

        changed_north = replace(
            north,
            coordinates=(replace(north.coordinates[0], ordinal=99),),
        )
        changed_south = replace(
            south,
            coordinates=(replace(south.coordinates[0], ordinal=77),),
        )
        case["named_projection"] = replace(
            named, parameters=(changed_north, changed_south)
        )
        changed_ordinals = self.project(case)
        self.assertEqual(
            tuple(row.scalar_name for row in ordered.parameters),
            tuple(row.scalar_name for row in changed_ordinals.parameters),
        )
        self.assertEqual(
            ordered.scalar_names_for_source("parameter", TRIM_CLASS, "triAmo"),
            changed_ordinals.scalar_names_for_source(
                "parameter", TRIM_CLASS, "triAmo"
            ),
        )
        self.assertEqual(changed_ordinals.parameters[0].coordinates[0].ordinal, 99)

    def test_inactive_connector_requires_no_binding(self):
        interface = self.interface()
        interface["parameters"].append(
            {
                "id": "enable_hold",
                "type": {"kind": "primitive", "primitive": "boolean"},
                "shape": {"kind": "scalar"},
                "configurability": "fixed",
                "default": False,
            }
        )
        interface["connectors"].append(
            {
                "id": "hold",
                "direction": "input",
                "type": {"kind": "primitive", "primitive": "boolean"},
                "shape": {"kind": "scalar"},
                "presence": {
                    "kind": "when",
                    "guard": {
                        "op": "eq",
                        "left": {
                            "kind": "parameter",
                            "parameter": "enable_hold",
                        },
                        "right": {
                            "kind": "literal",
                            "type": {
                                "kind": "primitive",
                                "primitive": "boolean",
                            },
                            "value": True,
                        },
                    },
                },
            }
        )
        named = self.named_through_stages(interface=interface)
        self.assertNotIn("hold", [row.connector_id for row in named.connectors])
        arguments = self.valid_arguments(named)
        arguments["member_bindings"] = (
            self.binding("parameter", "sample_period_s", "samplePeriod"),
            self.binding("parameter", "enable_hold", "have_hol"),
            self.binding("connector", "requests", "numOfReq"),
        )
        result = self.project(arguments)
        self.assertNotIn("hold", [row.connector_id for row in result.connectors])

    def test_unknown_forward_and_reverse_lookups_fail_cleanly(self):
        result = self.project(self.valid_arguments())
        with self.assertRaisesRegex(KeyError, "unknown scalar name"):
            result.claim_for_scalar("c_absent")
        for key in (
            ("parameter", TRIM_CLASS, "absent"),
            ("connector", TIME_CLASS, "numOfReq"),
            ("signal", TRIM_CLASS, "numOfReq"),
        ):
            with self.subTest(key=key), self.assertRaises(KeyError) as caught:
                result.scalar_names_for_source(*key)
            self.assertEqual(caught.exception.args[0], key)

    def test_named_projection_forgery_checks_are_atomic(self):
        valid = self.direct_named()
        parameter = valid.parameters[0]
        coordinate = self.coordinate()
        cases = {
            "wrong projection type": (cast(Any, object()), "invalid_named_projection"),
            "empty canonical id": (
                replace(valid, canonical_id=""),
                "invalid_named_metadata",
            ),
            "Boolean revision": (
                replace(valid, revision=True),
                "invalid_named_metadata",
            ),
            "mutable rows": (
                replace(valid, parameters=list(valid.parameters)),
                "invalid_named_container",
            ),
            "wrong row type": (
                replace(valid, parameters=(object(),)),
                "invalid_named_row",
            ),
            "empty owner": (
                replace(valid, parameters=(replace(parameter, parameter_id=""),)),
                "invalid_owner_id",
            ),
            "wrong namespace": (
                replace(valid, parameters=(replace(parameter, scalar_name="c_wrong"),)),
                "scalar_name_namespace",
            ),
            "duplicate scalar": (
                replace(valid, parameters=(parameter, parameter)),
                "duplicate_scalar_name",
            ),
            "coordinate container": (
                replace(valid, parameters=(replace(parameter, coordinates=[]),)),
                "invalid_coordinates",
            ),
            "coordinate row": (
                replace(valid, parameters=(replace(parameter, coordinates=(object(),)),)),
                "invalid_coordinate",
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
            "non UTF-8 scalar": (
                replace(
                    valid,
                    parameters=(replace(parameter, scalar_name="p_bad\ud800"),),
                ),
                "utf8_encoding",
            ),
        }
        for name, (named, code) in cases.items():
            with self.subTest(name=name):
                arguments = self.valid_arguments(named)
                self.assert_projection_error(arguments, code)

    def test_input_record_and_container_types_are_exact(self):
        arguments = self.valid_arguments()
        invalid_binding_class = replace(
            arguments["member_bindings"][0],
            canonical_class_path="Buildings.Controls.OBC.ASHRAE.G36..Bad",
        )
        cases = (
            ("inventory", "source_inventory", [], "invalid_inventory"),
            (
                "pin record",
                "source_pins",
                (object(), arguments["source_pins"][1]),
                "invalid_source_pin",
            ),
            (
                "class container",
                "class_claims",
                list(arguments["class_claims"]),
                "invalid_class_claims",
            ),
            (
                "class record",
                "class_claims",
                (object(),),
                "invalid_class_claim",
            ),
            (
                "locator record",
                "class_claims",
                (replace(arguments["class_claims"][0], file=object()),),
                "invalid_file_locator",
            ),
            (
                "binding container",
                "member_bindings",
                list(arguments["member_bindings"]),
                "invalid_member_bindings",
            ),
            (
                "binding record",
                "member_bindings",
                (object(),),
                "invalid_member_binding",
            ),
            (
                "binding class",
                "member_bindings",
                (invalid_binding_class, arguments["member_bindings"][1]),
                "invalid_binding_class_path",
            ),
        )
        for name, key, value, code in cases:
            with self.subTest(name=name):
                case = self.valid_arguments()
                case[key] = value
                self.assert_projection_error(case, code)

    def test_output_is_frozen_detached_recursive_and_projection_has_no_io(self):
        arguments = self.valid_arguments()
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
            result = self.project(arguments)

        with self.assertRaises(FrozenInstanceError):
            setattr(result, "revision", 2)
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0], "source_member", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(result.connectors[0].coordinates[0], "member_id", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0].file, "path", "changed")

        named = arguments["named_projection"]
        claim = arguments["class_claims"][0]
        binding = arguments["member_bindings"][0]
        trim_file = next(
            row
            for row in arguments["source_inventory"]["snapshots"][0]["files"]
            if row["path"] == TRIM_PATH
        )
        object.__setattr__(named, "canonical_id", "changed")
        object.__setattr__(named.parameters[0], "scalar_name", "p_changed")
        object.__setattr__(named.connectors[0].coordinates[0], "member_id", "changed")
        trim_file["git_blob_sha1"] = "sha1:" + "0" * 40
        object.__setattr__(arguments["source_pins"][0], "revision", "0" * 40)
        object.__setattr__(claim, "canonical_class_path", TIME_CLASS)
        object.__setattr__(claim.file, "path", TIME_PATH)
        object.__setattr__(binding, "source_member", "changed")

        self.assertEqual(result.canonical_id, "G36-05-16-SOURCE-CLAIM-TEST")
        self.assertEqual(result.parameters[0].source_member, "samplePeriod")
        self.assertEqual(result.parameters[0].canonical_class_path, TRIM_CLASS)
        self.assertEqual(result.parameters[0].file.path, TRIM_PATH)
        self.assertEqual(result.parameters[0].file.git_blob_sha1, TRIM_BLOB)
        self.assertEqual(result.connectors[0].coordinates[0].member_id, "first")

        def assert_no_mutable_values(value):
            self.assertNotIsInstance(value, (dict, list, set))
            if is_dataclass(value):
                for field in fields(value):
                    assert_no_mutable_values(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    assert_no_mutable_values(item)

        assert_no_mutable_values(result)

    def test_output_shape_excludes_deferred_contracts_and_names_stay_unchanged(self):
        result = self.project(self.valid_arguments())
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
        self.assertEqual(
            tuple(
                field.name
                for field in fields(routine_scalar_names.NamedScalarProjection)
            ),
            ("canonical_id", "revision", "parameters", "connectors"),
        )
        self.assertTrue(
            {
                field.name
                for field in fields(routine_scalar_names.NamedScalarProjection)
            }.isdisjoint({"source_claims", "source_map", "provenance"})
        )

        names = set()

        def collect_names(value):
            if is_dataclass(value):
                for field in fields(value):
                    names.add(field.name)
                    collect_names(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    collect_names(item)

        collect_names(result)
        forbidden = {
            "graph_node",
            "source_span",
            "line",
            "column",
            "engine_path",
            "connector_iri",
            "iri",
            "cxf_id",
            "deployment_id",
            "semantic_binding",
            "point_binding",
            "runtime",
            "persistence",
            "content_hash",
            "source_index",
            "dependency_closure",
            "production_status",
        }
        self.assertTrue(names.isdisjoint(forbidden))
        self.assertIn(
            "does not verify declarations",
            routine_scalar_source_claims.__doc__ or "",
        )
        self.assertIn(
            "public or persisted source map",
            " ".join(
                (
                    routine_scalar_source_claims.ScalarSourceClaimProjection.__doc__
                    or ""
                ).split()
            ),
        )


if __name__ == "__main__":
    unittest.main()
