#!/usr/bin/env python3
"""Validate offline routine semantic contracts and synthetic fixtures."""

import hashlib
import json
import sys
from pathlib import Path
from urllib.parse import urldefrag

from jsonschema import Draft202012Validator
from rdflib import Graph, URIRef
from rdflib.namespace import OWL, RDF

try:
    from tools.lint import routine_schemas
except ModuleNotFoundError:
    import routine_schemas


REPO_ROOT = Path(__file__).resolve().parents[2]
ONTOLOGY_ROOT = Path("routines/ontology")
PIN_PATH = ONTOLOGY_ROOT / "ontology-pins.json"
VOCABULARY_PATH = ONTOLOGY_ROOT / "ocl-vocabulary.ttl"
FIXTURE_ROOT = Path("tools/lint/tests/fixtures/routine_semantics")
PROFILE_FIXTURE = "routine-semantic-profile.jsonld"
DERIVATION_FIXTURE = "routine-derivation-manifest.jsonld"
ONTOLOGY_FILES = frozenset((PIN_PATH.name, VOCABULARY_PATH.name))
FIXTURE_SCHEMAS = {
    PROFILE_FIXTURE: routine_schemas.SEMANTIC_PROFILE_ID,
    DERIVATION_FIXTURE: routine_schemas.DERIVATION_MANIFEST_ID,
}
LOCAL_NAMESPACE = "urn:open-control-library:ontology:"
ALLOWED_ASPECTS = frozenset(
    (
        "s223:Aspect-Setpoint",
        "s223:Aspect-Delta",
        "s223:Aspect-Maximum",
    )
)
MAPPING_STATUSES = frozenset(("verified", "provisional"))
EXPECTED_PINS = {
    "schema": "cxf-library/ontology-pins/v1",
    "brick": {
        "namespace": "https://brickschema.org/schema/Brick#",
        "repository": "BrickSchema/Brick",
        "release": "v1.4.4",
        "commit": "4b5be60d27f9b4d96fe477f45513fa71afebe684",
        "artifact_url": "https://github.com/BrickSchema/Brick/releases/download/v1.4.4/Brick.ttl",
        "sha256": "b65720b7b9b64c646745c689777e6138c0d59ce0088df0aeb78fbd444d04d8e7",
    },
    "s223_compatibility": {
        "core_namespace": "http://data.ashrae.org/standard223#",
        "g36_extension_namespace": "http://data.ashrae.org/standard223/1.0/extensions/g36#",
        "version": "1.0.0-ppr.2.1",
        "repository": "open223/open223.info",
        "commit": "97656845cab16183e64e9611c94f40a6fad95226",
        "path": "223p-1.0.0-ppr2.1.ttl",
        "git_blob_sha1": "c2ee998a1e0f5cc3e496ff9c20c30e01019ff250",
        "artifact_url": "https://open223.info/223p-1.0.0-ppr2.1.ttl",
        "sha256": "1f156f9938c0be430d2216e01e31bb183c438ba318d8d4a23d2f074ebcd6f573",
        "qudt_import_observation": "http://qudt.org/3.1.8/shacl/qudt-all",
        "status": "compatibility-baseline-not-final-standard",
    },
    "qudt": {
        "quantitykind_namespace": "http://qudt.org/vocab/quantitykind/",
        "unit_namespace": "http://qudt.org/vocab/unit/",
        "repository": "qudt/qudt-public-repo",
        "release": "v3.1.4",
        "annotated_tag": "e6cba51f5769691a926e000cbeb044d4d5cd754e",
        "commit": "5a19ef66a5b8d8c404f469244304afc7d9f83eaa",
        "quantity_kinds": {
            "path": "src/main/rdf/vocab/quantitykinds/VOCAB_QUDT-QUANTITY-KINDS-ALL.ttl",
            "git_blob_sha1": "256b28f66ecc810d90327eee229d6db402a52ad1",
            "sha256": "3e7027106b4f1abbfc6634ebe38b27500af59e79a0b941a49fd6a60819adb4cb",
        },
        "units": {
            "path": "src/main/rdf/vocab/unit/VOCAB_QUDT-UNITS-ALL.ttl",
            "git_blob_sha1": "ebf64443fe04d3036f770a6766e328112ba28657",
            "sha256": "5c236e25c58a571f3110ac3ac0197f64fff0131498dab9eac2ff06932ed39e3b",
        },
    },
    "local": {
        "namespace": LOCAL_NAMESPACE,
        "version": "0.1.0-draft",
        "path": VOCABULARY_PATH.as_posix(),
        "sha256": "abb29ebe89fee52f2c73e4ee1335d394e41f95a68f9c7b6d8803d991d17d17a1",
    },
}


