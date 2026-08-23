#!/usr/bin/env python3
"""Validate the non-executable routine catalog boundary."""

import json
import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
REGISTRY_PATH = Path("routines/registry.json")
GENERATED_REGISTRY_PATH = Path("routines/generated-registry.json")
SCOPE_PATH = Path("routines/g36/scope.json")
COVERAGE_PATH = Path("routines/g36/coverage.json")
SOURCE_RELEASE_PIN_PATH = Path("routines/g36/SOURCE_RELEASE_PIN")
SOURCE_DEVELOPMENT_PIN_PATH = Path("routines/g36/SOURCE_DEVELOPMENT_PIN")
LEGACY_PIN_PATHS = (
    Path("routines/g36/DONOR_PIN"),
    Path("routines/g36/SOURCE_PIN"),
)
LEGACY_FIXED_PATH = Path("routines/g36/generic/air-economizer-high-limits")

PROFILE = "ASHRAE Guideline 36-2021 Section 5"
REGISTRY_SCHEMA = "cxf-library/routine-registry/v2"
GENERATED_REGISTRY_SCHEMA = "cxf-library/generated-routine-registry/v1"
SCOPE_SCHEMA = "cxf-library/g36-scope/v1"
COVERAGE_SCHEMA = "cxf-library/g36-coverage/v2"

REGISTRY_KEYS = frozenset(("schema", "routines"))
GENERATED_REGISTRY_KEYS = frozenset(("schema", "deployments"))
SCOPE_KEYS = frozenset(("schema", "profile", "status", "sections"))
SCOPE_ROW_KEYS = frozenset(
    ("id", "section", "name", "status", "source_disposition", "destination")
)
COVERAGE_KEYS = frozenset(("schema", "profile", "status", "scope", "claims"))

SCOPE_ROWS = (
    ("5.1", "g36/shared/general", "mixed"),
    ("5.2", "g36/zones/ventilation", "upstream-partial"),
    ("5.3", "g36/zones/thermal", "upstream-broad"),
    ("5.4", "g36/zone-groups", "upstream-broad"),
    ("5.5", "g36/terminal-units/cooling-only", "upstream-broad"),
    ("5.6", "g36/terminal-units/reheat", "upstream-broad"),
    ("5.7", "g36/terminal-units/parallel-fan-cv", "upstream-broad"),
    ("5.8", "g36/terminal-units/parallel-fan-vv", "upstream-broad"),
    ("5.9", "g36/terminal-units/series-fan-cv", "upstream-broad"),
    ("5.10", "g36/terminal-units/series-fan-vv", "upstream-broad"),
    ("5.11", "g36/terminal-units/dual-duct-snap", "upstream-broad"),
    ("5.12", "g36/terminal-units/dual-duct-mix-inlet", "upstream-broad"),
    ("5.13", "g36/terminal-units/dual-duct-mix-discharge", "upstream-broad"),
    ("5.14", "g36/terminal-units/dual-duct-cold-min", "upstream-broad"),
    ("5.15", "g36/ahus/system-modes", "upstream-embedded"),
    ("5.16", "g36/ahus/multizone-vav", "upstream-broad"),
    ("5.17", "g36/ahus/dual-fan-dual-duct", "independent-authoring"),
    ("5.18", "g36/ahus/single-zone-vav", "upstream-broad"),
    ("5.19", "g36/exhaust-fans/constant-speed", "independent-authoring"),
    ("5.20", "g36/plants/chilled-water", "development-source"),
    ("5.21", "g36/plants/hot-water", "independent-authoring"),
    ("5.22", "g36/fan-coil-units", "upstream-partial"),
)
EXPECTED_SECTIONS = tuple(row[0] for row in SCOPE_ROWS)
SCOPE_CONTRACT = {
    section: {
        "id": f"G36-SCOPE-05-{index:02d}",
        "destination": destination,
        "source_disposition": source_disposition,
    }
    for index, (section, destination, source_disposition) in enumerate(SCOPE_ROWS, 1)
}

PIN_RE = re.compile(r"[0-9a-f]{40}\n")


def _read_json(repo_root, relative_path, errors):
    label = relative_path.as_posix()
    try:
        text = (repo_root / relative_path).read_text(encoding="utf-8")
    except FileNotFoundError:
        errors.append(f"{label}: file is missing")
        return None
    except (OSError, UnicodeError):
        errors.append(f"{label}: unable to read file")
        return None
    try:
        value = json.loads(text)
    except json.JSONDecodeError as exc:
        errors.append(f"{label}: invalid JSON at line {exc.lineno}, column {exc.colno}")
        return None
    if not isinstance(value, dict):
        errors.append(f"{label}: must contain a JSON object")
        return None
    return value


