#!/usr/bin/env python3
"""Validate routine schema resources and their synthetic contract fixtures."""

import importlib.metadata
import json
import math
import re
import sys
from pathlib import Path, PurePosixPath
from urllib.parse import unquote, urldefrag

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError
from referencing import Registry
from referencing.jsonschema import DRAFT202012


REPO_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = Path("routines/schemas")
FIXTURE_ROOT = Path("tools/lint/tests/fixtures/routine_schemas")
G36_ROOT = Path("routines/g36")
DIALECT = "https://json-schema.org/draft/2020-12/schema"

COMMON_ID = "https://open-control-library.example/schemas/routine-common-v1.json"
CLASS_MANIFEST_ID = (
    "https://open-control-library.example/schemas/routine-class-manifest-v1.json"
)
INTERFACE_ID = "https://open-control-library.example/schemas/routine-interface-v2.json"
SPECIALIZATION_ID = (
    "https://open-control-library.example/schemas/routine-specialization-v1.json"
)

SCHEMA_FILES = {
    "common.schema.json": COMMON_ID,
    "class-manifest.schema.json": CLASS_MANIFEST_ID,
    "interface.schema.json": INTERFACE_ID,
    "specialization.schema.json": SPECIALIZATION_ID,
}
FIXTURE_SCHEMAS = {
    "class-manifest.json": CLASS_MANIFEST_ID,
    "interface.json": INTERFACE_ID,
    "specialization.json": SPECIALIZATION_ID,
}
DEPENDENCIES = {
    "jsonschema": "4.26.0",
    "referencing": "0.37.0",
}
CANONICAL_RE = re.compile(
    r"^G36-05-(?P<section>0[1-9]|1[0-9]|2[0-2])-(?P<slug>[A-Z]+(?:-[A-Z]+)*)$"
)
NUMERIC_TYPES = frozenset(("real", "integer"))


class _DuplicateKeyError(ValueError):
    pass


class _NonFiniteNumberError(ValueError):
    pass


def _object_without_duplicates(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise _DuplicateKeyError(f"duplicate object key {key!r}")
        value[key] = item
    return value


def _reject_nonfinite(value):
    raise _NonFiniteNumberError(f"non-finite number {value!r} is forbidden")


def _read_json(repo_root, relative_path, errors):
    label = relative_path.as_posix()
    try:
        raw = (repo_root / relative_path).read_bytes()
    except FileNotFoundError:
        errors.append(f"{label}: file is missing")
        return None
    except OSError:
        errors.append(f"{label}: unable to read file")
        return None
    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_object_without_duplicates,
            parse_constant=_reject_nonfinite,
        )
    except UnicodeDecodeError:
        errors.append(f"{label}: file is not UTF-8")
        return None
    except _DuplicateKeyError as exc:
        errors.append(f"{label}: {exc}")
        return None
    except _NonFiniteNumberError as exc:
        errors.append(f"{label}: {exc}")
        return None
    except json.JSONDecodeError as exc:
        errors.append(
            f"{label}: invalid JSON at line {exc.lineno}, column {exc.colno}"
        )
        return None
    if not isinstance(value, dict):
        errors.append(f"{label}: must contain a JSON object")
        return None
    return value


def _walk_refs(value, location="$"):
    if isinstance(value, dict):
        for key, item in value.items():
            child = f"{location}.{key}"
            if key == "$ref":
                yield child, item
            yield from _walk_refs(item, child)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield from _walk_refs(item, f"{location}[{index}]")


def _pointer_target(document, fragment):
    if not fragment:
        return document
    pointer = unquote(fragment)
    if not pointer.startswith("/"):
        raise ValueError("fragment must be an absolute JSON Pointer")
    target = document
    for encoded in pointer[1:].split("/"):
        if re.search(r"~(?![01])", encoded):
            raise ValueError("fragment contains an invalid JSON Pointer escape")
        token = encoded.replace("~1", "/").replace("~0", "~")
        if isinstance(target, dict) and token in target:
            target = target[token]
        elif isinstance(target, list) and token.isdigit() and int(token) < len(target):
            target = target[int(token)]
        else:
            raise KeyError(token)
    return target


