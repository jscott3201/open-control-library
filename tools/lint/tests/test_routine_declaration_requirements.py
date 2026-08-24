import ast
import copy
import inspect
import json
import os
import unittest
from dataclasses import FrozenInstanceError, fields, is_dataclass, replace
from pathlib import Path
from unittest import mock

from tools.lint import (
    routine_declaration_requirements,
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


class RoutineDeclarationRequirementTests(unittest.TestCase):
    def setUp(self):
        self.inventory = json.loads(INVENTORY_PATH.read_text(encoding="utf-8"))
        self.release_revision = RELEASE_PIN_PATH.read_text(encoding="utf-8").strip()
        self.development_revision = DEVELOPMENT_PIN_PATH.read_text(
            encoding="utf-8"
        ).strip()

    @staticmethod
    def interface():
        return {
            "schema": "cxf-library/routine-interface/v3",
            "canonical_id": "G36-05-16-DECLARATION-REQUIREMENT-TEST",
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
            "canonical_id": "G36-05-16-DECLARATION-REQUIREMENT-TEST",
            "revision": 1,
            "parameters": [],
            "members": [],
        }

    @staticmethod
    def coordinate(dimension_id="zones", member_id="north-zone", ordinal=0):
        return routine_scalar_abi.ScalarCoordinate(
            dimension_id, member_id, ordinal
        )

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
    def locator(path=TRIM_PATH, blob=TRIM_BLOB):
        return routine_scalar_source_claims.SourceFileLocator(path, blob)

    def parameter(
        self,
        parameter_id="setting",
        *,
        coordinates=(),
        scalar_name=None,
        class_path=TRIM_CLASS,
        source_member=None,
        snapshot="release",
        revision=None,
        locator=None,
    ):
        return routine_scalar_source_claims.ScalarParameterSourceClaim(
            scalar_name
            or self.scalar_name("p_", parameter_id, coordinates),
            parameter_id,
            coordinates,
            class_path,
            source_member or parameter_id,
            snapshot,
            revision or self.release_revision,
            locator or self.locator(),
        )

    def connector(
        self,
        connector_id="signal",
        *,
        coordinates=(),
        scalar_name=None,
        class_path=TRIM_CLASS,
        source_member=None,
        snapshot="release",
        revision=None,
        locator=None,
    ):
        return routine_scalar_source_claims.ScalarConnectorSourceClaim(
            scalar_name
            or self.scalar_name("c_", connector_id, coordinates),
            connector_id,
            coordinates,
            class_path,
            source_member or connector_id,
            snapshot,
            revision or self.release_revision,
            locator or self.locator(),
        )

    def direct_projection(self, parameters=None, connectors=None):
        return routine_scalar_source_claims.ScalarSourceClaimProjection(
            "G36-05-16-DIRECT-DECLARATION-REQUIREMENT-TEST",
            1,
            (self.parameter(),) if parameters is None else parameters,
            (self.connector(),) if connectors is None else connectors,
        )

    def through_existing_stages(self):
        resolved = routine_resolution.resolve_specialization(
            self.interface(), self.specialization()
        )
        abi = routine_scalar_abi.project_scalar_abi(resolved)
        named = routine_scalar_names.allocate_scalar_names(abi)
        source_claims = routine_scalar_source_claims.project_scalar_source_claims(
            named,
            source_inventory=copy.deepcopy(self.inventory),
            source_pins=(
                routine_scalar_source_claims.SourcePin(
                    "release", self.release_revision
                ),
                routine_scalar_source_claims.SourcePin(
                    "development", self.development_revision
                ),
            ),
            class_claims=(
                routine_scalar_source_claims.SourceClassClaim(
                    TRIM_CLASS,
                    "release",
                    self.release_revision,
                    self.locator(),
                ),
            ),
            member_bindings=(
                routine_scalar_source_claims.SourceMemberBinding(
                    "parameter", "sample_period_s", TRIM_CLASS, "samplePeriod"
                ),
                routine_scalar_source_claims.SourceMemberBinding(
                    "connector", "requests", TRIM_CLASS, "numOfReq"
                ),
            ),
        )
        return named, source_claims

    @staticmethod
    def project(source_claim_projection):
        return routine_declaration_requirements.project_declaration_requirements(
            source_claim_projection
        )

    def assert_projection_error(self, source_claim_projection, expected_code):
        attempts = []
        for _ in range(2):
            with mock.patch.object(
                routine_declaration_requirements,
                "ParameterDeclarationRequirement",
                side_effect=AssertionError("parameter requirement allocated"),
            ), mock.patch.object(
                routine_declaration_requirements,
                "ConnectorDeclarationRequirement",
                side_effect=AssertionError("connector requirement allocated"),
            ), mock.patch.object(
                routine_declaration_requirements,
                "DeclarationRequirementProjection",
                side_effect=AssertionError("requirement projection allocated"),
            ):
                with self.assertRaises(
                    routine_declaration_requirements.DeclarationRequirementError
                ) as caught:
                    self.project(source_claim_projection)
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

    def test_release_trim_and_respond_claims_become_owner_requirements(self):
        named, source_claims = self.through_existing_stages()
        result = self.project(source_claims)

        self.assertEqual(result.canonical_id, named.canonical_id)
        self.assertEqual(result.revision, named.revision)
        self.assertEqual(len(result.parameters), 1)
        self.assertEqual(len(result.connectors), 1)

        parameter = result.parameters[0]
        self.assertEqual(parameter.parameter_id, "sample_period_s")
        self.assertEqual(parameter.source_member, "samplePeriod")
        self.assertEqual(
            parameter.scalar_names,
            tuple(row.scalar_name for row in named.parameters),
        )

        connector = result.connectors[0]
        self.assertEqual(connector.connector_id, "requests")
        self.assertEqual(connector.source_member, "numOfReq")
        self.assertEqual(
            connector.scalar_names,
            tuple(row.scalar_name for row in named.connectors),
        )
        self.assertEqual(len(connector.scalar_names), 2)
        for requirement in (parameter, connector):
            self.assertEqual(requirement.canonical_class_path, TRIM_CLASS)
            self.assertEqual(requirement.snapshot, "release")
            self.assertEqual(requirement.revision, self.release_revision)
            self.assertEqual(requirement.file.path, TRIM_PATH)
            self.assertEqual(requirement.file.git_blob_sha1, TRIM_BLOB)

    def test_vector_matrix_dedup_preserves_first_owner_and_scalar_order(self):
        north = self.coordinate("zones", "north-zone", 0)
        south = self.coordinate("zones", "south-zone", 1)
        primary = self.coordinate("positions", "primary", 0)
        secondary = self.coordinate("positions", "secondary", 1)
        matrix_rows = (
            self.parameter(
                "matrix_weights", coordinates=(north, primary)
            ),
            self.parameter("gain"),
            self.parameter(
                "matrix_weights", coordinates=(north, secondary)
            ),
            self.parameter(
                "matrix_weights", coordinates=(south, primary)
            ),
        )
        connector_rows = (
            self.connector(
                "beta", coordinates=(north,), source_member="betaValue"
            ),
            self.connector(
                "alpha", coordinates=(north,), source_member="alphaValue"
            ),
            self.connector(
                "beta", coordinates=(south,), source_member="betaValue"
            ),
            self.connector(
                "alpha", coordinates=(south,), source_member="alphaValue"
            ),
        )
        result = self.project(
            self.direct_projection(matrix_rows, connector_rows)
        )

        self.assertEqual(
            [item.parameter_id for item in result.parameters],
            ["matrix_weights", "gain"],
        )
        self.assertEqual(
            result.parameters[0].scalar_names,
            tuple(
                row.scalar_name
                for row in matrix_rows
                if row.parameter_id == "matrix_weights"
            ),
        )
        self.assertEqual(
            [item.connector_id for item in result.connectors],
            ["beta", "alpha"],
        )
        self.assertEqual(
            result.connectors[0].scalar_names,
            (connector_rows[0].scalar_name, connector_rows[2].scalar_name),
        )
        for requirement in (*result.parameters, *result.connectors):
            names = {item.name for item in fields(requirement)}
            self.assertNotIn("coordinates", names)
            self.assertNotIn("source_index", names)
            self.assertNotIn("indices", names)

    def test_same_owner_id_is_independent_across_namespaces(self):
        projection = self.direct_projection(
            parameters=(
                self.parameter("setting", source_member="sharedMember"),
            ),
            connectors=(
                self.connector("setting", source_member="sharedMember"),
            ),
        )
        result = self.project(projection)

        self.assertEqual(result.parameters[0].parameter_id, "setting")
        self.assertEqual(result.connectors[0].connector_id, "setting")
        self.assertNotEqual(
            result.parameters[0].scalar_names,
            result.connectors[0].scalar_names,
        )

    def test_conflicting_owner_source_identities_fail_instead_of_merging(self):
        first = self.connector(
            "requests", coordinates=(self.coordinate(member_id="north-zone"),)
        )
        second = self.connector(
            "requests",
            coordinates=(self.coordinate(member_id="south-zone", ordinal=1),),
        )
        cases = {
            "snapshot": replace(second, snapshot="development"),
            "revision": replace(second, revision="0" * 40),
            "path": replace(
                second,
                file=self.locator(TIME_PATH, "sha1:" + "1" * 40),
            ),
            "blob": replace(
                second, file=self.locator(TRIM_PATH, "sha1:" + "2" * 40)
            ),
            "class": replace(second, canonical_class_path=TIME_CLASS),
            "member": replace(second, source_member="otherMember"),
        }
        for label, changed in cases.items():
            with self.subTest(label=label):
                self.assert_projection_error(
                    self.direct_projection((), (first, changed)),
                    "inconsistent_owner_source",
                )

    def test_malformed_projection_rows_metadata_and_coordinates_fail_closed(self):
        valid = self.direct_projection()
        parameter = valid.parameters[0]
        coordinate = self.coordinate()

        class ProjectionSubclass(
            routine_scalar_source_claims.ScalarSourceClaimProjection
        ):
            pass

        class ParameterSubclass(
            routine_scalar_source_claims.ScalarParameterSourceClaim
        ):
            pass

        cases = (
            ("wrong projection", object(), "invalid_source_claim_projection"),
            (
                "projection subclass",
                ProjectionSubclass(
                    valid.canonical_id,
                    valid.revision,
                    valid.parameters,
                    valid.connectors,
                ),
                "invalid_source_claim_projection",
            ),
            (
                "canonical id",
                replace(valid, canonical_id="G36-SCOPE-05-16"),
                "invalid_metadata",
            ),
            (
                "bounded canonical id",
                replace(valid, canonical_id="G36-05-16-" + "A" * 129),
                "invalid_metadata",
            ),
            (
                "Boolean revision",
                replace(valid, revision=True),
                "invalid_metadata",
            ),
            (
                "parameter container",
                replace(valid, parameters=list(valid.parameters)),
                "invalid_source_claim_container",
            ),
            (
                "wrong row kind",
                replace(valid, parameters=(valid.connectors[0],)),
                "invalid_source_claim_row",
            ),
            (
                "row subclass",
                replace(
                    valid,
                    parameters=(
                        ParameterSubclass(
                            parameter.scalar_name,
                            parameter.parameter_id,
                            parameter.coordinates,
                            parameter.canonical_class_path,
                            parameter.source_member,
                            parameter.snapshot,
                            parameter.revision,
                            parameter.file,
                        ),
                    ),
                ),
                "invalid_source_claim_row",
            ),
            (
                "empty owner",
                replace(valid, parameters=(replace(parameter, parameter_id=""),)),
                "invalid_owner_id",
            ),
            (
                "owner syntax",
                replace(valid, parameters=(replace(parameter, parameter_id="Bad-ID"),)),
                "invalid_owner_id",
            ),
            (
                "bounded owner",
                replace(valid, parameters=(replace(parameter, parameter_id="a" * 65),)),
                "invalid_owner_id",
            ),
            (
                "scalar syntax",
                replace(valid, parameters=(replace(parameter, scalar_name="p_not_hex"),)),
                "invalid_scalar_name",
            ),
            (
                "bounded scalar",
                replace(valid, parameters=(replace(parameter, scalar_name="p_" + "a" * 645),)),
                "invalid_scalar_name",
            ),
            (
                "scalar mismatch",
                replace(valid, parameters=(replace(parameter, scalar_name="p_61"),)),
                "scalar_name_mismatch",
            ),
            (
                "coordinate container",
                replace(valid, parameters=(replace(parameter, coordinates=[]),)),
                "invalid_coordinates",
            ),
            (
                "coordinate row",
                replace(valid, parameters=(replace(parameter, coordinates=(object(),)),)),
                "invalid_coordinate",
            ),
            (
                "coordinate rank",
                replace(
                    valid,
                    parameters=(
                        replace(
                            parameter,
                            coordinates=(coordinate, coordinate, coordinate),
                        ),
                    ),
                ),
                "invalid_coordinates",
            ),
            (
                "dimension id",
                replace(
                    valid,
                    parameters=(
                        replace(
                            parameter,
                            coordinates=(replace(coordinate, dimension_id="Bad"),),
                        ),
                    ),
                ),
                "invalid_dimension_id",
            ),
            (
                "member id",
                replace(
                    valid,
                    parameters=(
                        replace(
                            parameter,
                            coordinates=(replace(coordinate, member_id="north_zone"),),
                        ),
                    ),
                ),
                "invalid_member_id",
            ),
            (
                "negative ordinal",
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
            (
                "Boolean ordinal",
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
        )
        for label, projection, code in cases:
            with self.subTest(label=label):
                self.assert_projection_error(projection, code)

    def test_malformed_source_identity_fields_fail_closed(self):
        valid = self.direct_projection()
        parameter = valid.parameters[0]

        class LocatorSubclass(routine_scalar_source_claims.SourceFileLocator):
            pass

        cases = (
            (
                "class",
                replace(parameter, canonical_class_path="Bad.Class"),
                "invalid_source_class",
            ),
            (
                "member",
                replace(parameter, source_member="bad.member"),
                "invalid_source_member",
            ),
            (
                "snapshot",
                replace(parameter, snapshot="candidate"),
                "invalid_source_snapshot",
            ),
            (
                "revision",
                replace(parameter, revision=self.release_revision.upper()),
                "invalid_source_revision",
            ),
            (
                "locator",
                replace(parameter, file=object()),
                "invalid_file_locator",
            ),
            (
                "locator subclass",
                replace(parameter, file=LocatorSubclass(TRIM_PATH, TRIM_BLOB)),
                "invalid_file_locator",
            ),
            (
                "unsafe path",
                replace(
                    parameter,
                    file=self.locator(
                        "Buildings/Controls/OBC/ASHRAE/G36/../escape.mo"
                    ),
                ),
                "invalid_source_path",
            ),
            (
                "non Modelica path",
                replace(
                    parameter,
                    file=self.locator(
                        "Buildings/Controls/OBC/ASHRAE/G36/Generic/package.order"
                    ),
                ),
                "invalid_source_path",
            ),
            (
                "blob",
                replace(parameter, file=self.locator(TRIM_PATH, "sha1:ABC")),
                "invalid_source_blob",
            ),
        )
        for label, changed, code in cases:
            with self.subTest(label=label):
                self.assert_projection_error(
                    self.direct_projection((changed,), ()), code
                )

    def test_duplicate_scalar_names_source_keys_and_owner_mappings_fail(self):
        parameter = self.parameter()
        self.assert_projection_error(
            self.direct_projection((parameter, parameter), ()),
            "duplicate_scalar_name",
        )

        first = self.parameter("first", source_member="sharedMember")
        second = self.parameter("second", source_member="sharedMember")
        self.assert_projection_error(
            self.direct_projection((first, second), ()),
            "duplicate_source_key",
        )

        connector = self.connector(
            "setting", scalar_name=parameter.scalar_name
        )
        diagnostics = self.assert_projection_error(
            self.direct_projection((parameter,), (connector,)),
            "cross_kind_collision",
        )
        self.assertIn(
            "scalar_name_namespace", {item.code for item in diagnostics}
        )

    def test_complete_diagnostics_are_sorted_deterministic_and_atomic(self):
        valid = self.direct_projection()
        hostile = replace(
            valid,
            canonical_id="bad",
            revision=False,
            parameters=(
                replace(
                    valid.parameters[0],
                    parameter_id="Bad-ID",
                    scalar_name="x_bad",
                    coordinates=[],
                    canonical_class_path="Bad.Class",
                    source_member="bad.member",
                    snapshot="candidate",
                    revision="ABC",
                    file=object(),
                ),
            ),
            connectors=list(valid.connectors),
        )
        diagnostics = self.assert_projection_error(hostile, "invalid_metadata")
        codes = {item.code for item in diagnostics}
        self.assertTrue(
            {
                "invalid_owner_id",
                "invalid_scalar_name",
                "invalid_coordinates",
                "invalid_source_class",
                "invalid_source_member",
                "invalid_source_snapshot",
                "invalid_source_revision",
                "invalid_file_locator",
                "invalid_source_claim_container",
            }.issubset(codes)
        )

    def test_output_is_frozen_detached_and_projection_is_pure(self):
        north = self.coordinate()
        south = self.coordinate(member_id="south-zone", ordinal=1)
        source_claims = self.direct_projection(
            parameters=(
                self.parameter("weights", coordinates=(north,)),
                self.parameter("weights", coordinates=(south,)),
            ),
            connectors=(),
        )
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
            "random.random", side_effect=AssertionError("random access")
        ):
            result = self.project(source_claims)

        with self.assertRaises(FrozenInstanceError):
            setattr(result, "revision", 2)
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0], "parameter_id", "changed")
        with self.assertRaises(FrozenInstanceError):
            setattr(result.parameters[0].file, "path", "changed")

        first = source_claims.parameters[0]
        object.__setattr__(source_claims, "canonical_id", "changed")
        object.__setattr__(first, "parameter_id", "changed")
        object.__setattr__(first, "source_member", "changed")
        object.__setattr__(first, "scalar_name", "p_changed")
        object.__setattr__(first.file, "path", TIME_PATH)
        object.__setattr__(first.file, "git_blob_sha1", "sha1:" + "0" * 40)

        requirement = result.parameters[0]
        self.assertEqual(
            result.canonical_id,
            "G36-05-16-DIRECT-DECLARATION-REQUIREMENT-TEST",
        )
        self.assertEqual(requirement.parameter_id, "weights")
        self.assertEqual(requirement.source_member, "weights")
        self.assertEqual(requirement.file.path, TRIM_PATH)
        self.assertEqual(requirement.file.git_blob_sha1, TRIM_BLOB)
        self.assertEqual(len(requirement.scalar_names), 2)

        def assert_no_mutable_values(value):
            self.assertNotIsInstance(value, (dict, list, set))
            if is_dataclass(value):
                for field in fields(value):
                    assert_no_mutable_values(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    assert_no_mutable_values(item)

        assert_no_mutable_values(result)

    def test_api_and_documentation_keep_the_future_evidence_boundary(self):
        result = self.project(self.direct_projection())
        self.assertEqual(
            tuple(
                item.name
                for item in fields(
                    routine_declaration_requirements.DeclarationRequirementProjection
                )
            ),
            ("canonical_id", "revision", "parameters", "connectors"),
        )
        expected_requirement_fields = (
            "parameter_id",
            "canonical_class_path",
            "source_member",
            "snapshot",
            "revision",
            "file",
            "scalar_names",
        )
        self.assertEqual(
            tuple(
                item.name
                for item in fields(
                    routine_declaration_requirements.ParameterDeclarationRequirement
                )
            ),
            expected_requirement_fields,
        )
        self.assertEqual(
            tuple(
                item.name
                for item in fields(
                    routine_declaration_requirements.ConnectorDeclarationRequirement
                )
            ),
            ("connector_id", *expected_requirement_fields[1:]),
        )

        field_names = set()

        def collect_field_names(value):
            if is_dataclass(value):
                for field in fields(value):
                    field_names.add(field.name)
                    collect_field_names(getattr(value, field.name))
            elif isinstance(value, tuple):
                for item in value:
                    collect_field_names(item)

        collect_field_names(result)
        self.assertTrue(
            field_names.isdisjoint(
                {
                    "declaration_verified",
                    "evidence",
                    "source_index",
                    "serialized",
                    "json",
                    "protocol",
                    "parser",
                    "rust",
                    "dependency_closure",
                    "enum_locator",
                }
            )
        )
        module_doc = " ".join(
            (routine_declaration_requirements.__doc__ or "").split()
        )
        projection_doc = " ".join(
            (
                routine_declaration_requirements.DeclarationRequirementProjection.__doc__
                or ""
            ).split()
        )
        self.assertIn("future declaration checks", module_doc)
        self.assertIn("does not parse source", module_doc)
        self.assertIn("do not assert declaration evidence", projection_doc)

        tree = ast.parse(inspect.getsource(routine_declaration_requirements))
        imported_modules = set()
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                imported_modules.update(alias.name for alias in node.names)
            elif isinstance(node, ast.ImportFrom) and node.module is not None:
                imported_modules.add(node.module)
        forbidden_roots = {
            "asyncio",
            "http",
            "os",
            "pathlib",
            "requests",
            "socket",
            "subprocess",
            "urllib",
        }
        self.assertTrue(
            all(
                module.split(".")[0] not in forbidden_roots
                for module in imported_modules
            )
        )
        self.assertTrue(
            all(
                "parser" not in module.lower() and "rust" not in module.lower()
                for module in imported_modules
            )
        )


if __name__ == "__main__":
    unittest.main()
