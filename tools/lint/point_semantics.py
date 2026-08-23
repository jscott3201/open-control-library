#!/usr/bin/env python3
"""Validate point dictionaries against the repository's pinned semantic contract."""

import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools.point_resolution import (  # noqa: E402
    POINT_SCHEMA_V1,
    POINT_SCHEMA_V2,
    POINT_SCHEMAS,
    load_point_corpus,
    read_json_object,
)


POINTS_ROOT = Path("points")
PINS_PATH = Path("routines/ontology/ontology-pins.json")

POINT_SCHEMA = POINT_SCHEMA_V1
PINS_SCHEMA = "cxf-library/ontology-pins/v1"

NAMESPACE_REQUIRED_KEYS = frozenset(("brick", "s223", "quantitykind", "unit"))
NAMESPACE_OPTIONAL_KEYS = frozenset(("s223_g36",))
NAMESPACE_RECORD_KEYS = frozenset(("iri", "verified_version"))
POINT_REQUIRED_KEYS = frozenset(
    ("name", "description", "kind", "unit", "qudt_unit", "brick", "s223")
)
POINT_OPTIONAL_KEYS = frozenset(("notes", "derived", "provisional"))
S223_REQUIRED_KEYS = frozenset(
    ("pattern", "property_class", "quantitykind", "unit", "medium", "aspects")
)
S223_OPTIONAL_KEYS = frozenset(("enumerationkind",))

PROPERTY_CLASSES = frozenset(
    (
        "QuantifiableObservableProperty",
        "QuantifiableActuatableProperty",
        "QuantifiableProperty",
        "EnumeratedObservableProperty",
        "EnumeratedActuatableProperty",
        "EnumerableProperty",
    )
)
ACTUATABLE_CLASS_BY_KIND = {
    "real": "QuantifiableActuatableProperty",
    "int": "QuantifiableActuatableProperty",
    "bool": "EnumeratedActuatableProperty",
}

NAME_RE = re.compile(r"^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$")
ACTUATABLE_NAME_RE = re.compile(r"(?:_cmd$|_sp(?:_|$))")

# This table is the reviewed VAV/SYS/zone audit boundary, not a general ontology term catalog.
REVIEWED_MAPPING_EXPECTATIONS = {
    ("points/sys.points.json", "oat"): {
        "qudt_unit": "DEG_C",
        "property_class": "QuantifiableObservableProperty",
        "quantitykind": "Temperature",
        "unit": "DEG_C",
        "medium": "Fluid-Air",
        "aspects": [],
    },
    ("points/sys.points.json", "rht_vlv_cmd"): {
        "qudt_unit": "PERCENT",
        "property_class": "QuantifiableActuatableProperty",
        "quantitykind": "DimensionlessRatio",
        "unit": "PERCENT",
        "medium": None,
        "aspects": ["Binary-Position"],
    },
    ("points/vav.points.json", "rht_vlv_cmd"): {
        "qudt_unit": "PERCENT",
        "property_class": "QuantifiableActuatableProperty",
        "quantitykind": "DimensionlessRatio",
        "unit": "PERCENT",
        "medium": None,
        "aspects": ["Binary-Position"],
    },
    ("points/vav.points.json", "vav_dat"): {
        "qudt_unit": "DEG_C",
        "property_class": "QuantifiableObservableProperty",
        "quantitykind": "Temperature",
        "unit": "DEG_C",
        "medium": "Fluid-Air",
        "aspects": [],
    },
    ("points/vav.points.json", "zone_airflow"): {
        "qudt_unit": "L-PER-SEC",
        "property_class": "QuantifiableObservableProperty",
        "quantitykind": "VolumeFlowRate",
        "unit": "L-PER-SEC",
        "medium": "Fluid-Air",
        "aspects": [],
    },
    ("points/vav.points.json", "zone_airflow_sp"): {
        "qudt_unit": "L-PER-SEC",
        "property_class": "QuantifiableActuatableProperty",
        "quantitykind": "VolumeFlowRate",
        "unit": "L-PER-SEC",
        "medium": "Fluid-Air",
        "aspects": ["Aspect-Setpoint"],
    },
    ("points/vav.points.json", "zone_airflow_sp_min"): {
        "qudt_unit": "L-PER-SEC",
        "property_class": "QuantifiableActuatableProperty",
        "quantitykind": "VolumeFlowRate",
        "unit": "L-PER-SEC",
        "medium": "Fluid-Air",
        "aspects": ["Aspect-Setpoint"],
    },
    ("points/vav.points.json", "zone_clg_request"): {
        "qudt_unit": "PERCENT",
        "property_class": "QuantifiableObservableProperty",
        "quantitykind": "DimensionlessRatio",
        "unit": "PERCENT",
        "medium": None,
        "aspects": [],
    },
    ("points/vav.points.json", "zone_dmpr_pos"): {
        "qudt_unit": "PERCENT",
        "property_class": "QuantifiableObservableProperty",
        "quantitykind": "DimensionlessRatio",
        "unit": "PERCENT",
        "medium": None,
        "aspects": [],
    },
    ("points/zone.points.json", "zone_temp"): {
        "qudt_unit": "DEG_C",
        "property_class": "QuantifiableObservableProperty",
        "quantitykind": "Temperature",
        "unit": "DEG_C",
        "medium": "Fluid-Air",
        "aspects": [],
    },
    ("points/zone.points.json", "zone_temp_sp_clg"): {
        "qudt_unit": "DEG_C",
        "property_class": "QuantifiableActuatableProperty",
        "quantitykind": "Temperature",
        "unit": "DEG_C",
        "medium": "Fluid-Air",
        "aspects": ["Aspect-Setpoint"],
    },
    ("points/zone.points.json", "zone_temp_sp_htg"): {
        "qudt_unit": "DEG_C",
        "property_class": "QuantifiableActuatableProperty",
        "quantitykind": "Temperature",
        "unit": "DEG_C",
        "medium": "Fluid-Air",
        "aspects": ["Aspect-Setpoint"],
    },
    ("points/zone.points.json", "occ_sensor"): {
        "qudt_unit": None,
        "property_class": "EnumeratedObservableProperty",
        "quantitykind": None,
        "unit": None,
        "medium": None,
        "aspects": [],
        "enumerationkind": "Binary-OccupiedUnoccupied",
    },
}


