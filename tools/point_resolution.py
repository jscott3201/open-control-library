"""Load and resolve point dictionaries without implicit cross-family lookup."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path


POINTS_ROOT = Path("points")
POINT_SCHEMA_V1 = "cxf-library/points/v1"
POINT_SCHEMA_V2 = "cxf-library/points/v2"
POINT_SCHEMAS = frozenset((POINT_SCHEMA_V1, POINT_SCHEMA_V2))

TOP_LEVEL_REQUIRED_KEYS = {
    POINT_SCHEMA_V1: frozenset(("schema", "equipment", "namespaces", "points")),
    POINT_SCHEMA_V2: frozenset(
        ("schema", "equipment", "namespaces", "imports", "aliases", "points")
    ),
}
TOP_LEVEL_OPTIONAL_KEYS = frozenset(("notes",))
ALIAS_KEYS = frozenset(("name", "target"))

NAME_RE = re.compile(r"^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$")
DICTIONARY_PATH_RE = re.compile(
    r"^points/[a-z][a-z0-9]*(?:_[a-z0-9]+)*\.points\.json$"
)
POINT_REF_RE = re.compile(
    r"^(points/[a-z][a-z0-9]*(?:_[a-z0-9]+)*\.points\.json)"
    r"#([a-z][a-z0-9]*(?:_[a-z0-9]+)*)$"
)


class PointResolutionError(ValueError):
    """A point corpus or reference cannot be resolved under the v1/v2 contract."""


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


def read_json_object(repo_root, relative_path, errors):
    """Read one local UTF-8 JSON object with duplicate and non-finite rejection."""
    relative_path = Path(relative_path)
    label = relative_path.as_posix()
    try:
        raw = (Path(repo_root) / relative_path).read_bytes()
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


def _check_keys(value, required, optional, label, errors):
    actual = set(value)
    for key in sorted(required - actual):
        errors.append(f"{label}: missing required key {key!r}")
    for key in sorted(actual - required - optional):
        errors.append(f"{label}: unexpected key {key!r}")


def _valid_name(value):
    return isinstance(value, str) and NAME_RE.fullmatch(value) is not None


@dataclass(frozen=True)
class ResolvedPoint:
    path: str
    name: str
    record: dict

    @property
    def ref(self):
        return f"{self.path}#{self.name}"


@dataclass(frozen=True)
class PointDictionary:
    path: str
    document: dict
    imports: tuple[str, ...]
    aliases: tuple[tuple[str, str], ...]
    points: dict[str, dict]

    def alias_target(self, name):
        return dict(self.aliases).get(name)


@dataclass(frozen=True)
class PointCorpus:
    paths: tuple[str, ...]
    documents: dict[str, dict]
    dictionaries: dict[str, PointDictionary]
    errors: tuple[str, ...]

    def require_valid(self):
        if self.errors:
            raise PointResolutionError("\n".join(self.errors))
        return self

    def resolve_bare(self, dictionary_path, name):
        self.require_valid()
        path = Path(dictionary_path).as_posix()
        if DICTIONARY_PATH_RE.fullmatch(path) is None:
            raise PointResolutionError(f"malformed dictionary path {path!r}")
        if not _valid_name(name):
            raise PointResolutionError(f"malformed bare point name {name!r}")
        dictionary = self.dictionaries.get(path)
        if dictionary is None:
            raise PointResolutionError(f"point dictionary {path!r} is missing")
        if name in dictionary.points:
            return ResolvedPoint(path, name, dictionary.points[name])
        target = dictionary.alias_target(name)
        if target is not None:
            return self._resolve_alias(path, name, target)
        raise PointResolutionError(f"{path} has no local point or alias {name!r}")

    def resolve_ref(self, ref):
        self.require_valid()
        if not isinstance(ref, str):
            raise PointResolutionError(f"malformed point reference {ref!r}")
        match = POINT_REF_RE.fullmatch(ref)
        if match is None:
            raise PointResolutionError(f"malformed point reference {ref!r}")
        path, name = match.groups()
        return self.resolve_bare(path, name)

    def _resolve_alias(self, source_path, alias_name, target):
        match = POINT_REF_RE.fullmatch(target)
        if match is None:
            raise PointResolutionError(
                f"{source_path} alias {alias_name!r} has malformed target {target!r}"
            )
        target_path, target_name = match.groups()
        dictionary = self.dictionaries.get(target_path)
        if dictionary is None or target_name not in dictionary.points:
            raise PointResolutionError(
                f"{source_path} alias {alias_name!r} does not target a concrete point"
            )
        return ResolvedPoint(target_path, target_name, dictionary.points[target_name])


def _dictionary_identity(value, path, errors):
    label = path.as_posix()
    expected_equipment = path.name.removesuffix(".points.json")
    equipment = value.get("equipment")
    if not _valid_name(equipment):
        errors.append(f"{label}: equipment must be a lower-case snake_case identifier")
    elif equipment != expected_equipment:
        errors.append(
            f"{label}: equipment must match filename stem {expected_equipment!r}"
        )
    if "notes" in value and not isinstance(value["notes"], str):
        errors.append(f"{label}: notes must be a string")


def _local_points(value, path, errors):
    label = path.as_posix()
    points = value.get("points")
    if not isinstance(points, list):
        errors.append(f"{label}: points must be an array")
        return {}
    records = {}
    first_indices = {}
    for index, point in enumerate(points):
        point_label = f"{label}: points[{index}]"
        if not isinstance(point, dict):
            errors.append(f"{point_label}: must be an object")
            continue
        name = point.get("name")
        if not _valid_name(name):
            errors.append(
                f"{point_label}.name: must be a lower-case snake_case identifier"
            )
            continue
        if name in first_indices:
            errors.append(
                f"{point_label}.name: duplicate {name!r}; first used at index "
                f"{first_indices[name]}"
            )
            continue
        first_indices[name] = index
        records[name] = point
    return records


def _imports(value, path, errors):
    label = path.as_posix()
    imports = value.get("imports")
    if not isinstance(imports, list):
        errors.append(f"{label}: imports must be an array")
        return ()
    valid = []
    first_indices = {}
    for index, target in enumerate(imports):
        import_label = f"{label}: imports[{index}]"
        if not isinstance(target, str) or DICTIONARY_PATH_RE.fullmatch(target) is None:
            errors.append(
                f"{import_label}: must be a root-relative points/<family>.points.json path"
            )
            continue
        if target in first_indices:
            errors.append(
                f"{import_label}: duplicate {target!r}; first used at index "
                f"{first_indices[target]}"
            )
            continue
        first_indices[target] = index
        if target == label:
            errors.append(f"{import_label}: self-import is forbidden")
            continue
        valid.append(target)
    return tuple(valid)


def _aliases(value, path, local_points, errors):
    label = path.as_posix()
    aliases = value.get("aliases")
    if not isinstance(aliases, list):
        errors.append(f"{label}: aliases must be an array")
        return ()
    valid = []
    first_indices = {}
    for index, alias in enumerate(aliases):
        alias_label = f"{label}: aliases[{index}]"
        if not isinstance(alias, dict):
            errors.append(f"{alias_label}: must be an object")
            continue
        _check_keys(alias, ALIAS_KEYS, frozenset(), alias_label, errors)
        name = alias.get("name")
        target = alias.get("target")
        name_valid = _valid_name(name)
        if not name_valid:
            errors.append(
                f"{alias_label}.name: must be a lower-case snake_case identifier"
            )
        elif name in first_indices:
            errors.append(
                f"{alias_label}.name: duplicate {name!r}; first used at index "
                f"{first_indices[name]}"
            )
            name_valid = False
        else:
            first_indices[name] = index
            if name in local_points:
                errors.append(f"{alias_label}.name: collides with local point {name!r}")
                name_valid = False
        target_valid = isinstance(target, str) and POINT_REF_RE.fullmatch(target) is not None
        if not target_valid:
            errors.append(
                f"{alias_label}.target: must be points/<family>.points.json#<name>"
            )
        if name_valid and target_valid:
            valid.append((name, target))
    return tuple(valid)


def _cycle_errors(dictionaries):
    state = {}
    stack = []
    cycles = set()

    def visit(path):
        state[path] = 1
        stack.append(path)
        for target in sorted(dictionaries[path].imports):
            if target not in dictionaries:
                continue
            if state.get(target, 0) == 0:
                visit(target)
            elif state.get(target) == 1:
                start = stack.index(target)
                members = stack[start:]
                rotations = [
                    tuple(members[index:] + members[:index])
                    for index in range(len(members))
                ]
                cycles.add(min(rotations))
        stack.pop()
        state[path] = 2

    for path in sorted(dictionaries):
        if state.get(path, 0) == 0:
            visit(path)
    return [f"points: import cycle: {' -> '.join(cycle + (cycle[0],))}" for cycle in sorted(cycles)]


def load_point_corpus(repo_root):
    """Load every point dictionary and collect deterministic contract errors."""
    repo_root = Path(repo_root)
    errors = []
    try:
        absolute_paths = sorted((repo_root / POINTS_ROOT).glob("*.points.json"))
    except OSError:
        errors.append(f"{POINTS_ROOT.as_posix()}: unable to discover point dictionaries")
        absolute_paths = []
    if not absolute_paths:
        errors.append(f"{POINTS_ROOT.as_posix()}: no *.points.json dictionaries found")

    paths = tuple(path.relative_to(repo_root).as_posix() for path in absolute_paths)
    documents = {}
    dictionaries = {}
    for absolute_path, path_string in zip(absolute_paths, paths):
        path = Path(path_string)
        value = read_json_object(repo_root, path, errors)
        if value is None:
            continue
        documents[path_string] = value
        schema = value.get("schema")
        if not isinstance(schema, str) or schema not in POINT_SCHEMAS:
            if "schema" not in value:
                errors.append(f"{path_string}: missing required key 'schema'")
            errors.append(
                f"{path_string}: schema must be one of "
                f"{', '.join(repr(item) for item in sorted(POINT_SCHEMAS))}"
            )
        else:
            _check_keys(
                value,
                TOP_LEVEL_REQUIRED_KEYS[schema],
                TOP_LEVEL_OPTIONAL_KEYS,
                path_string,
                errors,
            )
        _dictionary_identity(value, path, errors)
        points = _local_points(value, path, errors)
        if schema == POINT_SCHEMA_V2:
            imports = _imports(value, path, errors)
            aliases = _aliases(value, path, points, errors)
        else:
            imports = ()
            aliases = ()
        dictionaries[path_string] = PointDictionary(
            path_string, value, imports, aliases, points
        )

    for path in sorted(dictionaries):
        dictionary = dictionaries[path]
        for index, target in enumerate(dictionary.imports):
            if target not in dictionaries:
                errors.append(f"{path}: imports[{index}]: target {target!r} is missing")
        imported = set(dictionary.imports)
        for index, (name, target) in enumerate(dictionary.aliases):
            match = POINT_REF_RE.fullmatch(target)
            if match is None:
                continue
            target_path, target_name = match.groups()
            alias_label = f"{path}: aliases[{index}]"
            if target_path not in imported:
                errors.append(
                    f"{alias_label}.target: target path {target_path!r} is not in imports"
                )
                continue
            target_dictionary = dictionaries.get(target_path)
            if target_dictionary is None:
                errors.append(
                    f"{alias_label}.target: target dictionary {target_path!r} is missing"
                )
                continue
            if target_name in dict(target_dictionary.aliases):
                errors.append(
                    f"{alias_label}.target: alias-to-alias target {target!r} is forbidden"
                )
            elif target_name not in target_dictionary.points:
                errors.append(
                    f"{alias_label}.target: concrete point {target!r} is missing"
                )

    errors.extend(_cycle_errors(dictionaries))
    return PointCorpus(paths, documents, dictionaries, tuple(sorted(errors)))
