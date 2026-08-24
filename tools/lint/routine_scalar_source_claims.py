"""Join internal scalar names to inventory-anchored caller source claims.

Output from this module contains inventory-verified file locators plus
caller-supplied Modelica class and member claims. It does not verify declarations
or define a public or persisted source map.
"""

from dataclasses import dataclass
from itertools import groupby
import re
from typing import Iterable, cast

from tools.lint import routine_scalar_abi, routine_scalar_names


_SCHEMA = "cxf-library/g36-source-inventory/v1"
_REPOSITORY = "https://github.com/lbl-srg/modelica-buildings.git"
_SOURCE_ROOT = "Buildings/Controls/OBC/ASHRAE/G36"
_INVENTORY_SCOPE = "source-root-regular-files"
_DEPENDENCY_CLOSURE = "not-inventoried"
_ROLES = ("release", "development")
_TOP_LEVEL_KEYS = (
    "schema",
    "repository",
    "source_root",
    "inventory_scope",
    "dependency_closure",
    "license",
    "snapshots",
)
_LICENSE_KEYS = ("upstream_path", "retained_path", "git_blob_sha1", "sha256")
_SNAPSHOT_KEYS = (
    "role",
    "revision",
    "root_tree_sha1",
    "file_count",
    "total_bytes",
    "modelica_file_count",
    "package_order_count",
    "files",
)
_FILE_KEYS = ("path", "mode", "bytes", "git_blob_sha1", "sha256")
_PIN_RE = re.compile(r"[0-9a-f]{40}")
_SHA1_RE = re.compile(r"sha1:[0-9a-f]{40}")
_SHA256_RE = re.compile(r"sha256:[0-9a-f]{64}")
_IDENTIFIER_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
_CLASS_PREFIX = "Buildings.Controls.OBC.ASHRAE.G36"
_MAX_IDENTIFIER_LENGTH = 255
_MAX_CLASS_PATH_LENGTH = 1024


@dataclass(frozen=True, order=True)
class SourceClaimDiagnostic:
    """One deterministic validation failure for the internal claim join."""

    code: str
    owner_kind: str
    owner_id: str
    location: str
    message: str

    def __str__(self):
        return (
            f"{self.code}: {self.owner_kind} {self.owner_id}: "
            f"{self.location}: {self.message}"
        )


class SourceClaimError(ValueError):
    """Complete, sorted diagnostics raised instead of a partial projection."""

    def __init__(self, diagnostics: Iterable[SourceClaimDiagnostic]):
        self.diagnostics = tuple(sorted(diagnostics))
        super().__init__("\n".join(str(diagnostic) for diagnostic in self.diagnostics))


@dataclass(frozen=True)
class SourcePin:
    """A caller-supplied role and exact source revision."""

    role: str
    revision: str


@dataclass(frozen=True)
class SourceFileLocator:
    """A supplied G36 path and Git blob ID; projection verifies membership."""

    path: str
    git_blob_sha1: str


@dataclass(frozen=True)
class SourceClassClaim:
    """A caller claim that anchors one Modelica class to one inventoried file.

    The claim does not verify a declaration or define a public or persisted map.
    """

    canonical_class_path: str
    snapshot: str
    revision: str
    file: SourceFileLocator


@dataclass(frozen=True)
class SourceMemberBinding:
    """A caller claim binding one scalar owner to a Modelica class member.

    The claim does not verify a declaration or define a public or persisted map.
    """

    owner_kind: str
    owner_id: str
    canonical_class_path: str
    source_member: str


@dataclass(frozen=True)
class ScalarParameterSourceClaim:
    """An internal parameter claim with an inventory-verified file locator.

    The class and member remain caller claims. This row neither verifies a
    declaration nor defines a public or persisted source map.
    """

    scalar_name: str
    parameter_id: str
    coordinates: tuple[routine_scalar_abi.ScalarCoordinate, ...]
    canonical_class_path: str
    source_member: str
    snapshot: str
    revision: str
    file: SourceFileLocator


@dataclass(frozen=True)
class ScalarConnectorSourceClaim:
    """An internal connector claim with an inventory-verified file locator.

    The class and member remain caller claims. This row neither verifies a
    declaration nor defines a public or persisted source map.
    """

    scalar_name: str
    connector_id: str
    coordinates: tuple[routine_scalar_abi.ScalarCoordinate, ...]
    canonical_class_path: str
    source_member: str
    snapshot: str
    revision: str
    file: SourceFileLocator


@dataclass(frozen=True)
class ScalarSourceClaimProjection:
    """The internal scalar-to-source claim join.

    Rows contain inventory-verified file locators and caller-supplied class and
    member claims. They do not verify declarations or define a public or
    persisted source map. Reverse lookup is recomputed from the forward rows.
    """

    canonical_id: str
    revision: int
    parameters: tuple[ScalarParameterSourceClaim, ...]
    connectors: tuple[ScalarConnectorSourceClaim, ...]

    def claim_for_scalar(
        self, scalar_name: str
    ) -> ScalarParameterSourceClaim | ScalarConnectorSourceClaim:
        """Return the unique forward row for a scalar name."""

        for row in self.parameters:
            if row.scalar_name == scalar_name:
                return row
        for row in self.connectors:
            if row.scalar_name == scalar_name:
                return row
        raise KeyError(f"unknown scalar name: {scalar_name!r}")

    def scalar_names_for_source(
        self,
        owner_kind: str,
        canonical_class_path: str,
        source_member: str,
    ) -> tuple[str, ...]:
        """Derive ordered scalar names for one source-side owner key."""

        key = (owner_kind, canonical_class_path, source_member)
        if owner_kind == "parameter":
            rows = self.parameters
        elif owner_kind == "connector":
            rows = self.connectors
        else:
            raise KeyError(key)
        names = tuple(
            row.scalar_name
            for row in rows
            if row.canonical_class_path == canonical_class_path
            and row.source_member == source_member
        )
        if not names:
            raise KeyError(key)
        return names