def _read_pin(repo_root, relative_path, errors):
    label = relative_path.as_posix()
    try:
        value = (repo_root / relative_path).read_text(encoding="utf-8")
    except FileNotFoundError:
        errors.append(f"{label}: file is missing")
        return None
    except (OSError, UnicodeError):
        errors.append(f"{label}: unable to read file")
        return None
    if PIN_RE.fullmatch(value) is None:
        errors.append(
            f"{label}: must contain one lowercase 40-hex Git commit followed by a newline"
        )
        return None
    return value.rstrip("\n")


def _check_exact_keys(value, expected, label, errors):
    actual = set(value)
    if actual == expected:
        return
    details = []
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    if missing:
        details.append(f"missing {', '.join(missing)}")
    if unexpected:
        details.append(f"unexpected {', '.join(unexpected)}")
    errors.append(
        f"{label}: keys must be exactly {', '.join(sorted(expected))} "
        f"({'; '.join(details)})"
    )


def _validate_empty_registry(value, path, expected_keys, schema, array_key, errors):
    if value is None:
        return None
    label = path.as_posix()
    _check_exact_keys(value, expected_keys, label, errors)
    if value.get("schema") != schema:
        errors.append(f"{label}: schema must be {schema!r}")
    rows = value.get(array_key)
    if not isinstance(rows, list):
        errors.append(f"{label}: {array_key} must be an array")
        return None
    if rows:
        errors.append(
            f"{label}: {array_key} must remain empty until production catalog rows are implemented"
        )
    return len(rows)


def _relative_path_problem(value):
    if not isinstance(value, str):
        return "must be a string"
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
    if len(segments) < 2 or segments[0] != "g36":
        return "path must be below g36/"
    return None


def _record_unique(value, key, index, seen, errors):
    label = SCOPE_PATH.as_posix()
    if not isinstance(value, str):
        errors.append(f"{label}: sections[{index}].{key} must be a string")
        return
    if value in seen:
        errors.append(
            f"{label}: sections[{index}].{key}: duplicate {value!r}; "
            f"first used by sections[{seen[value]}]"
        )
    else:
        seen[value] = index


def _validate_scope(scope, errors):
    if scope is None:
        return
    label = SCOPE_PATH.as_posix()
    _check_exact_keys(scope, SCOPE_KEYS, label, errors)
    if scope.get("schema") != SCOPE_SCHEMA:
        errors.append(f"{label}: schema must be {SCOPE_SCHEMA!r}")
    if scope.get("profile") != PROFILE:
        errors.append(f"{label}: profile must be {PROFILE!r}")
    if scope.get("status") != "planned":
        errors.append(f"{label}: status must be 'planned'")

    rows = scope.get("sections")
    if not isinstance(rows, list):
        errors.append(f"{label}: sections must be an array")
        return
    if len(rows) != len(SCOPE_ROWS):
        errors.append(f"{label}: sections must contain exactly 22 rows")

    seen = {key: {} for key in ("id", "section", "destination")}
    actual_sections = []
    for index, row in enumerate(rows):
        row_label = f"{label}: sections[{index}]"
        if not isinstance(row, dict):
            errors.append(f"{row_label} must be an object")
            continue
        _check_exact_keys(row, SCOPE_ROW_KEYS, row_label, errors)
        for key in seen:
            _record_unique(row.get(key), key, index, seen[key], errors)

        section = row.get("section")
        if isinstance(section, str):
            actual_sections.append(section)
        name = row.get("name")
        if (
            not isinstance(name, str)
            or not name
            or name != name.strip()
            or any(ord(character) < 32 for character in name)
        ):
            errors.append(f"{row_label}.name must be a nonempty trimmed string")
        if row.get("status") != "planned":
            errors.append(f"{row_label}.status must be 'planned'")

        destination = row.get("destination")
        problem = _relative_path_problem(destination)
        if problem:
            errors.append(f"{row_label}.destination: {problem}")

        expected = SCOPE_CONTRACT.get(section) if isinstance(section, str) else None
        if expected is None:
            continue
        for key in ("id", "source_disposition", "destination"):
            if row.get(key) != expected[key]:
                errors.append(f"{row_label}.{key} must be {expected[key]!r}")

    if set(actual_sections) != set(EXPECTED_SECTIONS):
        errors.append(f"{label}: sections must contain exactly sections 5.1 through 5.22")
    if actual_sections != list(EXPECTED_SECTIONS):
        errors.append(f"{label}: sections must be ordered from 5.1 through 5.22")