def _read_json(repo_root, relative_path, errors):
    return read_json_object(repo_root, relative_path, errors)


def _check_keys(value, required, optional, label, errors):
    actual = set(value)
    for key in sorted(required - actual):
        errors.append(f"{label}: missing required key {key!r}")
    for key in sorted(actual - required - optional):
        errors.append(f"{label}: unexpected key {key!r}")


def _nonempty_string(value):
    return isinstance(value, str) and bool(value.strip())


def _optional_string(value):
    return value is None or _nonempty_string(value)


def _pin_field(pins: dict, path: tuple[str, ...], errors: list[str]) -> str | None:
    value = pins
    for key in path:
        if not isinstance(value, dict) or key not in value:
            errors.append(f"{PINS_PATH.as_posix()}: missing pin field $.{'.'.join(path)}")
            return None
        value = value[key]
    if not isinstance(value, str) or not value.strip():
        errors.append(
            f"{PINS_PATH.as_posix()}: pin field $.{'.'.join(path)} must be a nonempty string"
        )
        return None
    return value


def _load_namespace_expectations(repo_root, errors):
    pins = _read_json(repo_root, PINS_PATH, errors)
    if pins is None:
        return {}
    if pins.get("schema") != PINS_SCHEMA:
        errors.append(f"{PINS_PATH.as_posix()}: schema must be {PINS_SCHEMA!r}")

    brick_namespace = _pin_field(pins, ("brick", "namespace"), errors)
    brick_release = _pin_field(pins, ("brick", "release"), errors)
    s223_namespace = _pin_field(
        pins, ("s223_compatibility", "core_namespace"), errors
    )
    s223_g36_namespace = _pin_field(
        pins, ("s223_compatibility", "g36_extension_namespace"), errors
    )
    s223_version = _pin_field(pins, ("s223_compatibility", "version"), errors)
    quantitykind_namespace = _pin_field(
        pins, ("qudt", "quantitykind_namespace"), errors
    )
    unit_namespace = _pin_field(pins, ("qudt", "unit_namespace"), errors)
    qudt_release = _pin_field(pins, ("qudt", "release"), errors)

    brick_version = (
        brick_release.removeprefix("v") if brick_release is not None else None
    )
    qudt_version = (
        f"QUDT {qudt_release.removeprefix('v')}"
        if qudt_release is not None
        else None
    )
    values = {
        "brick": (brick_namespace, brick_version),
        "s223": (s223_namespace, s223_version),
        "s223_g36": (s223_g36_namespace, s223_version),
        "quantitykind": (quantitykind_namespace, qudt_version),
        "unit": (unit_namespace, qudt_version),
    }
    return {
        key: value
        for key, value in values.items()
        if value[0] is not None and value[1] is not None
    }