def _detached_text(value: str) -> str:
    return str.encode(value, "utf-8").decode("utf-8")


def _diagnostic_owner(value) -> str:
    if isinstance(value, str) and value:
        try:
            return _detached_text(value)
        except UnicodeEncodeError:
            pass
    return "$"


def _diagnostic(
    diagnostics: list[SourceClaimDiagnostic],
    code: str,
    owner_kind: str,
    owner_id,
    location: str,
    message: str,
) -> None:
    diagnostics.append(
        SourceClaimDiagnostic(
            code,
            owner_kind,
            _diagnostic_owner(owner_id),
            location,
            message,
        )
    )


def _text(
    value,
    *,
    diagnostics: list[SourceClaimDiagnostic],
    code: str,
    owner_kind: str,
    owner_id,
    location: str,
    label: str,
) -> str | None:
    if not isinstance(value, str) or not value:
        _diagnostic(
            diagnostics,
            code,
            owner_kind,
            owner_id,
            location,
            f"{label} must be a non-empty string",
        )
        return None
    try:
        return _detached_text(value)
    except UnicodeEncodeError:
        _diagnostic(
            diagnostics,
            "utf8_encoding",
            owner_kind,
            owner_id,
            location,
            f"{label} must be UTF-8 encodable",
        )
        return None


def _exact_keys(
    value,
    expected: tuple[str, ...],
    *,
    diagnostics: list[SourceClaimDiagnostic],
    code: str,
    owner_kind: str,
    owner_id,
    location: str,
    label: str,
) -> bool:
    if type(value) is not dict:
        _diagnostic(
            diagnostics,
            code,
            owner_kind,
            owner_id,
            location,
            f"{label} must be an object",
        )
        return False
    if tuple(value) != expected:
        _diagnostic(
            diagnostics,
            code,
            owner_kind,
            owner_id,
            location,
            f"{label} keys must appear exactly in governed order",
        )
        return False
    return True