def _govern_files(repo_root, relative_root, expected_names, errors):
    directory = repo_root / relative_root
    actual_names = (
        {
            path.name
            for path in directory.iterdir()
            if path.is_file() and not path.name.startswith(".")
        }
        if directory.is_dir()
        else set()
    )
    for name in sorted(set(expected_names) - actual_names):
        errors.append(f"{(relative_root / name).as_posix()}: governed file is missing")
    for name in sorted(actual_names - set(expected_names)):
        errors.append(f"{(relative_root / name).as_posix()}: unexpected governed file")


def _read_utf8_bytes(repo_root, relative_path, errors):
    label = relative_path.as_posix()
    try:
        raw = (repo_root / relative_path).read_bytes()
    except FileNotFoundError:
        return None
    except OSError:
        errors.append(f"{label}: unable to read file")
        return None
    try:
        raw.decode("utf-8")
    except UnicodeDecodeError:
        errors.append(f"{label}: file is not UTF-8")
        return None
    return raw


def _compare_exact(actual, expected, label, errors, location="$"):
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            errors.append(f"{label}: {location} must be an object")
            return
        actual_keys = set(actual)
        expected_keys = set(expected)
        for key in sorted(expected_keys - actual_keys):
            errors.append(f"{label}: {location}.{key} is required by the pinned contract")
        for key in sorted(actual_keys - expected_keys):
            errors.append(f"{label}: {location}.{key} is not allowed by the pinned contract")
        for key in sorted(actual_keys & expected_keys):
            _compare_exact(actual[key], expected[key], label, errors, f"{location}.{key}")
        return
    if actual != expected:
        errors.append(f"{label}: {location} must be {expected!r}, found {actual!r}")


def _check_jsonld_safety(value, label, errors, location="$"):
    start = len(errors)
    if isinstance(value, dict):
        for key, item in value.items():
            child = f"{location}.{key}"
            if key == "@import":
                errors.append(f"{label}: {child}: JSON-LD @import is forbidden")
            if key == "@context":
                if child != "$.@context":
                    errors.append(f"{label}: {child}: nested JSON-LD context is forbidden")
                elif not isinstance(item, dict):
                    errors.append(
                        f"{label}: {child}: context must be one embedded local object"
                    )
            _check_jsonld_safety(item, label, errors, child)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            _check_jsonld_safety(item, label, errors, f"{location}[{index}]")
    return len(errors) == start


def _parse_vocabulary(raw, errors):
    if raw is None:
        return None
    label = VOCABULARY_PATH.as_posix()
    try:
        graph = Graph().parse(data=raw, format="turtle", publicID=LOCAL_NAMESPACE)
    except Exception as exc:
        errors.append(f"{label}: Turtle parse failed: {exc}")
        return None
    if any(graph.triples((None, OWL.imports, None))):
        errors.append(f"{label}: owl:imports is forbidden")
    owned_types = frozenset((OWL.Class, OWL.ObjectProperty, OWL.DatatypeProperty))
    for subject, _, object_type in graph.triples((None, RDF.type, None)):
        if object_type in owned_types and not str(subject).startswith(LOCAL_NAMESPACE):
            errors.append(
                f"{label}: class/property subject {subject} is outside the local namespace"
            )
    return graph


def _check_local_class(value, label, graph, errors):
    if not isinstance(value, str) or not value.startswith("ocl:") or graph is None:
        return
    term = URIRef(LOCAL_NAMESPACE + value.removeprefix("ocl:"))
    if (term, RDF.type, OWL.Class) not in graph:
        errors.append(f"{label}: local class {value!r} is absent from the local vocabulary")