def _validate_namespaces(namespaces, label, expected, errors):
    if not isinstance(namespaces, dict):
        errors.append(f"{label}: namespaces must be an object")
        return
    namespace_label = f"{label}: namespaces"
    _check_keys(
        namespaces,
        NAMESPACE_REQUIRED_KEYS,
        NAMESPACE_OPTIONAL_KEYS,
        namespace_label,
        errors,
    )
    for name in sorted(NAMESPACE_REQUIRED_KEYS | NAMESPACE_OPTIONAL_KEYS):
        if name not in namespaces:
            continue
        record = namespaces[name]
        record_label = f"{namespace_label}.{name}"
        if not isinstance(record, dict):
            errors.append(f"{record_label}: must be an object")
            continue
        _check_keys(record, NAMESPACE_RECORD_KEYS, frozenset(), record_label, errors)
        for field in ("iri", "verified_version"):
            if field in record and not _nonempty_string(record[field]):
                errors.append(f"{record_label}.{field}: must be a nonempty string")
        if name not in expected:
            continue
        expected_iri, expected_version = expected[name]
        if record.get("iri") != expected_iri:
            errors.append(
                f"{record_label}.iri: must equal ontology pin {expected_iri!r}"
            )
        if record.get("verified_version") != expected_version:
            errors.append(
                f"{record_label}.verified_version: must equal ontology pin echo "
                f"{expected_version!r}"
            )


def _validate_s223(value, point, label, errors):
    if value is None:
        return None
    if not isinstance(value, dict):
        errors.append(f"{label}.s223: must be an object or null")
        return None
    _check_keys(
        value,
        S223_REQUIRED_KEYS,
        S223_OPTIONAL_KEYS,
        f"{label}.s223",
        errors,
    )
    if "pattern" in value and not _nonempty_string(value["pattern"]):
        errors.append(f"{label}.s223.pattern: must be a nonempty string")

    property_class = value.get("property_class")
    if property_class == "EnumeratedProperty":
        errors.append(f"{label}.s223.property_class: EnumeratedProperty is forbidden")
    elif not isinstance(property_class, str) or property_class not in PROPERTY_CLASSES:
        errors.append(
            f"{label}.s223.property_class: must be one of "
            f"{', '.join(sorted(PROPERTY_CLASSES))}"
        )

    for field in ("quantitykind", "unit", "medium"):
        if field in value and not _optional_string(value[field]):
            errors.append(f"{label}.s223.{field}: must be a nonempty string or null")

    aspects = value.get("aspects")
    if not isinstance(aspects, list):
        errors.append(f"{label}.s223.aspects: must be an array")
    else:
        for index, aspect in enumerate(aspects):
            if not _nonempty_string(aspect):
                errors.append(
                    f"{label}.s223.aspects[{index}]: must be a nonempty string"
                )

    if "enumerationkind" in value and not _nonempty_string(value["enumerationkind"]):
        errors.append(f"{label}.s223.enumerationkind: must be a nonempty string")

    if property_class == "QuantifiableActuatableProperty":
        if not _nonempty_string(value.get("quantitykind")):
            errors.append(
                f"{label}.s223.quantitykind: QuantifiableActuatableProperty requires "
                "a nonempty quantitykind"
            )
        if not _nonempty_string(value.get("unit")):
            errors.append(
                f"{label}.s223.unit: QuantifiableActuatableProperty requires a nonempty unit"
            )
        if value.get("unit") != point.get("qudt_unit"):
            errors.append(f"{label}.s223.unit: must equal the point qudt_unit")
    return property_class