def _valid_nonnegative_integer(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _safe_source_path(path) -> str | None:
    if not isinstance(path, str) or not path:
        return "path must be a non-empty string"
    try:
        path.encode("utf-8")
    except UnicodeEncodeError:
        return "path must be UTF-8 encodable"
    if path.startswith("/"):
        return "absolute paths are forbidden"
    if "\\" in path:
        return "backslashes are forbidden"
    if any(ord(character) < 32 or ord(character) == 127 for character in path):
        return "control characters are forbidden"
    segments = path.split("/")
    if "" in segments:
        return "empty path segments are forbidden"
    if "." in segments:
        return "dot path segments are forbidden"
    if ".." in segments:
        return "parent traversal is forbidden"
    if not path.startswith(f"{_SOURCE_ROOT}/"):
        return f"path must be below {_SOURCE_ROOT}/"
    return None


def _class_path(value) -> str | None:
    if not isinstance(value, str) or not value:
        return None
    try:
        detached = _detached_text(value)
    except UnicodeEncodeError:
        return None
    if len(detached) > _MAX_CLASS_PATH_LENGTH:
        return None
    prefix = f"{_CLASS_PREFIX}."
    if not detached.startswith(prefix):
        return None
    suffix = detached[len(prefix) :]
    segments = suffix.split(".")
    if not segments or any(
        len(segment) > _MAX_IDENTIFIER_LENGTH
        or _IDENTIFIER_RE.fullmatch(segment) is None
        for segment in segments
    ):
        return None
    return detached


def _source_member(value) -> str | None:
    if not isinstance(value, str) or not value:
        return None
    try:
        detached = _detached_text(value)
    except UnicodeEncodeError:
        return None
    if len(detached) > _MAX_IDENTIFIER_LENGTH:
        return None
    if _IDENTIFIER_RE.fullmatch(detached) is None:
        return None
    return detached


def _validate_pins(source_pins, diagnostics):
    start = len(diagnostics)
    by_role: dict[str, list[str]] = {role: [] for role in _ROLES}
    valid_revisions = []
    if not isinstance(source_pins, tuple):
        _diagnostic(
            diagnostics,
            "invalid_source_pins",
            "pins",
            "$",
            "$.source_pins",
            "source_pins must be a tuple",
        )
        return {}, False

    for pin in source_pins:
        location = "$.source_pins"
        if type(pin) is not SourcePin:
            _diagnostic(
                diagnostics,
                "invalid_source_pin",
                "pins",
                "$",
                location,
                "source_pins must contain only SourcePin values",
            )
            continue
        role = getattr(pin, "role", None)
        revision = getattr(pin, "revision", None)
        valid_role = role in _ROLES
        detached_role = _detached_text(role) if valid_role else None
        if not valid_role:
            _diagnostic(
                diagnostics,
                "invalid_source_role",
                "pins",
                role,
                f"{location}.role",
                "role must be 'release' or 'development'",
            )
        valid_revision = isinstance(revision, str) and _PIN_RE.fullmatch(revision)
        if not valid_revision:
            _diagnostic(
                diagnostics,
                "invalid_source_revision",
                "pins",
                role,
                f"{location}.revision",
                "revision must be 40 lowercase hexadecimal characters",
            )
        if valid_role and valid_revision:
            detached = _detached_text(cast(str, revision))
            by_role[cast(str, detached_role)].append(detached)
            valid_revisions.append(detached)

    if len(source_pins) != 2:
        _diagnostic(
            diagnostics,
            "invalid_source_pins",
            "pins",
            "$",
            "$.source_pins",
            "source_pins must contain exactly release and development",
        )
    for role in _ROLES:
        count = len(by_role[role])
        if count == 0:
            _diagnostic(
                diagnostics,
                "missing_source_pin",
                "pins",
                role,
                "$.source_pins",
                f"source_pins is missing {role!r}",
            )
        elif count > 1:
            _diagnostic(
                diagnostics,
                "duplicate_source_pin",
                "pins",
                role,
                "$.source_pins",
                f"source_pins contains {count} {role!r} pins",
            )
    for revision, group in groupby(sorted(valid_revisions)):
        matches = tuple(group)
        if len(matches) > 1:
            _diagnostic(
                diagnostics,
                "duplicate_source_revision",
                "pins",
                "$",
                "$.source_pins",
                f"source revision {revision!r} is used more than once",
            )

    valid = len(diagnostics) == start
    pins = {
        role: revisions[0]
        for role, revisions in by_role.items()
        if len(revisions) == 1
    }
    return pins, valid


def _validate_license(value, diagnostics):
    location = "$.source_inventory.license"
    if not _exact_keys(
        value,
        _LICENSE_KEYS,
        diagnostics=diagnostics,
        code="invalid_inventory_license",
        owner_kind="inventory",
        owner_id="$",
        location=location,
        label="license",
    ):
        return
    constants = {
        "upstream_path": "Buildings/legal.html",
        "retained_path": "routines/g36/LICENSE-BUILDINGS.html",
    }
    for key, expected in constants.items():
        if value.get(key) != expected:
            _diagnostic(
                diagnostics,
                "invalid_inventory_license",
                "inventory",
                "$",
                f"{location}.{key}",
                f"{key} must match the governed inventory contract",
            )
    if not isinstance(value.get("git_blob_sha1"), str) or _SHA1_RE.fullmatch(
        value.get("git_blob_sha1")
    ) is None:
        _diagnostic(
            diagnostics,
            "invalid_inventory_license",
            "inventory",
            "$",
            f"{location}.git_blob_sha1",
            "git_blob_sha1 must match sha1:<40 lowercase hex>",
        )
    if not isinstance(value.get("sha256"), str) or _SHA256_RE.fullmatch(
        value.get("sha256")
    ) is None:
        _diagnostic(
            diagnostics,
            "invalid_inventory_license",
            "inventory",
            "$",
            f"{location}.sha256",
            "sha256 must match sha256:<64 lowercase hex>",
        )


def _validate_file_rows(files, role, diagnostics):
    location = f"$.source_inventory.snapshots[{_ROLES.index(role)}].files"
    if type(files) is not list:
        _diagnostic(
            diagnostics,
            "invalid_inventory_files",
            "inventory",
            role,
            location,
            "files must be a list",
        )
        return {}, None, False

    start = len(diagnostics)
    entries: list[tuple[str, str]] = []
    byte_sizes = []
    paths = []
    for index, row in enumerate(files):
        row_location = f"{location}[{index}]"
        if not _exact_keys(
            row,
            _FILE_KEYS,
            diagnostics=diagnostics,
            code="invalid_inventory_file",
            owner_kind="inventory",
            owner_id=role,
            location=row_location,
            label="file row",
        ):
            continue
        path = row.get("path")
        problem = _safe_source_path(path)
        valid_path = problem is None
        if problem is not None:
            _diagnostic(
                diagnostics,
                "unsafe_inventory_path",
                "inventory",
                role,
                f"{row_location}.path",
                problem,
            )
        if row.get("mode") != "100644":
            _diagnostic(
                diagnostics,
                "invalid_inventory_file",
                "inventory",
                role,
                f"{row_location}.mode",
                "mode must be '100644'",
            )
        size = row.get("bytes")
        valid_size = _valid_nonnegative_integer(size)
        if not valid_size:
            _diagnostic(
                diagnostics,
                "invalid_inventory_file",
                "inventory",
                role,
                f"{row_location}.bytes",
                "bytes must be a non-negative non-Boolean integer",
            )
        blob = row.get("git_blob_sha1")
        valid_blob = isinstance(blob, str) and _SHA1_RE.fullmatch(blob) is not None
        if not valid_blob:
            _diagnostic(
                diagnostics,
                "invalid_inventory_blob",
                "inventory",
                role,
                f"{row_location}.git_blob_sha1",
                "git_blob_sha1 must match sha1:<40 lowercase hex>",
            )
        sha256 = row.get("sha256")
        if not isinstance(sha256, str) or _SHA256_RE.fullmatch(sha256) is None:
            _diagnostic(
                diagnostics,
                "invalid_inventory_file",
                "inventory",
                role,
                f"{row_location}.sha256",
                "sha256 must match sha256:<64 lowercase hex>",
            )
        if valid_path:
            detached_path = _detached_text(path)
            paths.append(detached_path)
            if valid_blob:
                entries.append((detached_path, _detached_text(blob)))
        if valid_size:
            byte_sizes.append(int(size))

    for path, group in groupby(sorted(paths)):
        matches = tuple(group)
        if len(matches) > 1:
            _diagnostic(
                diagnostics,
                "duplicate_inventory_path",
                "inventory",
                role,
                location,
                f"path {path!r} occurs {len(matches)} times",
            )
    if paths and paths != sorted(paths):
        _diagnostic(
            diagnostics,
            "inventory_file_order",
            "inventory",
            role,
            location,
            "file paths must be lexicographically ordered",
        )

    valid = len(diagnostics) == start
    file_map = dict(entries) if valid else {}
    counts = None
    if valid:
        counts = {
            "file_count": len(files),
            "total_bytes": sum(byte_sizes),
            "modelica_file_count": sum(path.endswith(".mo") for path in paths),
            "package_order_count": sum(
                path.endswith("/package.order") for path in paths
            ),
        }
    return file_map, counts, valid


def _validate_snapshot(snapshot, index, expected_role, pins, diagnostics):
    location = f"$.source_inventory.snapshots[{index}]"
    if not _exact_keys(
        snapshot,
        _SNAPSHOT_KEYS,
        diagnostics=diagnostics,
        code="invalid_inventory_snapshot",
        owner_kind="inventory",
        owner_id=expected_role,
        location=location,
        label="snapshot",
    ):
        return {}, False

    start = len(diagnostics)
    if snapshot.get("role") != expected_role:
        _diagnostic(
            diagnostics,
            "inventory_snapshot_role",
            "inventory",
            expected_role,
            f"{location}.role",
            f"snapshot role must be {expected_role!r}",
        )
    revision = snapshot.get("revision")
    if not isinstance(revision, str) or _PIN_RE.fullmatch(revision) is None:
        _diagnostic(
            diagnostics,
            "invalid_inventory_revision",
            "inventory",
            expected_role,
            f"{location}.revision",
            "revision must be 40 lowercase hexadecimal characters",
        )
    elif expected_role in pins and revision != pins[expected_role]:
        _diagnostic(
            diagnostics,
            "inventory_snapshot_revision",
            "inventory",
            expected_role,
            f"{location}.revision",
            "snapshot revision must equal its supplied source pin",
        )
    root_tree = snapshot.get("root_tree_sha1")
    if not isinstance(root_tree, str) or _SHA1_RE.fullmatch(root_tree) is None:
        _diagnostic(
            diagnostics,
            "invalid_inventory_snapshot",
            "inventory",
            expected_role,
            f"{location}.root_tree_sha1",
            "root_tree_sha1 must match sha1:<40 lowercase hex>",
        )

    valid_counts = True
    for key in (
        "file_count",
        "total_bytes",
        "modelica_file_count",
        "package_order_count",
    ):
        if not _valid_nonnegative_integer(snapshot.get(key)):
            valid_counts = False
            _diagnostic(
                diagnostics,
                "invalid_inventory_count",
                "inventory",
                expected_role,
                f"{location}.{key}",
                f"{key} must be a non-negative non-Boolean integer",
            )

    file_map, expected_counts, files_valid = _validate_file_rows(
        snapshot.get("files"), expected_role, diagnostics
    )
    if valid_counts and files_valid and expected_counts is not None:
        for key, expected in expected_counts.items():
            if snapshot.get(key) != expected:
                _diagnostic(
                    diagnostics,
                    "inventory_count_mismatch",
                    "inventory",
                    expected_role,
                    f"{location}.{key}",
                    f"{key} must equal the value derived from files",
                )

    valid = len(diagnostics) == start
    return file_map if valid else {}, valid


def _validate_inventory(source_inventory, pins, diagnostics):
    start = len(diagnostics)
    location = "$.source_inventory"
    if not _exact_keys(
        source_inventory,
        _TOP_LEVEL_KEYS,
        diagnostics=diagnostics,
        code="invalid_inventory",
        owner_kind="inventory",
        owner_id="$",
        location=location,
        label="source_inventory",
    ):
        return {}, False

    constants = {
        "schema": _SCHEMA,
        "repository": _REPOSITORY,
        "source_root": _SOURCE_ROOT,
        "inventory_scope": _INVENTORY_SCOPE,
        "dependency_closure": _DEPENDENCY_CLOSURE,
    }
    for key, expected in constants.items():
        if source_inventory.get(key) != expected:
            _diagnostic(
                diagnostics,
                "inventory_constant",
                "inventory",
                "$",
                f"{location}.{key}",
                f"{key} must equal the governed constant",
            )
    _validate_license(source_inventory.get("license"), diagnostics)

    snapshots = source_inventory.get("snapshots")
    files_by_role = {}
    if type(snapshots) is not list or len(snapshots) != 2:
        _diagnostic(
            diagnostics,
            "invalid_inventory_snapshots",
            "inventory",
            "$",
            f"{location}.snapshots",
            "snapshots must be a two-row list ordered release then development",
        )
    else:
        for index, role in enumerate(_ROLES):
            file_map, valid = _validate_snapshot(
                snapshots[index], index, role, pins, diagnostics
            )
            if valid:
                files_by_role[role] = file_map

    valid = len(diagnostics) == start
    return files_by_role if valid else {}, valid


def _validate_coordinates(
    coordinates,
    *,
    diagnostics,
    owner_kind,
    owner_id,
    location,
):
    if not isinstance(coordinates, tuple):
        _diagnostic(
            diagnostics,
            "invalid_coordinates",
            owner_kind,
            owner_id,
            f"{location}.coordinates",
            "coordinates must be a tuple",
        )
        return False
    valid = True
    for index, coordinate in enumerate(coordinates):
        coordinate_location = f"{location}.coordinates[{index}]"
        if type(coordinate) is not routine_scalar_abi.ScalarCoordinate:
            valid = False
            _diagnostic(
                diagnostics,
                "invalid_coordinate",
                owner_kind,
                owner_id,
                coordinate_location,
                "coordinates must contain only ScalarCoordinate values",
            )
            continue
        dimension = _text(
            getattr(coordinate, "dimension_id", None),
            diagnostics=diagnostics,
            code="invalid_dimension_id",
            owner_kind=owner_kind,
            owner_id=owner_id,
            location=f"{coordinate_location}.dimension_id",
            label="dimension_id",
        )
        member = _text(
            getattr(coordinate, "member_id", None),
            diagnostics=diagnostics,
            code="invalid_member_id",
            owner_kind=owner_kind,
            owner_id=owner_id,
            location=f"{coordinate_location}.member_id",
            label="member_id",
        )
        ordinal = getattr(coordinate, "ordinal", None)
        if not _valid_nonnegative_integer(ordinal):
            valid = False
            _diagnostic(
                diagnostics,
                "invalid_ordinal",
                owner_kind,
                owner_id,
                f"{coordinate_location}.ordinal",
                "ordinal must be a non-negative non-Boolean integer",
            )
        if dimension is None or member is None:
            valid = False
    return valid


def _validate_named_row(row, index, owner_kind, diagnostics):
    is_parameter = owner_kind == "parameter"
    row_type = (
        routine_scalar_names.NamedScalarParameterRow
        if is_parameter
        else routine_scalar_names.NamedScalarConnectorRow
    )
    owner_field = "parameter_id" if is_parameter else "connector_id"
    container = "parameters" if is_parameter else "connectors"
    prefix = "p_" if is_parameter else "c_"
    location = f"$.named_projection.{container}[{index}]"
    if type(row) is not row_type:
        _diagnostic(
            diagnostics,
            "invalid_named_row",
            owner_kind,
            "$",
            location,
            f"{container} must contain only exact named scalar row values",
        )
        return None

    owner_id = _text(
        getattr(row, owner_field, None),
        diagnostics=diagnostics,
        code="invalid_owner_id",
        owner_kind=owner_kind,
        owner_id=getattr(row, owner_field, None),
        location=f"{location}.{owner_field}",
        label=owner_field,
    )
    scalar_name = _text(
        getattr(row, "scalar_name", None),
        diagnostics=diagnostics,
        code="invalid_scalar_name",
        owner_kind=owner_kind,
        owner_id=owner_id,
        location=f"{location}.scalar_name",
        label="scalar_name",
    )
    valid = owner_id is not None and scalar_name is not None
    if scalar_name is not None and not scalar_name.startswith(prefix):
        valid = False
        _diagnostic(
            diagnostics,
            "scalar_name_namespace",
            owner_kind,
            owner_id,
            f"{location}.scalar_name",
            f"{owner_kind} scalar names must start with {prefix!r}",
        )
    if not _validate_coordinates(
        getattr(row, "coordinates", None),
        diagnostics=diagnostics,
        owner_kind=owner_kind,
        owner_id=owner_id,
        location=location,
    ):
        valid = False
    if not valid:
        return None
    return scalar_name, owner_id, row


def _validate_named_projection(named_projection, diagnostics):
    start = len(diagnostics)
    if type(named_projection) is not routine_scalar_names.NamedScalarProjection:
        _diagnostic(
            diagnostics,
            "invalid_named_projection",
            "projection",
            "$",
            "$.named_projection",
            "named_projection must be an exact NamedScalarProjection",
        )
        return (), (), set(), False

    canonical_id = _text(
        getattr(named_projection, "canonical_id", None),
        diagnostics=diagnostics,
        code="invalid_named_metadata",
        owner_kind="projection",
        owner_id="$",
        location="$.named_projection.canonical_id",
        label="canonical_id",
    )
    revision = getattr(named_projection, "revision", None)
    if not isinstance(revision, int) or isinstance(revision, bool) or revision < 1:
        _diagnostic(
            diagnostics,
            "invalid_named_metadata",
            "projection",
            "$",
            "$.named_projection.revision",
            "revision must be a positive non-Boolean integer",
        )

    candidates = {"parameter": [], "connector": []}
    scalar_names = []
    for owner_kind, container_name in (
        ("parameter", "parameters"),
        ("connector", "connectors"),
    ):
        rows = getattr(named_projection, container_name, None)
        if not isinstance(rows, tuple):
            _diagnostic(
                diagnostics,
                "invalid_named_container",
                "projection",
                "$",
                f"$.named_projection.{container_name}",
                f"{container_name} must be a tuple",
            )
            continue
        for index, row in enumerate(rows):
            candidate = _validate_named_row(row, index, owner_kind, diagnostics)
            if candidate is not None:
                candidates[owner_kind].append(candidate)
                scalar_names.append((candidate[0], owner_kind, candidate[1]))

    for scalar_name, group in groupby(
        sorted(scalar_names), key=lambda item: item[0]
    ):
        matches = tuple(group)
        if len(matches) > 1:
            _diagnostic(
                diagnostics,
                "duplicate_scalar_name",
                "projection",
                "$",
                "$.named_projection",
                f"scalar name {scalar_name!r} occurs {len(matches)} times",
            )

    owners = {
        (owner_kind, candidate[1])
        for owner_kind in ("parameter", "connector")
        for candidate in candidates[owner_kind]
    }
    valid = len(diagnostics) == start and canonical_id is not None
    return (
        tuple(candidates["parameter"]),
        tuple(candidates["connector"]),
        owners,
        valid,
    )


def _validate_class_claims(
    class_claims, pins, pins_valid, inventory_files, inventory_valid, diagnostics
):
    start = len(diagnostics)
    claims_by_path: dict[str, list[tuple]] = {}
    valid_candidates = []
    locator_groups: dict[tuple[str, str], list[str]] = {}
    if not isinstance(class_claims, tuple):
        _diagnostic(
            diagnostics,
            "invalid_class_claims",
            "class",
            "$",
            "$.class_claims",
            "class_claims must be a tuple",
        )
        return {}, {}, False

    for claim in class_claims:
        location = "$.class_claims"
        if type(claim) is not SourceClassClaim:
            _diagnostic(
                diagnostics,
                "invalid_class_claim",
                "class",
                "$",
                location,
                "class_claims must contain only SourceClassClaim values",
            )
            continue
        raw_class_path = getattr(claim, "canonical_class_path", None)
        class_path = _class_path(raw_class_path)
        if class_path is None:
            _diagnostic(
                diagnostics,
                "invalid_class_path",
                "class",
                raw_class_path,
                f"{location}.canonical_class_path",
                "canonical_class_path must be a bounded class below the G36 package",
            )
        role = getattr(claim, "snapshot", None)
        valid_role = role in _ROLES
        detached_role = _detached_text(role) if valid_role else None
        if not valid_role:
            _diagnostic(
                diagnostics,
                "invalid_class_snapshot",
                "class",
                raw_class_path,
                f"{location}.snapshot",
                "snapshot must be 'release' or 'development'",
            )
        revision = getattr(claim, "revision", None)
        valid_revision = isinstance(revision, str) and _PIN_RE.fullmatch(revision)
        if not valid_revision:
            _diagnostic(
                diagnostics,
                "invalid_class_revision",
                "class",
                raw_class_path,
                f"{location}.revision",
                "revision must be 40 lowercase hexadecimal characters",
            )
        elif valid_role and pins_valid and revision != pins[detached_role]:
            _diagnostic(
                diagnostics,
                "class_revision_mismatch",
                "class",
                raw_class_path,
                f"{location}.revision",
                "class claim revision must equal its supplied source pin",
            )

        locator = getattr(claim, "file", None)
        path = None
        blob = None
        locator_valid = type(locator) is SourceFileLocator
        if not locator_valid:
            _diagnostic(
                diagnostics,
                "invalid_file_locator",
                "class",
                raw_class_path,
                f"{location}.file",
                "file must be an exact SourceFileLocator",
            )
        else:
            raw_path = getattr(locator, "path", None)
            problem = _safe_source_path(raw_path)
            if problem is not None:
                locator_valid = False
                _diagnostic(
                    diagnostics,
                    "unsafe_class_path",
                    "class",
                    raw_class_path,
                    f"{location}.file.path",
                    problem,
                )
            else:
                path = _detached_text(cast(str, raw_path))
                if not path.endswith(".mo"):
                    locator_valid = False
                    _diagnostic(
                        diagnostics,
                        "non_modelica_locator",
                        "class",
                        raw_class_path,
                        f"{location}.file.path",
                        "the primary class locator must end in '.mo'",
                    )
            raw_blob = getattr(locator, "git_blob_sha1", None)
            if not isinstance(raw_blob, str) or _SHA1_RE.fullmatch(raw_blob) is None:
                locator_valid = False
                _diagnostic(
                    diagnostics,
                    "invalid_file_blob",
                    "class",
                    raw_class_path,
                    f"{location}.file.git_blob_sha1",
                    "git_blob_sha1 must match sha1:<40 lowercase hex>",
                )
            else:
                blob = _detached_text(raw_blob)

        structural = (
            class_path is not None
            and valid_role
            and valid_revision
            and locator_valid
            and path is not None
            and blob is not None
        )
        claim_candidate = (
            class_path,
            detached_role,
            _detached_text(cast(str, revision)) if valid_revision else revision,
            path,
            blob,
        )
        if class_path is not None:
            claims_by_path.setdefault(class_path, []).append(claim_candidate)
        if path is not None and blob is not None:
            locator_groups.setdefault((path, blob), []).append(
                class_path or _diagnostic_owner(raw_class_path)
            )
        if structural:
            valid_candidates.append(claim_candidate)
            if inventory_valid:
                inventoried_blob = inventory_files[detached_role].get(path)
                if inventoried_blob is None:
                    _diagnostic(
                        diagnostics,
                        "absent_file_locator",
                        "class",
                        class_path,
                        f"{location}.file.path",
                        "file path is absent from the claimed snapshot",
                    )
                elif inventoried_blob != blob:
                    _diagnostic(
                        diagnostics,
                        "file_blob_mismatch",
                        "class",
                        class_path,
                        f"{location}.file.git_blob_sha1",
                        "file blob does not match the claimed snapshot path",
                    )

    for class_path, claims in sorted(claims_by_path.items()):
        if len(claims) > 1:
            _diagnostic(
                diagnostics,
                "duplicate_class_claim",
                "class",
                class_path,
                "$.class_claims",
                f"canonical class has {len(claims)} claims",
            )
    for (path, blob), classes in sorted(locator_groups.items()):
        if len(classes) > 1:
            _diagnostic(
                diagnostics,
                "duplicate_file_locator",
                "class",
                sorted(classes)[0],
                "$.class_claims",
                f"file locator ({path!r}, {blob!r}) occurs {len(classes)} times",
            )

    valid_by_path = {
        candidate[0]: candidate
        for candidate in valid_candidates
        if candidate[0] is not None and len(claims_by_path[candidate[0]]) == 1
    }
    return claims_by_path, valid_by_path, len(diagnostics) == start


def _validate_member_bindings(member_bindings, expected_owners, projection_valid, diagnostics):
    start = len(diagnostics)
    bindings_by_owner: dict[tuple[str, str], list[tuple[str, str, str, str]]] = {}
    source_groups: dict[tuple[str, str, str], set[str]] = {}
    if not isinstance(member_bindings, tuple):
        _diagnostic(
            diagnostics,
            "invalid_member_bindings",
            "binding",
            "$",
            "$.member_bindings",
            "member_bindings must be a tuple",
        )
        return {}, False

    for binding in member_bindings:
        location = "$.member_bindings"
        if type(binding) is not SourceMemberBinding:
            _diagnostic(
                diagnostics,
                "invalid_member_binding",
                "binding",
                "$",
                location,
                "member_bindings must contain only SourceMemberBinding values",
            )
            continue
        owner_kind = getattr(binding, "owner_kind", None)
        valid_owner_kind = owner_kind in ("parameter", "connector")
        detached_owner_kind = (
            _detached_text(owner_kind) if valid_owner_kind else None
        )
        if not valid_owner_kind:
            _diagnostic(
                diagnostics,
                "invalid_owner_kind",
                "binding",
                getattr(binding, "owner_id", None),
                f"{location}.owner_kind",
                "owner_kind must be 'parameter' or 'connector'",
            )
        owner_id = _text(
            getattr(binding, "owner_id", None),
            diagnostics=diagnostics,
            code="invalid_owner_id",
            owner_kind=(
                cast(str, detached_owner_kind) if valid_owner_kind else "binding"
            ),
            owner_id=getattr(binding, "owner_id", None),
            location=f"{location}.owner_id",
            label="owner_id",
        )
        raw_class_path = getattr(binding, "canonical_class_path", None)
        class_path = _class_path(raw_class_path)
        if class_path is None:
            _diagnostic(
                diagnostics,
                "invalid_binding_class_path",
                "binding",
                owner_id,
                f"{location}.canonical_class_path",
                "canonical_class_path must be a bounded class below the G36 package",
            )
        raw_member = getattr(binding, "source_member", None)
        source_member = _source_member(raw_member)
        if source_member is None:
            _diagnostic(
                diagnostics,
                "invalid_source_member",
                "binding",
                owner_id,
                f"{location}.source_member",
                "source_member must be a bounded Modelica identifier",
            )
        if (
            valid_owner_kind
            and owner_id is not None
            and class_path is not None
            and source_member is not None
        ):
            candidate = (
                cast(str, detached_owner_kind),
                owner_id,
                class_path,
                source_member,
            )
            bindings_by_owner.setdefault(
                (cast(str, detached_owner_kind), owner_id), []
            ).append(candidate)
            source_groups.setdefault(
                (cast(str, detached_owner_kind), class_path, source_member), set()
            ).add(owner_id)

    for owner, bindings in sorted(bindings_by_owner.items()):
        if len(bindings) > 1:
            _diagnostic(
                diagnostics,
                "duplicate_member_binding",
                owner[0],
                owner[1],
                "$.member_bindings",
                f"owner has {len(bindings)} member bindings",
            )
    for source_key, owner_ids in sorted(source_groups.items()):
        if len(owner_ids) > 1:
            _diagnostic(
                diagnostics,
                "duplicate_source_key",
                source_key[0],
                sorted(owner_ids)[0],
                "$.member_bindings",
                "distinct owners claim the same class and source member",
            )

    if projection_valid:
        for owner_kind, owner_id in sorted(expected_owners):
            count = len(bindings_by_owner.get((owner_kind, owner_id), ()))
            if count == 0:
                _diagnostic(
                    diagnostics,
                    "missing_member_binding",
                    owner_kind,
                    owner_id,
                    "$.member_bindings",
                    "named scalar owner has no member binding",
                )
        for owner_kind, owner_id in sorted(set(bindings_by_owner) - expected_owners):
            opposite = "connector" if owner_kind == "parameter" else "parameter"
            if (opposite, owner_id) in expected_owners:
                _diagnostic(
                    diagnostics,
                    "cross_namespace_binding",
                    owner_kind,
                    owner_id,
                    "$.member_bindings",
                    f"owner exists only in the {opposite} namespace",
                )
            _diagnostic(
                diagnostics,
                "extra_member_binding",
                owner_kind,
                owner_id,
                "$.member_bindings",
                "member binding has no named scalar owner",
            )

    valid_by_owner = {
        owner: bindings[0]
        for owner, bindings in bindings_by_owner.items()
        if len(bindings) == 1
    }
    return valid_by_owner, len(diagnostics) == start


def _validate_claim_usage(claims_by_path, bindings_by_owner, diagnostics):
    referenced_paths = {binding[2] for binding in bindings_by_owner.values()}
    for class_path in sorted(referenced_paths):
        count = len(claims_by_path.get(class_path, ()))
        if count == 0:
            _diagnostic(
                diagnostics,
                "missing_class_claim",
                "class",
                class_path,
                "$.class_claims",
                "member binding references no supplied class claim",
            )
        elif count > 1:
            _diagnostic(
                diagnostics,
                "ambiguous_class_claim",
                "class",
                class_path,
                "$.class_claims",
                "member binding references more than one supplied class claim",
            )

    for class_path, claims in sorted(claims_by_path.items()):
        if class_path not in referenced_paths:
            _diagnostic(
                diagnostics,
                "unused_class_claim",
                "class",
                class_path,
                "$.class_claims",
                "class claim is not referenced by a member binding",
            )
            for claim in claims:
                if claim[3] is not None and claim[4] is not None:
                    _diagnostic(
                        diagnostics,
                        "extra_file_locator",
                        "class",
                        class_path,
                        "$.class_claims",
                        "file locator belongs to an unused class claim",
                    )


def _copy_coordinates(coordinates):
    return tuple(
        routine_scalar_abi.ScalarCoordinate(
            _detached_text(coordinate.dimension_id),
            _detached_text(coordinate.member_id),
            int(coordinate.ordinal),
        )
        for coordinate in coordinates
    )


def _output_locator(claim) -> SourceFileLocator:
    return SourceFileLocator(_detached_text(claim[3]), _detached_text(claim[4]))


def project_scalar_source_claims(
    named_projection: routine_scalar_names.NamedScalarProjection,
    *,
    source_inventory,
    source_pins: tuple[SourcePin, ...],
    class_claims: tuple[SourceClassClaim, ...],
    member_bindings: tuple[SourceMemberBinding, ...],
) -> ScalarSourceClaimProjection:
    """Validate the complete join and project inventory-anchored source claims.

    No output row is allocated until every input and cross-reference is valid.
    Inventory membership verifies only the exact snapshot path and blob; class
    and member declarations remain caller claims.
    """

    diagnostics: list[SourceClaimDiagnostic] = []
    pins, pins_valid = _validate_pins(source_pins, diagnostics)
    inventory_files, inventory_valid = _validate_inventory(
        source_inventory, pins, diagnostics
    )
    parameter_rows, connector_rows, expected_owners, projection_valid = (
        _validate_named_projection(named_projection, diagnostics)
    )
    claims_by_path, valid_claims, _ = _validate_class_claims(
        class_claims,
        pins,
        pins_valid,
        inventory_files,
        inventory_valid,
        diagnostics,
    )
    valid_bindings, _ = _validate_member_bindings(
        member_bindings, expected_owners, projection_valid, diagnostics
    )
    _validate_claim_usage(claims_by_path, valid_bindings, diagnostics)
    if diagnostics:
        raise SourceClaimError(diagnostics)

    parameters = tuple(
        ScalarParameterSourceClaim(
            scalar_name=_detached_text(scalar_name),
            parameter_id=_detached_text(owner_id),
            coordinates=_copy_coordinates(row.coordinates),
            canonical_class_path=_detached_text(binding[2]),
            source_member=_detached_text(binding[3]),
            snapshot=_detached_text(claim[1]),
            revision=_detached_text(claim[2]),
            file=_output_locator(claim),
        )
        for scalar_name, owner_id, row in parameter_rows
        for binding in (valid_bindings[("parameter", owner_id)],)
        for claim in (valid_claims[binding[2]],)
    )
    connectors = tuple(
        ScalarConnectorSourceClaim(
            scalar_name=_detached_text(scalar_name),
            connector_id=_detached_text(owner_id),
            coordinates=_copy_coordinates(row.coordinates),
            canonical_class_path=_detached_text(binding[2]),
            source_member=_detached_text(binding[3]),
            snapshot=_detached_text(claim[1]),
            revision=_detached_text(claim[2]),
            file=_output_locator(claim),
        )
        for scalar_name, owner_id, row in connector_rows
        for binding in (valid_bindings[("connector", owner_id)],)
        for claim in (valid_claims[binding[2]],)
    )
    return ScalarSourceClaimProjection(
        canonical_id=_detached_text(named_projection.canonical_id),
        revision=int(named_projection.revision),
        parameters=parameters,
        connectors=connectors,
    )