def _check_refs(schemas_by_id, schema_name, schema, errors):
    label = (SCHEMA_ROOT / schema_name).as_posix()
    for location, reference in _walk_refs(schema):
        if not isinstance(reference, str):
            errors.append(f"{label}: {location} must be a string")
            continue
        resource_id, fragment = urldefrag(reference)
        if resource_id not in SCHEMA_FILES.values():
            errors.append(
                f"{label}: {location} references forbidden resource {resource_id or reference!r}"
            )
            continue
        target_schema = schemas_by_id.get(resource_id)
        if target_schema is None:
            errors.append(
                f"{label}: {location} references unavailable local resource {resource_id}"
            )
            continue
        try:
            _pointer_target(target_schema, fragment)
        except (KeyError, ValueError) as exc:
            errors.append(f"{label}: {location} cannot resolve {reference!r}: {exc}")


def _dependency_errors():
    errors = []
    for package, expected in DEPENDENCIES.items():
        try:
            actual = importlib.metadata.version(package)
        except importlib.metadata.PackageNotFoundError:
            errors.append(f"dependency {package}=={expected} is not installed")
            continue
        if actual != expected:
            errors.append(
                f"dependency {package} must be {expected}, found {actual}"
            )
    return errors


def _load_schemas(repo_root, errors):
    schema_directory = repo_root / SCHEMA_ROOT
    actual_names = (
        {path.name for path in schema_directory.glob("*.schema.json")}
        if schema_directory.is_dir()
        else set()
    )
    expected_names = set(SCHEMA_FILES)
    for name in sorted(expected_names - actual_names):
        errors.append(f"{(SCHEMA_ROOT / name).as_posix()}: governed schema is missing")
    for name in sorted(actual_names - expected_names):
        errors.append(f"{(SCHEMA_ROOT / name).as_posix()}: unexpected schema file")

    schemas_by_id = {}
    schemas_by_name = {}
    for name, expected_id in SCHEMA_FILES.items():
        schema = _read_json(repo_root, SCHEMA_ROOT / name, errors)
        if schema is None:
            continue
        schemas_by_name[name] = schema
        if schema.get("$schema") != DIALECT:
            errors.append(
                f"{(SCHEMA_ROOT / name).as_posix()}: $schema must be {DIALECT!r}"
            )
        if schema.get("$id") != expected_id:
            errors.append(
                f"{(SCHEMA_ROOT / name).as_posix()}: $id must be {expected_id!r}"
            )
        else:
            schemas_by_id[expected_id] = schema
        try:
            Draft202012Validator.check_schema(schema)
        except SchemaError as exc:
            errors.append(
                f"{(SCHEMA_ROOT / name).as_posix()}: invalid Draft 2020-12 schema: {exc.message}"
            )

    for name, schema in schemas_by_name.items():
        _check_refs(schemas_by_id, name, schema, errors)
    if errors or len(schemas_by_id) != len(SCHEMA_FILES):
        return None, None

    registry = Registry().with_resources(
        (schema_id, DRAFT202012.create_resource(schema))
        for schema_id, schema in schemas_by_id.items()
    )
    return schemas_by_id, registry


def _instance_path(error):
    path = "$"
    for part in error.absolute_path:
        path += f"[{part}]" if isinstance(part, int) else f".{part}"
    return path