def _validate_coverage(coverage, scope, errors):
    if coverage is None:
        return
    label = COVERAGE_PATH.as_posix()
    _check_exact_keys(coverage, COVERAGE_KEYS, label, errors)
    if coverage.get("schema") != COVERAGE_SCHEMA:
        errors.append(f"{label}: schema must be {COVERAGE_SCHEMA!r}")
    if coverage.get("profile") != PROFILE:
        errors.append(f"{label}: profile must be {PROFILE!r}")
    if coverage.get("status") != "planned":
        errors.append(f"{label}: status must be 'planned'")
    if coverage.get("scope") != "scope.json":
        errors.append(f"{label}: scope must be 'scope.json'")
    if isinstance(scope, dict):
        if coverage.get("profile") != scope.get("profile"):
            errors.append(f"{label}: profile must equal scope.json profile")
        if coverage.get("status") != scope.get("status"):
            errors.append(f"{label}: status must equal scope.json status")
    claims = coverage.get("claims")
    if not isinstance(claims, list):
        errors.append(f"{label}: claims must be an array")
    elif claims:
        errors.append(
            f"{label}: claims must remain empty until coverage claims are implemented"
        )


def _validate_stale_artifacts(repo_root, errors):
    for relative_path in LEGACY_PIN_PATHS:
        if (repo_root / relative_path).exists():
            errors.append(f"{relative_path.as_posix()}: legacy pin must be absent")
    if (repo_root / LEGACY_FIXED_PATH).exists():
        errors.append(
            f"{LEGACY_FIXED_PATH.as_posix()}: legacy fixed-variant path must be absent"
        )
    g36_root = repo_root / "routines/g36"
    if g36_root.is_dir():
        for graph_path in sorted(g36_root.rglob("routine.cxf.jsonld")):
            relative_path = graph_path.relative_to(repo_root).as_posix()
            errors.append(
                f"{relative_path}: executable routine artifacts are forbidden until generated deployments are implemented"
            )


def _validate(repo_root):
    repo_root = Path(repo_root)
    errors = []
    registry = _read_json(repo_root, REGISTRY_PATH, errors)
    generated_registry = _read_json(repo_root, GENERATED_REGISTRY_PATH, errors)
    scope = _read_json(repo_root, SCOPE_PATH, errors)
    coverage = _read_json(repo_root, COVERAGE_PATH, errors)

    canonical_count = _validate_empty_registry(
        registry,
        REGISTRY_PATH,
        REGISTRY_KEYS,
        REGISTRY_SCHEMA,
        "routines",
        errors,
    )
    generated_count = _validate_empty_registry(
        generated_registry,
        GENERATED_REGISTRY_PATH,
        GENERATED_REGISTRY_KEYS,
        GENERATED_REGISTRY_SCHEMA,
        "deployments",
        errors,
    )
    _validate_scope(scope, errors)
    _validate_coverage(coverage, scope, errors)

    release_pin = _read_pin(repo_root, SOURCE_RELEASE_PIN_PATH, errors)
    development_pin = _read_pin(repo_root, SOURCE_DEVELOPMENT_PIN_PATH, errors)
    if release_pin is not None and release_pin == development_pin:
        errors.append("source release and development pins must be distinct")
    _validate_stale_artifacts(repo_root, errors)

    scope_count = 0
    if isinstance(scope, dict) and isinstance(scope.get("sections"), list):
        scope_count = len(scope["sections"])
    return (
        sorted(errors),
        scope_count,
        canonical_count if canonical_count is not None else 0,
        generated_count if generated_count is not None else 0,
    )


def validate(repo_root=REPO_ROOT):
    """Return catalog errors in deterministic order."""
    return _validate(repo_root)[0]


def main(repo_root=REPO_ROOT, argv=None):
    args = [] if argv is None else list(argv)
    if args:
        print("usage: routines.py")
        return 2
    errors, scope_count, canonical_count, generated_count = _validate(repo_root)
    if errors:
        print("\n".join(errors))
        return 1
    print(
        f"routine catalog lint: {scope_count} planned scope anchors, "
        f"{canonical_count} canonical routines, "
        f"{generated_count} generated deployments OK"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(argv=sys.argv[1:]))