def _duplicates(rows, field, label, errors):
    seen = {}
    for index, row in enumerate(rows):
        if not isinstance(row, dict) or field not in row:
            continue
        value = row[field]
        if not isinstance(value, str):
            continue
        if value in seen:
            errors.append(
                f"{label}[{index}].{field}: duplicate {value!r}; first used at index {seen[value]}"
            )
        else:
            seen[value] = index
    return seen


def _check_profile(profile, vocabulary, errors):
    label = (FIXTURE_ROOT / PROFILE_FIXTURE).as_posix()
    roles = profile.get("connector_roles")
    if not isinstance(roles, list):
        return []
    _duplicates(roles, "connector_id", f"{label}: $.connector_roles", errors)
    _duplicates([profile, *roles], "@id", f"{label}: semantic entities", errors)
    _check_local_class(profile.get("@type"), f"{label}: $.@type", vocabulary, errors)
    derived = []
    for index, role in enumerate(roles):
        if not isinstance(role, dict):
            continue
        role_label = f"{label}: $.connector_roles[{index}]"
        _check_local_class(role.get("@type"), f"{role_label}.@type", vocabulary, errors)
        semantic_role = role.get("semantic_role")
        if not isinstance(semantic_role, str) or not semantic_role.strip():
            errors.append(f"{role_label}.semantic_role: nonempty description is required")
        if role.get("mapping_status") not in MAPPING_STATUSES:
            errors.append(
                f"{role_label}.mapping_status: must be 'provisional' or 'verified'"
            )
        cardinality = role.get("cardinality")
        if isinstance(cardinality, dict):
            minimum = cardinality.get("minimum")
            maximum = cardinality.get("maximum")
            if (
                isinstance(minimum, int)
                and not isinstance(minimum, bool)
                and isinstance(maximum, int)
                and not isinstance(maximum, bool)
                and minimum > maximum
            ):
                errors.append(f"{role_label}.cardinality: minimum exceeds maximum")

        binding = role.get("binding")
        if not isinstance(binding, dict):
            continue
        _check_local_class(binding.get("@type"), f"{role_label}.binding.@type", vocabulary, errors)
        _check_local_class(
            binding.get("local_class"),
            f"{role_label}.binding.local_class",
            vocabulary,
            errors,
        )
        kind = binding.get("kind")
        if kind == "derived-signal":
            derived.append((index, binding))
        if kind != "physical-or-bms-point":
            continue
        topology_requirements = role.get("topology_requirements")
        if not isinstance(topology_requirements, list) or not topology_requirements:
            errors.append(
                f"{role_label}.topology_requirements: physical binding requires "
                "at least one obligation"
            )
        mapping = binding.get("s223_mapping")
        if not isinstance(mapping, dict):
            continue
        property_class = mapping.get("property_class")
        if property_class == "s223:EnumeratedProperty":
            errors.append(
                f"{role_label}.binding.s223_mapping.property_class: "
                "s223:EnumeratedProperty is forbidden"
            )
        if isinstance(property_class, str) and property_class.startswith(
            "s223:Quantifiable"
        ):
            for field in ("quantity_kind", "qudt_unit"):
                if not mapping.get(field):
                    errors.append(
                        f"{role_label}.binding.s223_mapping.{field}: "
                        "quantifiable mapping requires a value"
                    )
        if isinstance(property_class, str) and property_class.startswith(
            "s223:Enumerated"
        ):
            if not mapping.get("enumeration_kind"):
                errors.append(
                    f"{role_label}.binding.s223_mapping.enumeration_kind: "
                    "enumerated mapping requires a value"
                )
        aspects = mapping.get("aspects")
        if isinstance(aspects, list):
            for aspect in sorted(
                {item for item in aspects if isinstance(item, str)} - ALLOWED_ASPECTS
            ):
                errors.append(
                    f"{role_label}.binding.s223_mapping.aspects: "
                    f"unreviewed S223 aspect {aspect!r}"
                )
    return derived