def _load_fixtures(repo_root, schemas_by_id, registry, errors):
    fixture_directory = repo_root / FIXTURE_ROOT
    actual_names = (
        {path.name for path in fixture_directory.glob("*.json")}
        if fixture_directory.is_dir()
        else set()
    )
    expected_names = set(FIXTURE_SCHEMAS)
    for name in sorted(expected_names - actual_names):
        errors.append(f"{(FIXTURE_ROOT / name).as_posix()}: fixture is missing")
    for name in sorted(actual_names - expected_names):
        errors.append(f"{(FIXTURE_ROOT / name).as_posix()}: unexpected fixture file")

    fixtures = {}
    for name, schema_id in FIXTURE_SCHEMAS.items():
        value = _read_json(repo_root, FIXTURE_ROOT / name, errors)
        if value is None:
            continue
        fixtures[name] = value
        validator = Draft202012Validator(schemas_by_id[schema_id], registry=registry)
        for error in validator.iter_errors(value):
            errors.append(
                f"{(FIXTURE_ROOT / name).as_posix()}: {_instance_path(error)}: {error.message}"
            )
    if errors or len(fixtures) != len(FIXTURE_SCHEMAS):
        return None
    return fixtures


def _duplicates(rows, field, label, errors):
    values = {}
    for index, row in enumerate(rows):
        value = row[field]
        if value in values:
            errors.append(
                f"{label}[{index}].{field}: duplicate {value!r}; first used at index {values[value]}"
            )
        else:
            values[value] = index
    return values


def _path_problem(value, required_prefix=None):
    if value.startswith("/"):
        return "absolute paths are forbidden"
    if "\\" in value:
        return "backslashes are forbidden"
    if any(ord(character) < 32 or ord(character) == 127 for character in value):
        return "control characters are forbidden"
    segments = value.split("/")
    if "" in segments:
        return "empty path segments are forbidden"
    if "." in segments:
        return "dot path segments are forbidden"
    if ".." in segments:
        return "parent traversal is forbidden"
    if required_prefix is not None and not value.startswith(f"{required_prefix}/"):
        return f"path must be below {required_prefix}/"
    return None


def _type_info(type_use, types, label, errors):
    if type_use["kind"] == "primitive":
        return ("primitive", type_use["primitive"])
    name = type_use["type"]
    definition = types.get(name)
    if definition is None:
        errors.append(f"{label}: unknown named type {name!r}")
        return None
    if definition["kind"] == "alias":
        return ("primitive", definition["primitive"])
    return ("enum", name)


def _check_shape(shape, dimensions, label, errors):
    if shape["kind"] == "scalar":
        return
    for index, dimension_id in enumerate(shape["dimensions"]):
        if dimension_id not in dimensions:
            errors.append(
                f"{label}.dimensions[{index}]: unknown dimension {dimension_id!r}"
            )


def _check_scalar(value, type_info, enum_members, constraints, label, errors):
    if type_info is None:
        return
    kind, name = type_info
    valid = False
    if kind == "primitive" and name == "boolean":
        valid = isinstance(value, bool)
    elif kind == "primitive" and name == "integer":
        valid = isinstance(value, int) and not isinstance(value, bool)
    elif kind == "primitive" and name == "real":
        valid = (
            isinstance(value, (int, float))
            and not isinstance(value, bool)
            and math.isfinite(value)
        )
    elif kind == "enum":
        valid = isinstance(value, str) and value in enum_members.get(name, set())
    if not valid:
        expected = name if kind == "primitive" else f"member of enum {name!r}"
        errors.append(f"{label}: value must be {expected}")
        return
    if constraints and kind == "primitive" and name in NUMERIC_TYPES:
        if "minimum" in constraints and value < constraints["minimum"]:
            errors.append(
                f"{label}: value {value!r} is below minimum {constraints['minimum']!r}"
            )
        if "maximum" in constraints and value > constraints["maximum"]:
            errors.append(
                f"{label}: value {value!r} exceeds maximum {constraints['maximum']!r}"
            )


