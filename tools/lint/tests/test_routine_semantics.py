import contextlib
import copy
import io
import json
import re
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from rdflib import Graph, Literal, Namespace, URIRef
from rdflib.namespace import RDF, XSD

from tools.lint import routine_semantics


PRODUCT_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_FILES = tuple(
    f"routines/schemas/{name}" for name in routine_semantics.routine_schemas.SCHEMA_FILES
)
ONTOLOGY_FILES = tuple(
    f"routines/ontology/{name}" for name in sorted(routine_semantics.ONTOLOGY_FILES)
)
SHACL_FILES = tuple(
    f"routines/ontology/shacl/{name}"
    for name in sorted(routine_semantics.SHACL_FILES)
)
FIXTURE_FILES = tuple(
    f"tools/lint/tests/fixtures/routine_semantics/{name}"
    for name in routine_semantics.FIXTURE_SCHEMAS
)
PROFILE_PATH, MANIFEST_PATH = FIXTURE_FILES
OCL = Namespace(routine_semantics.LOCAL_NAMESPACE)
QUDT_UNIT = Namespace("http://qudt.org/vocab/unit/")


class RoutineSemanticTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        for relative_path in SCHEMA_FILES + ONTOLOGY_FILES + SHACL_FILES + FIXTURE_FILES:
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

    def parse_jsonld(self, relative_path):
        document = self.read_json(relative_path)
        errors = []
        safe = routine_semantics._check_jsonld_safety(
            document, relative_path, errors
        )
        graph = routine_semantics._parse_jsonld(
            document, Path(relative_path), safe, errors
        )
        self.assertEqual(errors, [])
        if graph is None:
            self.fail(f"{relative_path} did not produce an RDF graph")
        return document, graph

    def parse_shapes(self):
        errors = []
        vocabulary = routine_semantics._parse_vocabulary(
            (self.root / routine_semantics.VOCABULARY_PATH).read_bytes(), errors
        )
        shapes = routine_semantics._parse_shapes(
            (self.root / routine_semantics.SHAPE_PATH).read_bytes(),
            vocabulary,
            errors,
        )
        self.assertEqual(errors, [])
        if shapes is None:
            self.fail("governed SHACL graph did not parse")
        return shapes

    def validate_graph(self, graph, fixture_path):
        errors = []
        routine_semantics._validate_shacl_graph(
            graph, self.parse_shapes(), fixture_path, errors
        )
        self.assertEqual(errors, sorted(errors))
        self.assertFalse(any("Traceback" in error for error in errors))
        return errors

    def assert_error(self, expected):
        errors = routine_semantics.validate(self.root)
        self.assertTrue(
            any(expected in error for error in errors),
            f"{expected!r} not found in {errors!r}",
        )
        self.assertEqual(errors, sorted(errors))
        self.assertFalse(any("Traceback" in error for error in errors))
        return errors

    def test_production_contracts_are_clean_repeatable_and_network_free(self):
        fixture_bytes = {
            path: (PRODUCT_ROOT / path).read_bytes() for path in FIXTURE_FILES
        }
        with mock.patch(
            "rdflib.plugins.shared.jsonld.context.source_to_json",
            side_effect=AssertionError("JSON-LD context loading attempted"),
        ), mock.patch(
            "rdflib.parser._urlopen",
            side_effect=AssertionError("URL opening attempted"),
        ), mock.patch(
            "urllib.request.urlopen",
            side_effect=AssertionError("URL opening attempted"),
        ):
            self.assertEqual(routine_semantics.validate(PRODUCT_ROOT), [])
            first = routine_semantics.validate(self.root)
            second = routine_semantics.validate(self.root)
        self.assertEqual(first, [])
        self.assertEqual(second, first)
        self.assertEqual(
            fixture_bytes,
            {path: (PRODUCT_ROOT / path).read_bytes() for path in FIXTURE_FILES},
        )

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_semantics.main(self.root)
        self.assertEqual(result, 0)
        self.assertEqual(
            output.getvalue(),
            "routine semantic lint: ontology pins, local vocabulary, 1 SHACL graph, 2 schemas, 2 synthetic fixtures OK\n",
        )

    def test_json_loader_rejects_malformed_duplicate_nonfinite_and_non_utf8(self):
        path = self.root / MANIFEST_PATH
        original = path.read_bytes()
        cases = (
            (b"{", "invalid JSON at line 1, column 2"),
            (b'{"schema":"one","schema":"two"}\n', "duplicate object key 'schema'"),
            (
                original.replace(b'"freshness_limit_s": 300', b'"freshness_limit_s": NaN'),
                "non-finite number 'NaN' is forbidden",
            ),
            (
                original.replace(
                    b'"time_alignment_window_s": 60',
                    b'"time_alignment_window_s": Infinity',
                ),
                "non-finite number 'Infinity' is forbidden",
            ),
            (b"\xff", "file is not UTF-8"),
        )
        for content, expected in cases:
            with self.subTest(expected=expected):
                path.write_bytes(content)
                self.assert_error(expected)
                path.write_bytes(original)

    def test_governed_schema_pin_vocabulary_and_fixture_files_fail_closed(self):
        missing_paths = (
            "routines/schemas/routine-semantic-profile.schema.json",
            "routines/ontology/ontology-pins.json",
            "routines/ontology/ocl-vocabulary.ttl",
            "routines/ontology/shacl/open-control-routine-shapes.ttl",
            PROFILE_PATH,
        )
        for relative_path in missing_paths:
            with self.subTest(relative_path=relative_path):
                path = self.root / relative_path
                original = path.read_bytes()
                path.unlink()
                self.assert_error("governed" if "schemas/" in relative_path else "missing")
                path.write_bytes(original)

        extras = (
            "routines/schemas/extra.schema.json",
            "routines/ontology/extra.ttl",
            "routines/ontology/shacl/extra.ttl",
            "tools/lint/tests/fixtures/routine_semantics/extra.jsonld",
        )
        for relative_path in extras:
            with self.subTest(relative_path=relative_path):
                path = self.root / relative_path
                path.write_text("{}\n", encoding="utf-8")
                self.assert_error("unexpected")
                path.unlink()

    def test_schema_refs_are_local_and_resolvable(self):
        schema_path = "routines/schemas/routine-semantic-profile.schema.json"
        original = (self.root / schema_path).read_bytes()

        self.mutate(
            schema_path,
            lambda value: value["$defs"].update(
                bad={"$ref": "https://example.com/remote.schema.json"}
            ),
        )
        self.assert_error("references forbidden resource 'https://example.com/remote.schema.json'")
        (self.root / schema_path).write_bytes(original)

        self.mutate(
            schema_path,
            lambda value: value["$defs"].update(bad={"$ref": "#/$defs/missing"}),
        )
        self.assert_error("cannot resolve '#/$defs/missing'")

    def test_contexts_and_imports_are_rejected_before_jsonld_parsing(self):
        original = (self.root / PROFILE_PATH).read_bytes()
        cases = (
            (
                lambda value: value.update({"@context": "https://example.com/context"}),
                "context must be one embedded local object",
            ),
            (
                lambda value: value.update({"@context": [value["@context"]]}),
                "context must be one embedded local object",
            ),
            (
                lambda value: value["connector_roles"][0]["binding"].update(
                    {"@context": "https://example.com/nested"}
                ),
                "nested JSON-LD context is forbidden",
            ),
            (
                lambda value: value["@context"].update(
                    {"@import": "https://example.com/import"}
                ),
                "JSON-LD @import is forbidden",
            ),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                self.mutate(PROFILE_PATH, mutation)
                with mock.patch(
                    "rdflib.plugins.shared.jsonld.context.source_to_json",
                    side_effect=AssertionError("network access attempted"),
                ):
                    self.assert_error(expected)
                (self.root / PROFILE_PATH).write_bytes(original)

    def test_context_coverage_rejects_an_unmapped_schema_property(self):
        schema_path = "routines/schemas/routine-semantic-profile.schema.json"
        self.mutate(
            schema_path,
            lambda value: value["$defs"]["semanticContext"]["const"].pop("id"),
        )
        for fixture_path in (PROFILE_PATH, MANIFEST_PATH):
            self.mutate(fixture_path, lambda value: value["@context"].pop("id"))
        self.assert_error("semanticContext has no JSON-LD mapping for schema property 'id'")

    def test_derivation_jsonld_preserves_ids_and_policy_values(self):
        manifest, graph = self.parse_jsonld(MANIFEST_PATH)

        expected_ids = {
            "zone_air_temperature_max",
            "north_zone_temperature",
            "south_zone_temperature",
            "service_zone_temperature",
            "north-zone",
            "south-zone",
            "service-zone",
            "exclude_service_zone",
        }
        self.assertEqual(
            {str(value) for value in graph.objects(None, OCL.localId)}, expected_ids
        )

        manifest_id = URIRef(manifest["@id"])
        data_quality = graph.value(manifest_id, OCL.dataQualityPolicy)
        self.assertIsNotNone(data_quality)
        self.assertIn((data_quality, OCL.acceptedStatus, Literal("good")), graph)
        self.assertIn(
            (data_quality, OCL.rejectedInputPolicy, Literal("exclude-member")),
            graph,
        )
        self.assertIn((data_quality, OCL.minimumValidMembers, Literal(2)), graph)

        ready = graph.value(manifest_id, OCL.readyCondition)
        self.assertIsNotNone(ready)
        self.assertIn((ready, OCL.minimumValidMembers, Literal(2)), graph)
        self.assertIn(
            (ready, OCL.requireAllInputsInDomain, Literal(False)), graph
        )

        unit_policy = graph.value(manifest_id, OCL.unitPolicy)
        self.assertIsNotNone(unit_policy)
        self.assertIn((unit_policy, OCL.outputUnit, QUDT_UNIT.DEG_C), graph)
        self.assertIn(
            (unit_policy, OCL.inputUnitsPolicy, Literal("same-as-output")), graph
        )
        self.assertIn(
            (unit_policy, OCL.conversionPolicy, Literal("none")), graph
        )

    def test_fractional_freshness_and_alignment_are_valid_rdf_numbers(self):
        self.mutate(
            MANIFEST_PATH,
            lambda value: value.update(
                freshness_limit_s=0.5,
                time_alignment_window_s=0.5,
            ),
        )
        self.assertEqual(routine_semantics.validate(self.root), [])

        manifest, graph = self.parse_jsonld(MANIFEST_PATH)
        manifest_id = URIRef(manifest["@id"])
        self.assertEqual(
            graph.value(manifest_id, OCL.freshnessLimitSeconds).datatype,
            XSD.double,
        )
        self.assertEqual(
            graph.value(manifest_id, OCL.timeAlignmentWindowSeconds).datatype,
            XSD.double,
        )

    def test_software_source_signal_id_is_preserved_as_an_iri(self):
        signal_id = "urn:open-control-library:software-signal:north-zone-temperature"
        self.mutate(
            MANIFEST_PATH,
            lambda value: value["inputs"][0].update(
                source={
                    "kind": "software-signal",
                    "signal_id": signal_id,
                    "local_class": "ocl:SoftwareSignal",
                    "value_kind": "real",
                    "qudt_unit": "unit:DEG_C",
                    "member_id": "north-zone",
                }
            ),
        )
        self.assertEqual(routine_semantics.validate(self.root), [])

        manifest, graph = self.parse_jsonld(MANIFEST_PATH)
        input_id = URIRef(manifest["inputs"][0]["@id"])
        source = graph.value(input_id, OCL.source)
        self.assertIsNotNone(source)
        self.assertIn((source, OCL.signalId, URIRef(signal_id)), graph)

    def test_ontology_pin_constants_and_local_vocabulary_hash_are_enforced(self):
        pin_path = "routines/ontology/ontology-pins.json"
        pin_original = (self.root / pin_path).read_bytes()
        vocabulary_path = self.root / "routines/ontology/ocl-vocabulary.ttl"
        vocabulary_original = vocabulary_path.read_bytes()

        self.mutate(
            pin_path,
            lambda value: value["brick"].update(release="latest"),
        )
        self.assert_error("$.brick.release must be 'v1.4.4'")
        (self.root / pin_path).write_bytes(pin_original)

        self.mutate(
            pin_path,
            lambda value: value["local"].update(sha256="0" * 64),
        )
        self.assert_error("$.local.sha256 must be")
        (self.root / pin_path).write_bytes(pin_original)

        vocabulary_path.write_bytes(vocabulary_original + b"\n# changed bytes\n")
        self.assert_error("SHA-256 must be")

    def test_malformed_turtle_is_reported_without_a_traceback(self):
        path = self.root / "routines/ontology/ocl-vocabulary.ttl"
        path.write_text("@prefix ocl: <urn:open-control-library:ontology:> .\nocl: [", encoding="utf-8")
        self.assert_error("Turtle parse failed")

    def test_shape_graph_fails_closed_for_malformed_importing_and_advanced_content(self):
        path = self.root / routine_semantics.SHAPE_PATH
        original = path.read_bytes()
        cases = (
            (b"@prefix shape: <urn:open-control-library:shacl:routine:> .\nshape: [", "Turtle parse failed"),
            (b"\n<urn:test> <http://www.w3.org/2002/07/owl#imports> <https://example.com/shapes> .\n", "owl:imports is forbidden"),
            (b"\n<urn:test> <http://www.w3.org/ns/shacl#sparql> <urn:test:constraint> .\n", "forbidden term sh:sparql"),
            (b"\xff", "file is not UTF-8"),
        )
        for content, expected in cases:
            with self.subTest(expected=expected):
                path.write_bytes(content if content.startswith(b"@prefix") else original + content)
                self.assert_error(expected)
                path.write_bytes(original)

    def test_shape_ocl_terms_must_exist_in_the_governed_vocabulary(self):
        path = self.root / routine_semantics.SHAPE_PATH
        path.write_bytes(
            path.read_bytes()
            + b"\n<urn:open-control-library:shacl:routine:ProfileShape> "
            + b"<urn:open-control-library:ontology:undeclaredPredicate> \"x\" .\n"
        )
        self.assert_error(
            "OCL term ocl:undeclaredPredicate is absent from the governed vocabulary"
        )

    def test_shacl_receives_only_graphs_with_disabled_execution_features(self):
        real_validate = routine_semantics._pyshacl_validate
        self.assertIsNotNone(real_validate)
        calls = []

        def checked_validate(data_graph, *args, **kwargs):
            self.assertIsInstance(data_graph, Graph)
            self.assertIsInstance(kwargs["shacl_graph"], Graph)
            self.assertEqual(args, ())
            self.assertEqual(
                {key: kwargs[key] for key in kwargs if key != "shacl_graph"},
                {
                    "ont_graph": None,
                    "inference": "none",
                    "advanced": False,
                    "js": False,
                    "do_owl_imports": False,
                    "inplace": False,
                    "abort_on_first": False,
                    "allow_infos": False,
                    "allow_warnings": False,
                    "sparql_mode": False,
                },
            )
            data_before = frozenset(data_graph)
            result = real_validate(data_graph, **kwargs)
            self.assertEqual(frozenset(data_graph), data_before)
            calls.append(data_graph)
            return result

        with mock.patch.object(
            routine_semantics, "_pyshacl_validate", side_effect=checked_validate
        ):
            self.assertEqual(routine_semantics.validate(self.root), [])
        self.assertEqual(len(calls), 2)
        self.assertIsNot(calls[0], calls[1])

        _, data_graph = self.parse_jsonld(PROFILE_PATH)
        shapes_graph = self.parse_shapes()
        data_before = frozenset(data_graph)
        shapes_before = frozenset(shapes_graph)
        errors = []
        routine_semantics._validate_shacl_graph(
            data_graph, shapes_graph, PROFILE_PATH, errors
        )
        self.assertEqual(errors, [])
        self.assertEqual(frozenset(data_graph), data_before)
        self.assertEqual(frozenset(shapes_graph), shapes_before)

    def test_profile_rdf_mutations_have_stable_shacl_diagnostics(self):
        fixture_bytes = (self.root / PROFILE_PATH).read_bytes()

        def role(graph, connector_id="zone_air_temperatures"):
            return next(
                subject
                for subject in graph.subjects(OCL.connectorId, Literal(connector_id))
            )

        def remove_required(graph, document):
            graph.remove((role(graph), OCL.semanticRole, None))

        def replace_class(graph, document):
            subject = role(graph)
            graph.remove((subject, RDF.type, OCL.ConnectorSemanticRole))
            graph.add((subject, RDF.type, OCL.DerivationInput))

        def add_unexpected(graph, document):
            graph.add((role(graph), OCL.unexpectedPredicate, Literal("unexpected")))

        def replace_datatype(graph, document):
            graph.set(
                (
                    URIRef(document["@id"]),
                    OCL.canonicalRoutineRevision,
                    Literal("one"),
                )
            )

        cases = (
            (remove_required, "source_shape=shape:ConnectorRoleSemanticRoleProperty"),
            (replace_class, "source_shape=shape:ConnectorRoleShape"),
            (add_unexpected, "source_shape=shape:ConnectorRoleShape"),
            (replace_datatype, "source_shape=shape:ProfileRevisionProperty"),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                document, graph = self.parse_jsonld(PROFILE_PATH)
                mutation(graph, document)
                triples = frozenset(graph)
                first = self.validate_graph(graph, PROFILE_PATH)
                second = self.validate_graph(graph, PROFILE_PATH)
                self.assertEqual(first, second)
                self.assertEqual(frozenset(graph), triples)
                self.assertTrue(any(expected in error for error in first), first)
                self.assertIsNone(re.search(r"\bN[0-9a-f]{16,}\b", "\n".join(first)))
        self.assertEqual((self.root / PROFILE_PATH).read_bytes(), fixture_bytes)

    def test_manifest_rdf_mutations_have_stable_shacl_diagnostics(self):
        fixture_bytes = (self.root / MANIFEST_PATH).read_bytes()

        def remove_required(graph, document):
            root = URIRef(document["@id"])
            graph.remove((graph.value(root, OCL.algorithm), OCL.algorithmVersion, None))

        def replace_class(graph, document):
            subject = next(graph.subjects(RDF.type, OCL.DerivationInput))
            graph.remove((subject, RDF.type, OCL.DerivationInput))
            graph.add((subject, RDF.type, OCL.DerivationMember))

        def add_unexpected(graph, document):
            root = URIRef(document["@id"])
            policy = graph.value(root, OCL.dataQualityPolicy)
            graph.add((policy, OCL.unexpectedPredicate, Literal("unexpected")))

        def replace_datatype(graph, document):
            root = URIRef(document["@id"])
            ready = graph.value(root, OCL.readyCondition)
            graph.set((ready, OCL.minimumValidMembers, Literal("two")))

        cases = (
            (remove_required, "source_shape=shape:AlgorithmVersionProperty"),
            (replace_class, "source_shape=shape:InputShape"),
            (add_unexpected, "source_shape=shape:DataQualityShape"),
            (replace_datatype, "source_shape=shape:ReadyMinimumProperty"),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                document, graph = self.parse_jsonld(MANIFEST_PATH)
                mutation(graph, document)
                triples = frozenset(graph)
                first = self.validate_graph(graph, MANIFEST_PATH)
                second = self.validate_graph(graph, MANIFEST_PATH)
                self.assertEqual(first, second)
                self.assertEqual(frozenset(graph), triples)
                self.assertTrue(any(expected in error for error in first), first)
                self.assertFalse(any("focus=N" in error for error in first))
                self.assertFalse(any("value=N" in error for error in first))
        self.assertEqual((self.root / MANIFEST_PATH).read_bytes(), fixture_bytes)

    def test_dependency_validation_failure_and_runtime_errors_are_stable(self):
        missing = ["dependency pyshacl==0.31.0 is not installed"]
        with mock.patch.object(
            routine_semantics.routine_schemas,
            "_dependency_errors",
            return_value=missing.copy(),
        ), mock.patch.object(routine_semantics, "_pyshacl_validate", None):
            self.assertEqual(routine_semantics.validate(self.root), missing)
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                result = routine_semantics.main(self.root)
        self.assertEqual(result, 1)
        self.assertEqual(output.getvalue(), missing[0] + "\n")

        _, graph = self.parse_jsonld(PROFILE_PATH)
        shapes = self.parse_shapes()
        cases = (
            (RuntimeError("volatile details"), "SHACL validation error: RuntimeError"),
            ((False, object(), "volatile report text"), "SHACL validation failed"),
        )
        for outcome, expected in cases:
            with self.subTest(expected=expected):
                errors = []
                patched = (
                    mock.patch.object(
                        routine_semantics, "_pyshacl_validate", side_effect=outcome
                    )
                    if isinstance(outcome, Exception)
                    else mock.patch.object(
                        routine_semantics, "_pyshacl_validate", return_value=outcome
                    )
                )
                with patched:
                    routine_semantics._validate_shacl_graph(
                        graph, shapes, PROFILE_PATH, errors
                    )
                self.assertEqual(errors, [f"{PROFILE_PATH}: {expected}"])
                self.assertNotIn("volatile", errors[0])

    def test_duplicate_role_source_and_member_ids_are_rejected(self):
        profile_original = (self.root / PROFILE_PATH).read_bytes()
        manifest_original = (self.root / MANIFEST_PATH).read_bytes()

        self.mutate(
            PROFILE_PATH,
            lambda value: value["connector_roles"].append(
                copy.deepcopy(value["connector_roles"][0])
            ),
        )
        self.assert_error("connector_id: duplicate")
        (self.root / PROFILE_PATH).write_bytes(profile_original)

        self.mutate(
            MANIFEST_PATH,
            lambda value: value["inputs"][1].update(
                source_id=value["inputs"][0]["source_id"]
            ),
        )
        self.assert_error("source_id: duplicate")
        (self.root / MANIFEST_PATH).write_bytes(manifest_original)

        self.mutate(
            MANIFEST_PATH,
            lambda value: value["members"][1].update(id=value["members"][0]["id"]),
        )
        self.assert_error("$.members[1].id: duplicate")

    def test_s223_class_aspect_and_mapping_requirements_are_enforced(self):
        original = (self.root / PROFILE_PATH).read_bytes()

        def mapping(value):
            return value["connector_roles"][0]["binding"]["s223_mapping"]

        cases = (
            (
                lambda value: mapping(value).update(
                    property_class="s223:EnumeratedProperty"
                ),
                "s223:EnumeratedProperty is forbidden",
            ),
            (
                lambda value: mapping(value)["aspects"].append("s223:aggregate-max"),
                "unreviewed S223 aspect 's223:aggregate-max'",
            ),
            (
                lambda value: mapping(value).pop("quantity_kind"),
                "quantity_kind: quantifiable mapping requires a value",
            ),
            (
                lambda value: mapping(value).pop("qudt_unit"),
                "qudt_unit: quantifiable mapping requires a value",
            ),
            (
                lambda value: (
                    mapping(value).update(
                        property_class="s223:EnumeratedObservableProperty"
                    ),
                    mapping(value).pop("quantity_kind"),
                    mapping(value).pop("qudt_unit"),
                ),
                "enumeration_kind: enumerated mapping requires a value",
            ),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                self.mutate(PROFILE_PATH, mutation)
                self.assert_error(expected)
                (self.root / PROFILE_PATH).write_bytes(original)

    def test_input_connector_can_bind_an_actuatable_setpoint(self):
        profile = self.read_json(PROFILE_PATH)
        role = next(
            role
            for role in profile["connector_roles"]
            if role["connector_id"] == "zone_cooling_setpoint"
        )
        self.assertEqual(role["direction"], "input")
        self.assertEqual(
            role["binding"]["s223_mapping"]["property_class"],
            "s223:QuantifiableActuatableProperty",
        )
        self.assertEqual(routine_semantics.validate(self.root), [])

    def test_connector_semantic_status_and_topology_obligations_are_required(self):
        original = (self.root / PROFILE_PATH).read_bytes()
        cases = (
            (
                lambda value: value["connector_roles"][0].pop("semantic_role"),
                "semantic_role: nonempty description is required",
            ),
            (
                lambda value: value["connector_roles"][0].pop("mapping_status"),
                "mapping_status: must be 'provisional' or 'verified'",
            ),
            (
                lambda value: value["connector_roles"][0].pop(
                    "topology_requirements"
                ),
                "topology_requirements: physical binding requires at least one obligation",
            ),
            (
                lambda value: value["connector_roles"][0].update(
                    topology_requirements=[]
                ),
                "topology_requirements: physical binding requires at least one obligation",
            ),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                self.mutate(PROFILE_PATH, mutation)
                self.assert_error(expected)
                (self.root / PROFILE_PATH).write_bytes(original)

    def test_s223_medium_may_be_explicitly_not_applicable(self):
        self.mutate(
            PROFILE_PATH,
            lambda value: value["connector_roles"][0]["binding"][
                "s223_mapping"
            ].update(medium=None),
        )
        self.assertEqual(routine_semantics.validate(self.root), [])

    def test_derivation_manifest_names_the_ontology_pin_authority(self):
        original = (self.root / MANIFEST_PATH).read_bytes()
        cases = (
            (
                lambda value: value.pop("ontology_pins"),
                "'ontology_pins' is a required property",
            ),
            (
                lambda value: value.update(ontology_pins="other-pins.json"),
                "was expected",
            ),
        )
        for mutation, expected in cases:
            with self.subTest(expected=expected):
                self.mutate(MANIFEST_PATH, mutation)
                self.assert_error(expected)
                (self.root / MANIFEST_PATH).write_bytes(original)

    def test_cardinality_and_member_minimums_must_be_coherent(self):
        profile_original = (self.root / PROFILE_PATH).read_bytes()
        manifest_original = (self.root / MANIFEST_PATH).read_bytes()

        self.mutate(
            PROFILE_PATH,
            lambda value: value["connector_roles"][0]["cardinality"].update(
                minimum=4, maximum=3
            ),
        )
        self.assert_error("cardinality: minimum exceeds maximum")
        (self.root / PROFILE_PATH).write_bytes(profile_original)

        self.mutate(
            MANIFEST_PATH,
            lambda value: value["ready_condition"].update(minimum_valid_members=4),
        )
        self.assert_error("exceeds declared member count 3")
        (self.root / MANIFEST_PATH).write_bytes(manifest_original)

        self.mutate(
            MANIFEST_PATH,
            lambda value: value["ready_condition"].update(minimum_valid_members=1),
        )
        self.assert_error("minimum valid member counts must agree")

    def test_derivation_relationship_members_and_exclusions_fail_closed(self):
        profile_original = (self.root / PROFILE_PATH).read_bytes()
        manifest_original = (self.root / MANIFEST_PATH).read_bytes()

        def derived_role(value):
            return next(
                role
                for role in value["connector_roles"]
                if role["binding"]["kind"] == "derived-signal"
            )

        profile_cases = (
            (
                lambda value: derived_role(value)["binding"].pop(
                    "derivation_manifest_ref"
                ),
                "derived role requires a derivation manifest reference",
            ),
            (
                lambda value: value["connector_roles"].remove(derived_role(value)),
                "output is not referenced by a derived connector role",
            ),
        )
        for mutation, expected in profile_cases:
            with self.subTest(expected=expected):
                self.mutate(PROFILE_PATH, mutation)
                self.assert_error(expected)
                (self.root / PROFILE_PATH).write_bytes(profile_original)

        manifest_cases = (
            (
                lambda value: value["output"].update(id="different_output"),
                "binding.output_id must equal derivation manifest output id",
            ),
            (
                lambda value: value["inputs"][0]["source"].update(
                    member_id="undeclared-zone"
                ),
                "unknown member 'undeclared-zone'",
            ),
            (
                lambda value: value["exclusions"][0]["member_ids"].append(
                    "undeclared-zone"
                ),
                "unknown member 'undeclared-zone'",
            ),
            (
                lambda value: value.pop("exclusions"),
                "'exclusions' is a required property",
            ),
            (
                lambda value: value.update(
                    reset_behavior={
                        "kind": "source",
                        "source_id": "urn:open-control-library:source:missing",
                    }
                ),
                "reset_behavior.source_id: unknown source",
            ),
        )
        for mutation, expected in manifest_cases:
            with self.subTest(expected=expected):
                self.mutate(MANIFEST_PATH, mutation)
                self.assert_error(expected)
                (self.root / MANIFEST_PATH).write_bytes(manifest_original)

    def test_production_semantic_sidecars_remain_deferred(self):
        destination = self.root / "routines/g36/test-only/semantics.jsonld"
        destination.parent.mkdir(parents=True)
        shutil.copyfile(self.root / PROFILE_PATH, destination)
        self.assert_error("production semantic artifact is deferred")

    def test_cli_usage_sorted_errors_and_no_traceback(self):
        self.mutate(
            PROFILE_PATH,
            lambda value: (
                value["connector_roles"].append(
                    copy.deepcopy(value["connector_roles"][0])
                ),
                value["connector_roles"][0]["cardinality"].update(
                    minimum=5, maximum=3
                ),
            ),
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_semantics.main(self.root)
        self.assertEqual(result, 1)
        lines = output.getvalue().splitlines()
        self.assertEqual(lines, sorted(lines))
        self.assertNotIn("Traceback", output.getvalue())

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = routine_semantics.main(self.root, ["--network"])
        self.assertEqual(result, 2)
        self.assertEqual(output.getvalue(), "usage: routine_semantics.py\n")


if __name__ == "__main__":
    unittest.main()