def _check_manifest(manifest, vocabulary, errors):
    label = (FIXTURE_ROOT / DERIVATION_FIXTURE).as_posix()
    algorithm = manifest.get("algorithm")
    output = manifest.get("output")
    inputs = manifest.get("inputs")
    members = manifest.get("members")
    exclusions = manifest.get("exclusions")
    for location, value in (
        ("$.@type", manifest.get("@type")),
        ("$.algorithm.@type", algorithm.get("@type") if isinstance(algorithm, dict) else None),
        ("$.output.@type", output.get("@type") if isinstance(output, dict) else None),
    ):
        _check_local_class(value, f"{label}: {location}", vocabulary, errors)

    input_rows = inputs if isinstance(inputs, list) else []
    member_rows = members if isinstance(members, list) else []
    exclusion_rows = exclusions if isinstance(exclusions, list) else []
    _duplicates(input_rows, "id", f"{label}: $.inputs", errors)
    source_indexes = _duplicates(input_rows, "source_id", f"{label}: $.inputs", errors)
    _duplicates(member_rows, "id", f"{label}: $.members", errors)
    _duplicates(exclusion_rows, "id", f"{label}: $.exclusions", errors)

    entity_rows = [manifest]
    for value in (algorithm, output):
        if isinstance(value, dict):
            entity_rows.append(value)
    entity_rows.extend(row for row in input_rows + member_rows + exclusion_rows if isinstance(row, dict))
    _duplicates(entity_rows, "@id", f"{label}: semantic entities", errors)

    member_ids = set()
    for row in member_rows:
        if isinstance(row, dict) and isinstance(row.get("id"), str):
            member_ids.add(row["id"])
    linked_members = set()
    input_units = []
    for index, row in enumerate(input_rows):
        if not isinstance(row, dict):
            continue
        _check_local_class(
            row.get("@type"), f"{label}: $.inputs[{index}].@type", vocabulary, errors
        )
        source = row.get("source")
        if not isinstance(source, dict):
            continue
        _check_local_class(
            source.get("local_class"),
            f"{label}: $.inputs[{index}].source.local_class",
            vocabulary,
            errors,
        )
        member_id = source.get("member_id")
        if isinstance(member_id, str):
            linked_members.add(member_id)
            if member_id not in member_ids:
                errors.append(
                    f"{label}: $.inputs[{index}].source.member_id: "
                    f"unknown member {member_id!r}"
                )
        if source.get("value_kind") == "real":
            qudt_unit = source.get("qudt_unit")
            if not isinstance(qudt_unit, str) or not qudt_unit:
                errors.append(
                    f"{label}: $.inputs[{index}].source.qudt_unit: "
                    "real source requires a unit"
                )
            else:
                input_units.append((index, qudt_unit))
    for member_id in sorted(member_ids - linked_members):
        errors.append(f"{label}: $.members: member {member_id!r} has no linked input")

    for index, exclusion in enumerate(exclusion_rows):
        if not isinstance(exclusion, dict):
            continue
        _check_local_class(
            exclusion.get("@type"),
            f"{label}: $.exclusions[{index}].@type",
            vocabulary,
            errors,
        )
        excluded = exclusion.get("member_ids")
        if not isinstance(excluded, list):
            continue
        seen = set()
        for member_index, member_id in enumerate(excluded):
            if not isinstance(member_id, str):
                continue
            if member_id in seen:
                errors.append(
                    f"{label}: $.exclusions[{index}].member_ids[{member_index}]: "
                    f"duplicate member {member_id!r}"
                )
            seen.add(member_id)
            if member_id not in member_ids:
                errors.append(
                    f"{label}: $.exclusions[{index}].member_ids[{member_index}]: "
                    f"unknown member {member_id!r}"
                )

    member_count = len(member_ids)
    data_quality = manifest.get("data_quality")
    ready = manifest.get("ready_condition")
    quality_minimum = (
        data_quality.get("minimum_valid_members")
        if isinstance(data_quality, dict)
        else None
    )
    ready_minimum = (
        ready.get("minimum_valid_members") if isinstance(ready, dict) else None
    )
    for location, minimum in (
        ("$.data_quality.minimum_valid_members", quality_minimum),
        ("$.ready_condition.minimum_valid_members", ready_minimum),
    ):
        if isinstance(minimum, int) and not isinstance(minimum, bool) and minimum > member_count:
            errors.append(
                f"{label}: {location}: {minimum} exceeds declared member count {member_count}"
            )
    if (
        isinstance(quality_minimum, int)
        and isinstance(ready_minimum, int)
        and quality_minimum != ready_minimum
    ):
        errors.append(
            f"{label}: ready and data-quality minimum valid member counts must agree"
        )

    unit_policy = manifest.get("unit_policy")
    if isinstance(unit_policy, dict):
        input_policy = unit_policy.get("input_units")
        conversion = unit_policy.get("conversion")
        output_unit = unit_policy.get("output_unit")
        if input_policy == "same-as-output" and conversion != "none":
            errors.append(
                f"{label}: $.unit_policy.conversion must be 'none' for same-unit inputs"
            )
        if input_policy == "convert-to-output" and conversion != "algorithm-defined":
            errors.append(
                f"{label}: $.unit_policy.conversion must be 'algorithm-defined' when inputs are converted"
            )
        if input_policy == "same-as-output" and isinstance(output_unit, str):
            for index, input_unit in input_units:
                if input_unit != output_unit:
                    errors.append(
                        f"{label}: $.inputs[{index}].source.qudt_unit must equal "
                        f"output unit {output_unit!r}"
                    )

    output_scope = manifest.get("output_scope")
    if isinstance(output_scope, dict) and output_scope.get("kind") == "member":
        member_id = output_scope.get("member_id")
        if isinstance(member_id, str) and member_id not in member_ids:
            errors.append(f"{label}: $.output_scope.member_id: unknown member {member_id!r}")
    reset = manifest.get("reset_behavior")
    if isinstance(reset, dict) and reset.get("kind") == "source":
        source_id = reset.get("source_id")
        if isinstance(source_id, str) and source_id not in source_indexes:
            errors.append(f"{label}: $.reset_behavior.source_id: unknown source {source_id!r}")