def _check_value(
    value,
    type_info,
    shape,
    concrete_dimensions,
    enum_members,
    constraints,
    label,
    errors,
):
    if shape["kind"] == "scalar":
        _check_scalar(value, type_info, enum_members, constraints, label, errors)
        return
    extents = []
    for dimension_id in shape["dimensions"]:
        extent = concrete_dimensions.get(dimension_id)
        if extent is None:
            return
        extents.append(extent)
    if not isinstance(value, list):
        errors.append(f"{label}: array value must be a JSON array")
        return
    if len(value) != extents[0]:
        errors.append(
            f"{label}: dimension 0 length must be {extents[0]}, found {len(value)}"
        )
    if len(extents) == 1:
        for index, item in enumerate(value):
            _check_scalar(
                item,
                type_info,
                enum_members,
                constraints,
                f"{label}[{index}]",
                errors,
            )
        return
    for row_index, row in enumerate(value):
        if not isinstance(row, list):
            errors.append(f"{label}[{row_index}]: matrix row must be a JSON array")
            continue
        if len(row) != extents[1]:
            errors.append(
                f"{label}[{row_index}]: dimension 1 length must be {extents[1]}, found {len(row)}"
            )
        for column_index, item in enumerate(row):
            _check_scalar(
                item,
                type_info,
                enum_members,
                constraints,
                f"{label}[{row_index}][{column_index}]",
                errors,
            )


def _operand_type(operand, parameters, parameter_types, types, enum_members, label, errors):
    if operand["kind"] == "parameter":
        parameter_id = operand["parameter"]
        parameter = parameters.get(parameter_id)
        if parameter is None:
            errors.append(f"{label}: unknown guard parameter {parameter_id!r}")
            return None
        if parameter["shape"]["kind"] != "scalar":
            errors.append(f"{label}: guard parameter {parameter_id!r} must be scalar")
        return parameter_types.get(parameter_id)
    type_info = _type_info(operand["type"], types, f"{label}.type", errors)
    _check_scalar(
        operand["value"],
        type_info,
        enum_members,
        None,
        f"{label}.value",
        errors,
    )
    return type_info


def _types_compatible(left, right):
    if left is None or right is None:
        return True
    if left == right:
        return True
    return (
        left[0] == "primitive"
        and right[0] == "primitive"
        and left[1] in NUMERIC_TYPES
        and right[1] in NUMERIC_TYPES
    )


def _check_guard(
    guard, parameters, parameter_types, types, enum_members, label, errors
):
    operator = guard["op"]
    if operator in ("and", "or"):
        for index, operand in enumerate(guard["operands"]):
            _check_guard(
                operand,
                parameters,
                parameter_types,
                types,
                enum_members,
                f"{label}.operands[{index}]",
                errors,
            )
        return
    if operator == "not":
        _check_guard(
            guard["operand"],
            parameters,
            parameter_types,
            types,
            enum_members,
            f"{label}.operand",
            errors,
        )
        return
    left = _operand_type(
        guard["left"],
        parameters,
        parameter_types,
        types,
        enum_members,
        f"{label}.left",
        errors,
    )
    right = _operand_type(
        guard["right"],
        parameters,
        parameter_types,
        types,
        enum_members,
        f"{label}.right",
        errors,
    )
    if not _types_compatible(left, right):
        errors.append(f"{label}: guard operands have incompatible types {left} and {right}")
    if operator in ("lt", "lte", "gt", "gte"):
        for side, type_info in (("left", left), ("right", right)):
            if type_info is not None and not (
                type_info[0] == "primitive" and type_info[1] in NUMERIC_TYPES
            ):
                errors.append(
                    f"{label}.{side}: ordering comparison requires a numeric operand"
                )