def _validate_point(point, index, path, reviewed_points, errors):
    dictionary_label = path.as_posix()
    label = f"{dictionary_label}: points[{index}]"
    if not isinstance(point, dict):
        return
    _check_keys(point, POINT_REQUIRED_KEYS, POINT_OPTIONAL_KEYS, label, errors)

    name = point.get("name")
    if isinstance(name, str) and NAME_RE.fullmatch(name) is not None:
        reviewed_points[(dictionary_label, name)] = (index, point)

    if not _nonempty_string(point.get("description")):
        errors.append(f"{label}.description: must be a nonempty string")
    kind = point.get("kind")
    if not isinstance(kind, str) or kind not in ACTUATABLE_CLASS_BY_KIND:
        errors.append(f"{label}.kind: must be 'real', 'int', or 'bool'")
    if not _nonempty_string(point.get("unit")):
        errors.append(f"{label}.unit: must be a nonempty string")
    if not _optional_string(point.get("qudt_unit")):
        errors.append(f"{label}.qudt_unit: must be a nonempty string or null")
    if not _optional_string(point.get("brick")):
        errors.append(f"{label}.brick: must be a nonempty string or null")
    for field in ("notes",):
        if field in point and not isinstance(point[field], str):
            errors.append(f"{label}.{field}: must be a string")
    for field in ("derived", "provisional"):
        if field in point and not isinstance(point[field], bool):
            errors.append(f"{label}.{field}: must be a boolean")

    property_class = _validate_s223(point.get("s223"), point, label, errors)
    if (
        isinstance(name, str)
        and ACTUATABLE_NAME_RE.search(name)
        and point.get("s223") is not None
        and point.get("derived") is not True
        and isinstance(kind, str)
        and kind in ACTUATABLE_CLASS_BY_KIND
    ):
        expected_class = ACTUATABLE_CLASS_BY_KIND[kind]
        if property_class != expected_class:
            errors.append(
                f"{label}.s223.property_class: {name!r} must use {expected_class}"
            )


def _validate_dictionary(
    value, path, namespace_expectations, reviewed_points, errors
):
    if value is None:
        return
    label = path.as_posix()
    _validate_namespaces(value.get("namespaces"), label, namespace_expectations, errors)

    points = value.get("points")
    if not isinstance(points, list):
        return
    for index, point in enumerate(points):
        _validate_point(point, index, path, reviewed_points, errors)


def _validate_reviewed_mappings(reviewed_points, errors):
    reviewed_count = 0
    for key, expected in sorted(REVIEWED_MAPPING_EXPECTATIONS.items()):
        path, name = key
        label = f"{path}#{name}"
        record = reviewed_points.get(key)
        if record is None:
            errors.append(f"{label}: reviewed mapping is missing")
            continue
        reviewed_count += 1
        _, point = record
        if point.get("qudt_unit") != expected["qudt_unit"]:
            errors.append(
                f"{label}: qudt_unit must be {expected['qudt_unit']!r}, "
                f"found {point.get('qudt_unit')!r}"
            )
        s223 = point.get("s223")
        if not isinstance(s223, dict):
            errors.append(f"{label}: reviewed mapping requires an s223 object")
            continue
        fields = ("property_class", "quantitykind", "unit", "medium", "aspects")
        if "enumerationkind" in expected:
            fields += ("enumerationkind",)
        for field in fields:
            if s223.get(field) != expected[field]:
                errors.append(
                    f"{label}: s223.{field} must be {expected[field]!r}, "
                    f"found {s223.get(field)!r}"
                )
    return reviewed_count


def _validate(repo_root):
    repo_root = Path(repo_root)
    errors = []
    namespace_expectations = _load_namespace_expectations(repo_root, errors)
    corpus = load_point_corpus(repo_root)
    errors.extend(corpus.errors)

    reviewed_points = {}
    for path_string in corpus.paths:
        relative_path = Path(path_string)
        value = corpus.documents.get(path_string)
        _validate_dictionary(
            value, relative_path, namespace_expectations, reviewed_points, errors
        )
    reviewed_count = _validate_reviewed_mappings(reviewed_points, errors)
    return sorted(errors), len(corpus.paths), reviewed_count


def validate(repo_root=REPO_ROOT):
    """Return point dictionary errors in deterministic order."""
    return _validate(repo_root)[0]


def main(repo_root=REPO_ROOT, argv=None):
    args = [] if argv is None else list(argv)
    if args:
        print("usage: point_semantics.py")
        return 2
    errors, dictionary_count, reviewed_count = _validate(repo_root)
    if errors:
        print("\n".join(errors))
        return 1
    print(
        f"point semantic lint: {dictionary_count} dictionaries, "
        f"{reviewed_count} bounded reviewed mappings OK"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(argv=sys.argv[1:]))