def _check_cross_document(profile, manifest, derived, errors):
    profile_label = (FIXTURE_ROOT / PROFILE_FIXTURE).as_posix()
    manifest_label = (FIXTURE_ROOT / DERIVATION_FIXTURE).as_posix()
    if profile is None or manifest is None:
        if derived and manifest is None:
            errors.append(f"{profile_label}: derived role refers to unavailable manifest")
        return
    if profile.get("canonical_id") != manifest.get("canonical_id"):
        errors.append(f"{manifest_label}: $.canonical_id must equal semantic profile canonical_id")
    if profile.get("revision") != manifest.get("revision"):
        errors.append(f"{manifest_label}: $.revision must equal semantic profile revision")
    output = manifest.get("output")
    output_id = output.get("id") if isinstance(output, dict) else None
    matching_roles = 0
    for index, binding in derived:
        reference = binding.get("derivation_manifest_ref")
        binding_output = binding.get("output_id")
        if not isinstance(reference, str):
            errors.append(
                f"{profile_label}: $.connector_roles[{index}].binding: "
                "derived role requires a derivation manifest reference"
            )
            continue
        resource, fragment = urldefrag(reference)
        if resource != DERIVATION_FIXTURE:
            errors.append(
                f"{profile_label}: $.connector_roles[{index}].binding.derivation_manifest_ref: "
                f"must reference {DERIVATION_FIXTURE!r} in the synthetic fixture set"
            )
        if fragment != binding_output:
            errors.append(
                f"{profile_label}: $.connector_roles[{index}].binding.derivation_manifest_ref: "
                "fragment must equal derived output_id"
            )
        if binding_output != output_id:
            errors.append(
                f"{profile_label}: $.connector_roles[{index}].binding.output_id must equal "
                "derivation manifest output id"
            )
        else:
            matching_roles += 1
    if matching_roles == 0:
        errors.append(f"{manifest_label}: output is not referenced by a derived connector role")
    elif matching_roles > 1:
        errors.append(f"{manifest_label}: output is referenced by multiple derived connector roles")