def _check_manifest(manifest, errors):
    label = (FIXTURE_ROOT / "class-manifest.json").as_posix()
    match = CANONICAL_RE.fullmatch(manifest["id"])
    if match is not None:
        expected_section = f"5.{int(match.group('section'))}"
        if manifest["section"] != expected_section:
            errors.append(
                f"{label}: $.section must be {expected_section!r} for {manifest['id']!r}"
            )

    source = manifest["source"]
    if source["kind"] == "upstream":
        seen_paths = set()
        for index, locator in enumerate(source["files"]):
            path = locator["path"]
            problem = _path_problem(path, "Buildings/Controls/OBC/ASHRAE/G36")
            if problem:
                errors.append(f"{label}: $.source.files[{index}].path: {problem}")
            if path in seen_paths:
                errors.append(
                    f"{label}: $.source.files[{index}].path: duplicate {path!r}"
                )
            seen_paths.add(path)
    else:
        seen_paths = set()
        for index, path in enumerate(source["paths"]):
            problem = _path_problem(path)
            if problem:
                errors.append(f"{label}: $.source.paths[{index}]: {problem}")
            if path in seen_paths:
                errors.append(f"{label}: $.source.paths[{index}]: duplicate {path!r}")
            seen_paths.add(path)

    artifact_names = {
        "interface": "interface.json",
        "specialization_schema": "specialization.schema.json",
        "specialization_config": "specialization.json",
    }
    artifact_parents = set()
    for key, basename in artifact_names.items():
        path = manifest["artifacts"][key]
        problem = _path_problem(path)
        if problem:
            errors.append(f"{label}: $.artifacts.{key}: {problem}")
        if path.rsplit("/", 1)[-1] != basename:
            errors.append(
                f"{label}: $.artifacts.{key} must end with {basename!r}"
            )
        artifact_parents.add(PurePosixPath(path).parent.as_posix())
    if "." in artifact_parents or len(artifact_parents) != 1:
        errors.append(
            f"{label}: $.artifacts paths must share one non-root class directory"
        )