def _parse_jsonld(document, relative_path, safe, errors):
    if document is None or not safe:
        return
    label = relative_path.as_posix()
    try:
        payload = json.dumps(document, ensure_ascii=False, allow_nan=False)
        public_id = document.get("@id") if isinstance(document.get("@id"), str) else LOCAL_NAMESPACE
        Graph().parse(data=payload, format="json-ld", publicID=public_id)
    except Exception as exc:
        errors.append(f"{label}: JSON-LD parse failed: {exc}")


def _check_fixture_placement(repo_root, errors):
    g36_root = repo_root / "routines/g36"
    if not g36_root.is_dir():
        return
    forbidden_names = frozenset(("semantics.jsonld", "derivation-manifest.jsonld"))
    for path in sorted(g36_root.rglob("*.jsonld")):
        if path.name in forbidden_names:
            errors.append(
                f"{path.relative_to(repo_root).as_posix()}: production semantic artifact "
                "is deferred"
            )


def validate(repo_root=REPO_ROOT):
    """Return deterministic offline semantic-contract errors."""
    repo_root = Path(repo_root)
    errors = routine_schemas._dependency_errors()
    schemas_by_id, registry = routine_schemas._load_schemas(repo_root, errors)
    _govern_files(repo_root, ONTOLOGY_ROOT, ONTOLOGY_FILES, errors)
    _govern_files(repo_root, FIXTURE_ROOT, FIXTURE_SCHEMAS, errors)
    _check_fixture_placement(repo_root, errors)

    pins = routine_schemas._read_json(repo_root, PIN_PATH, errors)
    if pins is not None:
        _compare_exact(pins, EXPECTED_PINS, PIN_PATH.as_posix(), errors)

    vocabulary_raw = _read_utf8_bytes(repo_root, VOCABULARY_PATH, errors)
    if vocabulary_raw is not None:
        actual_hash = hashlib.sha256(vocabulary_raw).hexdigest()
        expected_hash = EXPECTED_PINS["local"]["sha256"]
        if actual_hash != expected_hash:
            errors.append(
                f"{VOCABULARY_PATH.as_posix()}: SHA-256 must be {expected_hash}, "
                f"found {actual_hash}"
            )
    vocabulary = _parse_vocabulary(vocabulary_raw, errors)

    fixtures = {}
    safety = {}
    for name in FIXTURE_SCHEMAS:
        path = FIXTURE_ROOT / name
        document = routine_schemas._read_json(repo_root, path, errors)
        if document is None:
            continue
        fixtures[name] = document
        safety[name] = _check_jsonld_safety(document, path.as_posix(), errors)

    if schemas_by_id is not None and registry is not None:
        for name, schema_id in FIXTURE_SCHEMAS.items():
            document = fixtures.get(name)
            if document is None:
                continue
            validator = Draft202012Validator(schemas_by_id[schema_id], registry=registry)
            for error in validator.iter_errors(document):
                errors.append(
                    f"{(FIXTURE_ROOT / name).as_posix()}: "
                    f"{routine_schemas._instance_path(error)}: {error.message}"
                )

    for name, document in fixtures.items():
        _parse_jsonld(document, FIXTURE_ROOT / name, safety.get(name, False), errors)

    profile = fixtures.get(PROFILE_FIXTURE)
    manifest = fixtures.get(DERIVATION_FIXTURE)
    derived = _check_profile(profile, vocabulary, errors) if profile is not None else []
    if manifest is not None:
        _check_manifest(manifest, vocabulary, errors)
    _check_cross_document(profile, manifest, derived, errors)
    return sorted(errors)


def main(repo_root=REPO_ROOT, argv=None):
    args = [] if argv is None else list(argv)
    if args:
        print("usage: routine_semantics.py")
        return 2
    errors = validate(repo_root)
    if errors:
        print("\n".join(errors))
        return 1
    print("routine semantic lint: ontology pins, local vocabulary, 2 schemas, 2 synthetic fixtures OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(argv=sys.argv[1:]))