def _check_interface_and_specialization(interface, specialization, errors):
    interface_label = (FIXTURE_ROOT / "interface.json").as_posix()
    specialization_label = (FIXTURE_ROOT / "specialization.json").as_posix()

    type_indexes = _duplicates(interface["types"], "id", f"{interface_label}: $.types", errors)
    types = {name: interface["types"][index] for name, index in type_indexes.items()}
    enum_members = {}
    for type_index, definition in enumerate(interface["types"]):
        if definition["kind"] == "alias":
            for key in ("quantity", "unit", "display_unit"):
                if key in definition and definition[key] != definition[key].strip():
                    errors.append(
                        f"{interface_label}: $.types[{type_index}].{key} must be trimmed"
                    )
            continue
        member_indexes = _duplicates(
            definition["members"],
            "id",
            f"{interface_label}: $.types[{type_index}].members",
            errors,
        )
        _duplicates(
            definition["members"],
            "symbol",
            f"{interface_label}: $.types[{type_index}].members",
            errors,
        )
        enum_members[definition["id"]] = set(member_indexes)

    dimension_indexes = _duplicates(
        interface["dimensions"], "id", f"{interface_label}: $.dimensions", errors
    )
    dimensions = {
        name: interface["dimensions"][index]
        for name, index in dimension_indexes.items()
    }
    parameter_indexes = _duplicates(
        interface["parameters"], "id", f"{interface_label}: $.parameters", errors
    )
    parameters = {
        name: interface["parameters"][index]
        for name, index in parameter_indexes.items()
    }
    _duplicates(
        interface["connectors"], "id", f"{interface_label}: $.connectors", errors
    )

    parameter_types = {}
    for index, parameter in enumerate(interface["parameters"]):
        parameter_types[parameter["id"]] = _type_info(
            parameter["type"],
            types,
            f"{interface_label}: $.parameters[{index}].type",
            errors,
        )
        _check_shape(
            parameter["shape"],
            dimensions,
            f"{interface_label}: $.parameters[{index}].shape",
            errors,
        )
        if parameter["configurability"] == "fixed" and "default" not in parameter:
            errors.append(
                f"{interface_label}: $.parameters[{index}]: fixed parameter requires a default"
            )
        constraints = parameter.get("constraints")
        if constraints:
            type_info = parameter_types[parameter["id"]]
            if type_info is not None and not (
                type_info[0] == "primitive" and type_info[1] in NUMERIC_TYPES
            ):
                errors.append(
                    f"{interface_label}: $.parameters[{index}].constraints require a numeric type"
                )
            if (
                "minimum" in constraints
                and "maximum" in constraints
                and constraints["minimum"] > constraints["maximum"]
            ):
                errors.append(
                    f"{interface_label}: $.parameters[{index}].constraints minimum exceeds maximum"
                )

    for index, dimension in enumerate(interface["dimensions"]):
        extent = dimension["extent"]
        if extent["kind"] != "parameter":
            continue
        parameter_id = extent["parameter"]
        parameter = parameters.get(parameter_id)
        if parameter is None:
            errors.append(
                f"{interface_label}: $.dimensions[{index}].extent.parameter: unknown parameter {parameter_id!r}"
            )
            continue
        if parameter["shape"]["kind"] != "scalar":
            errors.append(
                f"{interface_label}: $.dimensions[{index}]: dimension parameter {parameter_id!r} must be scalar"
            )
        if parameter_types.get(parameter_id) != ("primitive", "integer"):
            errors.append(
                f"{interface_label}: $.dimensions[{index}]: dimension parameter {parameter_id!r} must resolve to Integer"
            )

    for index, connector in enumerate(interface["connectors"]):
        _type_info(
            connector["type"],
            types,
            f"{interface_label}: $.connectors[{index}].type",
            errors,
        )
        _check_shape(
            connector["shape"],
            dimensions,
            f"{interface_label}: $.connectors[{index}].shape",
            errors,
        )
        if connector["presence"]["kind"] == "when":
            _check_guard(
                connector["presence"]["guard"],
                parameters,
                parameter_types,
                types,
                enum_members,
                f"{interface_label}: $.connectors[{index}].presence.guard",
                errors,
            )

    assignment_indexes = _duplicates(
        specialization["parameters"],
        "parameter",
        f"{specialization_label}: $.parameters",
        errors,
    )
    assignments = {
        name: specialization["parameters"][index]["value"]
        for name, index in assignment_indexes.items()
    }
    for index, assignment in enumerate(specialization["parameters"]):
        parameter_id = assignment["parameter"]
        parameter = parameters.get(parameter_id)
        if parameter is None:
            errors.append(
                f"{specialization_label}: $.parameters[{index}].parameter: unknown parameter {parameter_id!r}"
            )
        elif parameter["configurability"] == "fixed":
            errors.append(
                f"{specialization_label}: $.parameters[{index}]: fixed parameter {parameter_id!r} cannot be overridden"
            )

    for index, parameter in enumerate(interface["parameters"]):
        if (
            parameter["configurability"] == "configurable"
            and "default" not in parameter
            and parameter["id"] not in assignments
        ):
            errors.append(
                f"{specialization_label}: configurable parameter {parameter['id']!r} requires a value"
            )

    effective_values = {}
    for parameter_id, parameter in parameters.items():
        if parameter_id in assignments:
            effective_values[parameter_id] = assignments[parameter_id]
        elif "default" in parameter:
            effective_values[parameter_id] = parameter["default"]

    concrete_dimensions = {}
    parameter_dimensions = set()
    for dimension_id, dimension in dimensions.items():
        extent = dimension["extent"]
        if extent["kind"] == "fixed":
            concrete_dimensions[dimension_id] = extent["value"]
            continue
        parameter_dimensions.add(dimension_id)
        parameter_id = extent["parameter"]
        value = effective_values.get(parameter_id)
        if isinstance(value, int) and not isinstance(value, bool) and value > 0:
            concrete_dimensions[dimension_id] = value
        elif parameter_id in parameters:
            errors.append(
                f"{specialization_label}: dimension {dimension_id!r} requires positive Integer parameter {parameter_id!r}"
            )

    for index, parameter in enumerate(interface["parameters"]):
        type_info = parameter_types.get(parameter["id"])
        if "default" in parameter:
            _check_value(
                parameter["default"],
                type_info,
                parameter["shape"],
                concrete_dimensions,
                enum_members,
                parameter.get("constraints"),
                f"{interface_label}: $.parameters[{index}].default",
                errors,
            )
    for index, assignment in enumerate(specialization["parameters"]):
        parameter = parameters.get(assignment["parameter"])
        if parameter is None:
            continue
        _check_value(
            assignment["value"],
            parameter_types.get(parameter["id"]),
            parameter["shape"],
            concrete_dimensions,
            enum_members,
            parameter.get("constraints"),
            f"{specialization_label}: $.parameters[{index}].value",
            errors,
        )

    member_indexes = _duplicates(
        specialization["members"],
        "dimension",
        f"{specialization_label}: $.members",
        errors,
    )
    member_dimensions = set(member_indexes)
    for missing in sorted(parameter_dimensions - member_dimensions):
        errors.append(
            f"{specialization_label}: parameter-driven dimension {missing!r} requires stable members"
        )
    all_member_ids = {}
    for index, record in enumerate(specialization["members"]):
        dimension_id = record["dimension"]
        dimension = dimensions.get(dimension_id)
        if dimension is None:
            errors.append(
                f"{specialization_label}: $.members[{index}].dimension: unknown dimension {dimension_id!r}"
            )
        elif dimension["extent"]["kind"] != "parameter":
            errors.append(
                f"{specialization_label}: $.members[{index}]: dimension {dimension_id!r} is not parameter-driven"
            )
        expected = concrete_dimensions.get(dimension_id)
        if expected is not None and len(record["members"]) != expected:
            errors.append(
                f"{specialization_label}: $.members[{index}].members: expected {expected} members, found {len(record['members'])}"
            )
        for member_index, member_id in enumerate(record["members"]):
            if member_id in all_member_ids:
                errors.append(
                    f"{specialization_label}: $.members[{index}].members[{member_index}]: duplicate stable member {member_id!r}"
                )
            else:
                all_member_ids[member_id] = (index, member_index)


def _check_cross_document(manifest, interface, specialization, errors):
    manifest_label = (FIXTURE_ROOT / "class-manifest.json").as_posix()
    interface_label = (FIXTURE_ROOT / "interface.json").as_posix()
    specialization_label = (FIXTURE_ROOT / "specialization.json").as_posix()
    if manifest["id"] != interface["canonical_id"]:
        errors.append(
            f"{interface_label}: $.canonical_id must equal {manifest_label} $.id"
        )
    if manifest["id"] != specialization["canonical_id"]:
        errors.append(
            f"{specialization_label}: $.canonical_id must equal {manifest_label} $.id"
        )
    if manifest["revision"] != interface["revision"]:
        errors.append(f"{interface_label}: $.revision must equal class manifest revision")
    if manifest["revision"] != specialization["revision"]:
        errors.append(
            f"{specialization_label}: $.revision must equal class manifest revision"
        )


def _check_fixture_placement(repo_root, errors):
    g36_root = repo_root / G36_ROOT
    if not g36_root.is_dir():
        return
    for name in FIXTURE_SCHEMAS:
        for path in sorted(g36_root.rglob(name)):
            relative_path = path.relative_to(repo_root).as_posix()
            errors.append(
                f"{relative_path}: schema-only fixture artifact is forbidden below routines/g36"
            )


def validate(repo_root=REPO_ROOT):
    """Return deterministic routine-schema and synthetic-fixture errors."""
    repo_root = Path(repo_root)
    errors = _dependency_errors()
    schemas_by_id, registry = _load_schemas(repo_root, errors)
    _check_fixture_placement(repo_root, errors)
    if schemas_by_id is None or registry is None:
        return sorted(errors)
    fixtures = _load_fixtures(repo_root, schemas_by_id, registry, errors)
    if fixtures is None:
        return sorted(errors)
    manifest = fixtures["class-manifest.json"]
    interface = fixtures["interface.json"]
    specialization = fixtures["specialization.json"]
    _check_manifest(manifest, errors)
    _check_interface_and_specialization(interface, specialization, errors)
    _check_cross_document(manifest, interface, specialization, errors)
    return sorted(errors)


def main(repo_root=REPO_ROOT, argv=None):
    args = [] if argv is None else list(argv)
    if args:
        print("usage: routine_schemas.py")
        return 2
    errors = validate(repo_root)
    if errors:
        print("\n".join(errors))
        return 1
    print("routine schema lint: 4 schemas, 3 synthetic fixtures OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(argv=sys.argv[1:]))
