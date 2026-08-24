"""Materialize the validated internal join of scalar ABI rows and source claims.

Named rows supply order and ABI data. Source rows supply inventory-anchored file
locators plus caller claims for Modelica classes and members; those claims are
not declaration evidence. This module defines no serialized or public contract.
"""

from dataclasses import dataclass
import math
from typing import Iterable

from tools.lint import (
    routine_scalar_abi,
    routine_scalar_names,
    routine_scalar_source_claims,
)


_PRIMITIVES = ("boolean", "integer", "real")
_PARAMETER_SOURCES = ("assignment", "default")
_DIRECTIONS = ("input", "output")


@dataclass(frozen=True, order=True)
class BoundScalarDiagnostic:
    """One deterministic refusal from the internal scalar join."""

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


class BoundScalarError(ValueError):
    """Complete, sorted diagnostics raised before any bound row exists."""

    def __init__(self, diagnostics: Iterable[BoundScalarDiagnostic]):
        self.diagnostics = tuple(sorted(diagnostics))
        super().__init__("\n".join(str(diagnostic) for diagnostic in self.diagnostics))


@dataclass(frozen=True)
class BoundSourceClaim:
    """Detached source evidence for the validated join.

    The file locator retains the upstream inventory anchor. Class and member
    values remain caller claims rather than verified declarations.
    """

    canonical_class_path: str
    source_member: str
    snapshot: str
    revision: str
    file: routine_scalar_source_claims.SourceFileLocator


@dataclass(frozen=True)
class BoundScalarParameterRow:
    """One detached parameter ABI row joined to its caller source claim."""

    scalar_name: str
    parameter_id: str
    coordinates: tuple[routine_scalar_abi.ScalarCoordinate, ...]
    type: routine_scalar_abi.ScalarAbiType | routine_scalar_abi.ScalarEnumAbiType
    source: str
    value: bool | int | float | routine_scalar_abi.ScalarEnumAbiValue
    source_claim: BoundSourceClaim


@dataclass(frozen=True)
class BoundScalarConnectorRow:
    """One detached connector ABI row joined to its caller source claim."""

    scalar_name: str
    connector_id: str
    coordinates: tuple[routine_scalar_abi.ScalarCoordinate, ...]
    type: routine_scalar_abi.ScalarAbiType | routine_scalar_abi.ScalarEnumAbiType
    direction: str
    source_claim: BoundSourceClaim


@dataclass(frozen=True)
class BoundScalarProjection:
    """A frozen lowering input produced by validating both projections together.

    Named row order is authoritative. Source class and member values remain
    caller claims; this projection does not verify Modelica declarations.
    """

    canonical_id: str
    revision: int
    parameters: tuple[BoundScalarParameterRow, ...]
    connectors: tuple[BoundScalarConnectorRow, ...]

    def row_for_scalar(
        self, scalar_name: str
    ) -> BoundScalarParameterRow | BoundScalarConnectorRow:
        """Return the unique bound row without storing a second index."""

        for row in self.parameters:
            if row.scalar_name == scalar_name:
                return row
        for row in self.connectors:
            if row.scalar_name == scalar_name:
                return row
        if isinstance(scalar_name, str):
            displayed = scalar_name[:160]
            suffix = "..." if len(scalar_name) > len(displayed) else ""
            detail = f"{displayed!r}{suffix}"
        else:
            detail = f"<{type(scalar_name).__name__[:80]}>"
        raise KeyError(f"unknown scalar name: {detail}")


def _detached_text(value: str) -> str:
    return str.encode(value, "utf-8").decode("utf-8")


def _bounded_text(value: str, limit: int = 160) -> str:
    return value if len(value) <= limit else value[:limit] + "..."


def _diagnostic_owner(value) -> str:
    if isinstance(value, str) and value:
        try:
            return _bounded_text(_detached_text(value))
        except UnicodeEncodeError:
            pass
    return "$"


def _diagnostic(
    diagnostics: list[BoundScalarDiagnostic],
    code: str,
    owner_kind: str,
    owner_id,
    location: str,
    message: str,
) -> None:
    diagnostics.append(
        BoundScalarDiagnostic(
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
    diagnostics: list[BoundScalarDiagnostic],
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


def _valid_nonnegative_integer(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _validate_metadata(projection, label, diagnostics):
    canonical_id = _text(
        getattr(projection, "canonical_id", None),
        diagnostics=diagnostics,
        code="invalid_metadata",
        owner_kind="projection",
        owner_id=label,
        location=f"$.{label}.canonical_id",
        label="canonical_id",
    )
    revision = getattr(projection, "revision", None)
    if not isinstance(revision, int) or isinstance(revision, bool) or revision < 1:
        _diagnostic(
            diagnostics,
            "invalid_metadata",
            "projection",
            label,
            f"$.{label}.revision",
            "revision must be a positive non-Boolean integer",
        )
        revision = None
    return canonical_id, revision


def _validate_coordinates(
    coordinates,
    *,
    diagnostics,
    owner_kind,
    owner_id,
    location,
):
    if type(coordinates) is not tuple:
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
            _diagnostic(
                diagnostics,
                "invalid_coordinate",
                owner_kind,
                owner_id,
                coordinate_location,
                "coordinates must contain exact ScalarCoordinate values",
            )
            valid = False
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
            _diagnostic(
                diagnostics,
                "invalid_ordinal",
                owner_kind,
                owner_id,
                f"{coordinate_location}.ordinal",
                "ordinal must be a non-negative non-Boolean integer",
            )
            valid = False
        if dimension is None or member is None:
            valid = False
    return valid


def _expected_scalar_name(prefix, owner_id, coordinates) -> str:
    components = [owner_id.encode("utf-8").hex()]
    for coordinate in coordinates:
        components.extend(
            (
                coordinate.dimension_id.encode("utf-8").hex(),
                coordinate.member_id.encode("utf-8").hex(),
            )
        )
    return prefix + "_".join(components)


def _validate_abi_type(value, owner_kind, owner_id, location, diagnostics):
    if type(value) is routine_scalar_abi.ScalarAbiType:
        primitive = getattr(value, "primitive", None)
        if primitive not in _PRIMITIVES:
            _diagnostic(
                diagnostics,
                "invalid_abi_type",
                owner_kind,
                owner_id,
                f"{location}.primitive",
                "primitive must be 'boolean', 'integer', or 'real'",
            )
            primitive = None
        alias_type_id = getattr(value, "alias_type_id", None)
        optional_values = {}
        for field_name in ("quantity", "unit", "display_unit"):
            field_value = getattr(value, field_name, None)
            optional_values[field_name] = field_value
            if field_value is not None:
                _text(
                    field_value,
                    diagnostics=diagnostics,
                    code="invalid_abi_type",
                    owner_kind=owner_kind,
                    owner_id=owner_id,
                    location=f"{location}.{field_name}",
                    label=field_name,
                )
        if alias_type_id is not None:
            _text(
                alias_type_id,
                diagnostics=diagnostics,
                code="invalid_abi_type",
                owner_kind=owner_kind,
                owner_id=owner_id,
                location=f"{location}.alias_type_id",
                label="alias_type_id",
            )
        elif any(item is not None for item in optional_values.values()):
            _diagnostic(
                diagnostics,
                "invalid_abi_type",
                owner_kind,
                owner_id,
                location,
                "primitive ABI types cannot carry alias metadata",
            )
        return ("primitive", primitive)
    if type(value) is routine_scalar_abi.ScalarEnumAbiType:
        class_path = _text(
            getattr(value, "canonical_class_path", None),
            diagnostics=diagnostics,
            code="invalid_abi_type",
            owner_kind=owner_kind,
            owner_id=owner_id,
            location=f"{location}.canonical_class_path",
            label="enum canonical_class_path",
        )
        return ("enum", class_path)
    _diagnostic(
        diagnostics,
        "invalid_abi_type",
        owner_kind,
        owner_id,
        location,
        "type must be an exact scalar ABI type dataclass",
    )
    return None


def _validate_parameter_value(value, type_info, owner_id, location, diagnostics):
    if type_info is None:
        if type(value) not in (
            bool,
            int,
            float,
            routine_scalar_abi.ScalarEnumAbiValue,
        ):
            _diagnostic(
                diagnostics,
                "invalid_abi_value",
                "parameter",
                owner_id,
                location,
                "value must be an exact scalar ABI value",
            )
        return
    kind, name = type_info
    if kind == "enum":
        ordinal = (
            getattr(value, "ordinal", None)
            if type(value) is routine_scalar_abi.ScalarEnumAbiValue
            else None
        )
        if not isinstance(ordinal, int) or isinstance(ordinal, bool) or ordinal < 1:
            _diagnostic(
                diagnostics,
                "invalid_abi_value",
                "parameter",
                owner_id,
                location,
                "enum value must have a positive one-based ordinal",
            )
        return
    valid = False
    if name == "boolean":
        valid = type(value) is bool
    elif name == "integer":
        valid = type(value) is int
    elif name == "real":
        valid = type(value) in (int, float) and (
            type(value) is int or math.isfinite(value)
        )
    if not valid:
        _diagnostic(
            diagnostics,
            "invalid_abi_value",
            "parameter",
            owner_id,
            location,
            "value must match its scalar ABI type",
        )


def _validate_named_row(row, index, owner_kind, diagnostics):
    is_parameter = owner_kind == "parameter"
    row_type = (
        routine_scalar_names.NamedScalarParameterRow
        if is_parameter
        else routine_scalar_names.NamedScalarConnectorRow
    )
    container = "parameters" if is_parameter else "connectors"
    owner_field = "parameter_id" if is_parameter else "connector_id"
    prefix = "p_" if is_parameter else "c_"
    location = f"$.named_projection.{container}[{index}]"
    if type(row) is not row_type:
        _diagnostic(
            diagnostics,
            "invalid_named_row",
            owner_kind,
            "$",
            location,
            f"{container} must contain exact {row_type.__name__} values",
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
    coordinates_valid = _validate_coordinates(
        getattr(row, "coordinates", None),
        diagnostics=diagnostics,
        owner_kind=owner_kind,
        owner_id=owner_id,
        location=location,
    )
    if scalar_name is not None and not scalar_name.startswith(prefix):
        _diagnostic(
            diagnostics,
            "scalar_name_namespace",
            owner_kind,
            owner_id,
            f"{location}.scalar_name",
            f"{owner_kind} scalar names must start with {prefix!r}",
        )
    if owner_id is not None and scalar_name is not None and coordinates_valid:
        expected = _expected_scalar_name(prefix, owner_id, row.coordinates)
        if scalar_name != expected:
            _diagnostic(
                diagnostics,
                "scalar_name_mismatch",
                owner_kind,
                owner_id,
                f"{location}.scalar_name",
                "scalar name does not match its owner and stable coordinates",
            )

    type_info = _validate_abi_type(
        getattr(row, "type", None),
        owner_kind,
        owner_id,
        f"{location}.type",
        diagnostics,
    )
    if is_parameter:
        source = getattr(row, "source", None)
        if source not in _PARAMETER_SOURCES:
            _diagnostic(
                diagnostics,
                "invalid_parameter_source",
                owner_kind,
                owner_id,
                f"{location}.source",
                "source must be 'assignment' or 'default'",
            )
        _validate_parameter_value(
            getattr(row, "value", None),
            type_info,
            owner_id,
            f"{location}.value",
            diagnostics,
        )
    elif getattr(row, "direction", None) not in _DIRECTIONS:
        _diagnostic(
            diagnostics,
            "invalid_direction",
            owner_kind,
            owner_id,
            f"{location}.direction",
            "direction must be 'input' or 'output'",
        )
    if owner_id is None or scalar_name is None or not coordinates_valid:
        return None
    return scalar_name, row, index


def _validate_named_projection(projection, diagnostics):
    if type(projection) is not routine_scalar_names.NamedScalarProjection:
        _diagnostic(
            diagnostics,
            "invalid_named_projection",
            "projection",
            "$",
            "$.named_projection",
            "named_projection must be an exact NamedScalarProjection",
        )
        return None
    metadata = _validate_metadata(projection, "named_projection", diagnostics)
    candidates = {"parameter": [], "connector": []}
    for owner_kind, container in (
        ("parameter", "parameters"),
        ("connector", "connectors"),
    ):
        rows = getattr(projection, container, None)
        if type(rows) is not tuple:
            _diagnostic(
                diagnostics,
                "invalid_named_container",
                "projection",
                "$",
                f"$.named_projection.{container}",
                f"{container} must be a tuple",
            )
            continue
        for index, row in enumerate(rows):
            candidate = _validate_named_row(row, index, owner_kind, diagnostics)
            if candidate is not None:
                candidates[owner_kind].append(candidate)
    return metadata, candidates


def _validate_source_payload(row, owner_kind, owner_id, location, diagnostics):
    raw_class_path = getattr(row, "canonical_class_path", None)
    if routine_scalar_source_claims._class_path(raw_class_path) is None:
        _diagnostic(
            diagnostics,
            "invalid_source_class",
            owner_kind,
            owner_id,
            f"{location}.canonical_class_path",
            "canonical_class_path must be a bounded class below the G36 package",
        )
    raw_member = getattr(row, "source_member", None)
    if routine_scalar_source_claims._source_member(raw_member) is None:
        _diagnostic(
            diagnostics,
            "invalid_source_member",
            owner_kind,
            owner_id,
            f"{location}.source_member",
            "source_member must be a bounded Modelica identifier",
        )
    snapshot = getattr(row, "snapshot", None)
    if snapshot not in routine_scalar_source_claims._ROLES:
        _diagnostic(
            diagnostics,
            "invalid_source_snapshot",
            owner_kind,
            owner_id,
            f"{location}.snapshot",
            "snapshot must be 'release' or 'development'",
        )
    revision = getattr(row, "revision", None)
    if (
        not isinstance(revision, str)
        or routine_scalar_source_claims._PIN_RE.fullmatch(revision) is None
    ):
        _diagnostic(
            diagnostics,
            "invalid_source_revision",
            owner_kind,
            owner_id,
            f"{location}.revision",
            "source revision must be 40 lowercase hexadecimal characters",
        )
    locator = getattr(row, "file", None)
    if type(locator) is not routine_scalar_source_claims.SourceFileLocator:
        _diagnostic(
            diagnostics,
            "invalid_file_locator",
            owner_kind,
            owner_id,
            f"{location}.file",
            "file must be an exact SourceFileLocator",
        )
        return
    path = getattr(locator, "path", None)
    problem = routine_scalar_source_claims._safe_source_path(path)
    if problem is not None:
        _diagnostic(
            diagnostics,
            "invalid_source_path",
            owner_kind,
            owner_id,
            f"{location}.file.path",
            problem,
        )
    elif not isinstance(path, str) or not path.endswith(".mo"):
        _diagnostic(
            diagnostics,
            "invalid_source_path",
            owner_kind,
            owner_id,
            f"{location}.file.path",
            "source file path must end in '.mo'",
        )
    blob = getattr(locator, "git_blob_sha1", None)
    if (
        not isinstance(blob, str)
        or routine_scalar_source_claims._SHA1_RE.fullmatch(blob) is None
    ):
        _diagnostic(
            diagnostics,
            "invalid_source_blob",
            owner_kind,
            owner_id,
            f"{location}.file.git_blob_sha1",
            "git_blob_sha1 must match sha1:<40 lowercase hex>",
        )


def _validate_source_row(row, index, owner_kind, diagnostics):
    is_parameter = owner_kind == "parameter"
    row_type = (
        routine_scalar_source_claims.ScalarParameterSourceClaim
        if is_parameter
        else routine_scalar_source_claims.ScalarConnectorSourceClaim
    )
    container = "parameters" if is_parameter else "connectors"
    owner_field = "parameter_id" if is_parameter else "connector_id"
    prefix = "p_" if is_parameter else "c_"
    location = f"$.source_claim_projection.{container}[{index}]"
    if type(row) is not row_type:
        _diagnostic(
            diagnostics,
            "invalid_source_claim_row",
            owner_kind,
            "$",
            location,
            f"{container} must contain exact {row_type.__name__} values",
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
    coordinates_valid = _validate_coordinates(
        getattr(row, "coordinates", None),
        diagnostics=diagnostics,
        owner_kind=owner_kind,
        owner_id=owner_id,
        location=location,
    )
    if scalar_name is not None and not scalar_name.startswith(prefix):
        _diagnostic(
            diagnostics,
            "scalar_name_namespace",
            owner_kind,
            owner_id,
            f"{location}.scalar_name",
            f"{owner_kind} scalar names must start with {prefix!r}",
        )
    if owner_id is not None and scalar_name is not None and coordinates_valid:
        expected = _expected_scalar_name(prefix, owner_id, row.coordinates)
        if scalar_name != expected:
            _diagnostic(
                diagnostics,
                "scalar_name_mismatch",
                owner_kind,
                owner_id,
                f"{location}.scalar_name",
                "scalar name does not match its owner and stable coordinates",
            )
    _validate_source_payload(row, owner_kind, owner_id, location, diagnostics)
    if owner_id is None or scalar_name is None or not coordinates_valid:
        return None
    return scalar_name, row, index


def _validate_source_projection(projection, diagnostics):
    if type(projection) is not routine_scalar_source_claims.ScalarSourceClaimProjection:
        _diagnostic(
            diagnostics,
            "invalid_source_claim_projection",
            "projection",
            "$",
            "$.source_claim_projection",
            "source_claim_projection must be an exact ScalarSourceClaimProjection",
        )
        return None
    metadata = _validate_metadata(projection, "source_claim_projection", diagnostics)
    candidates = {"parameter": [], "connector": []}
    for owner_kind, container in (
        ("parameter", "parameters"),
        ("connector", "connectors"),
    ):
        rows = getattr(projection, container, None)
        if type(rows) is not tuple:
            _diagnostic(
                diagnostics,
                "invalid_source_claim_container",
                "projection",
                "$",
                f"$.source_claim_projection.{container}",
                f"{container} must be a tuple",
            )
            continue
        for index, row in enumerate(rows):
            candidate = _validate_source_row(row, index, owner_kind, diagnostics)
            if candidate is not None:
                candidates[owner_kind].append(candidate)
    return metadata, candidates


def _candidate_groups(candidates):
    groups = {}
    for candidate in candidates:
        groups.setdefault(candidate[0], []).append(candidate)
    return groups


def _validate_duplicates(label, candidates, diagnostics):
    groups = {
        owner_kind: _candidate_groups(candidates[owner_kind])
        for owner_kind in ("parameter", "connector")
    }
    for owner_kind in ("parameter", "connector"):
        for scalar_name, matches in sorted(groups[owner_kind].items()):
            if len(matches) > 1:
                _diagnostic(
                    diagnostics,
                    "duplicate_scalar_name",
                    owner_kind,
                    getattr(matches[0][1], f"{owner_kind}_id", None),
                    f"$.{label}.{owner_kind}s",
                    f"scalar name {_bounded_text(scalar_name)!r} occurs {len(matches)} times",
                )
    for scalar_name in sorted(set(groups["parameter"]) & set(groups["connector"])):
        _diagnostic(
            diagnostics,
            "cross_kind_collision",
            "projection",
            "$",
            f"$.{label}",
            f"scalar name {_bounded_text(scalar_name)!r} occurs in both namespaces",
        )
    return groups


def _compare_coordinates(named_row, claim_row, owner_kind, owner_id, diagnostics):
    named_coordinates = named_row.coordinates
    claim_coordinates = claim_row.coordinates
    if len(named_coordinates) != len(claim_coordinates):
        _diagnostic(
            diagnostics,
            "coordinate_count_mismatch",
            owner_kind,
            owner_id,
            "$.join.coordinates",
            "named and source-claim coordinate counts differ",
        )
    for index, (named, claim) in enumerate(
        zip(named_coordinates, claim_coordinates)
    ):
        location = f"$.join.coordinates[{index}]"
        if named.dimension_id != claim.dimension_id:
            _diagnostic(
                diagnostics,
                "dimension_mismatch",
                owner_kind,
                owner_id,
                f"{location}.dimension_id",
                "named and source-claim dimension IDs differ",
            )
        if named.member_id != claim.member_id:
            _diagnostic(
                diagnostics,
                "member_mismatch",
                owner_kind,
                owner_id,
                f"{location}.member_id",
                "named and source-claim member IDs differ",
            )
        if named.ordinal != claim.ordinal:
            _diagnostic(
                diagnostics,
                "ordinal_mismatch",
                owner_kind,
                owner_id,
                f"{location}.ordinal",
                "named and source-claim ordinals differ",
            )


def _validate_join(named_candidates, claim_candidates, diagnostics):
    named_groups = _validate_duplicates(
        "named_projection", named_candidates, diagnostics
    )
    claim_groups = _validate_duplicates(
        "source_claim_projection", claim_candidates, diagnostics
    )
    for owner_kind in ("parameter", "connector"):
        opposite = "connector" if owner_kind == "parameter" else "parameter"
        named_names = set(named_groups[owner_kind])
        claim_names = set(claim_groups[owner_kind])
        for scalar_name in sorted(named_names - claim_names):
            named_row = named_groups[owner_kind][scalar_name][0][1]
            owner_id = getattr(named_row, f"{owner_kind}_id", None)
            _diagnostic(
                diagnostics,
                "missing_source_claim",
                owner_kind,
                owner_id,
                f"$.source_claim_projection.{owner_kind}s",
                f"no source claim matches scalar {_bounded_text(scalar_name)!r}",
            )
            if scalar_name in claim_groups[opposite]:
                _diagnostic(
                    diagnostics,
                    "namespace_confusion",
                    owner_kind,
                    owner_id,
                    "$.join",
                    "matching source claim occurs in the opposite namespace",
                )
        for scalar_name in sorted(claim_names - named_names):
            claim_row = claim_groups[owner_kind][scalar_name][0][1]
            owner_id = getattr(claim_row, f"{owner_kind}_id", None)
            _diagnostic(
                diagnostics,
                "extra_source_claim",
                owner_kind,
                owner_id,
                f"$.source_claim_projection.{owner_kind}s",
                f"source claim scalar {_bounded_text(scalar_name)!r} has no named row",
            )
            if scalar_name in named_groups[opposite]:
                _diagnostic(
                    diagnostics,
                    "namespace_confusion",
                    owner_kind,
                    owner_id,
                    "$.join",
                    "source claim matches a named row in the opposite namespace",
                )
        for scalar_name in sorted(named_names & claim_names):
            named_matches = named_groups[owner_kind][scalar_name]
            claim_matches = claim_groups[owner_kind][scalar_name]
            if len(named_matches) != 1 or len(claim_matches) != 1:
                continue
            named_row = named_matches[0][1]
            claim_row = claim_matches[0][1]
            owner_field = f"{owner_kind}_id"
            named_owner = getattr(named_row, owner_field, None)
            claim_owner = getattr(claim_row, owner_field, None)
            if named_owner != claim_owner:
                _diagnostic(
                    diagnostics,
                    "owner_mismatch",
                    owner_kind,
                    named_owner,
                    f"$.join.{owner_field}",
                    "named and source-claim owner IDs differ",
                )
            _compare_coordinates(
                named_row, claim_row, owner_kind, named_owner, diagnostics
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


def _copy_type(value):
    if type(value) is routine_scalar_abi.ScalarEnumAbiType:
        return routine_scalar_abi.ScalarEnumAbiType(
            _detached_text(value.canonical_class_path)
        )
    return routine_scalar_abi.ScalarAbiType(
        primitive=_detached_text(value.primitive),
        alias_type_id=(
            _detached_text(value.alias_type_id)
            if value.alias_type_id is not None
            else None
        ),
        quantity=(
            _detached_text(value.quantity) if value.quantity is not None else None
        ),
        unit=_detached_text(value.unit) if value.unit is not None else None,
        display_unit=(
            _detached_text(value.display_unit)
            if value.display_unit is not None
            else None
        ),
    )


def _copy_value(value):
    if type(value) is routine_scalar_abi.ScalarEnumAbiValue:
        return routine_scalar_abi.ScalarEnumAbiValue(int(value.ordinal))
    if type(value) is bool:
        return bool(value)
    if type(value) is int:
        return int(value)
    return float(value)


def _copy_source_claim(row) -> BoundSourceClaim:
    return BoundSourceClaim(
        canonical_class_path=_detached_text(row.canonical_class_path),
        source_member=_detached_text(row.source_member),
        snapshot=_detached_text(row.snapshot),
        revision=_detached_text(row.revision),
        file=routine_scalar_source_claims.SourceFileLocator(
            _detached_text(row.file.path),
            _detached_text(row.file.git_blob_sha1),
        ),
    )


def bind_scalar_source_claims(
    named_projection: routine_scalar_names.NamedScalarProjection,
    source_claim_projection: routine_scalar_source_claims.ScalarSourceClaimProjection,
) -> BoundScalarProjection:
    """Validate and materialize one lowering input from two projections.

    The function matches rows by scalar name within their parameter or connector
    namespace, while retaining named-row order. It validates all input and join
    invariants before allocating output. Source classes and members remain caller
    claims rather than verified declarations.
    """

    diagnostics: list[BoundScalarDiagnostic] = []
    named = _validate_named_projection(named_projection, diagnostics)
    claims = _validate_source_projection(source_claim_projection, diagnostics)
    if named is not None and claims is not None:
        named_metadata, named_candidates = named
        claim_metadata, claim_candidates = claims
        named_canonical_id, named_revision = named_metadata
        claim_canonical_id, claim_revision = claim_metadata
        if (
            named_canonical_id is not None
            and claim_canonical_id is not None
            and named_canonical_id != claim_canonical_id
        ):
            _diagnostic(
                diagnostics,
                "canonical_id_mismatch",
                "projection",
                "$",
                "$.join.canonical_id",
                "projection canonical IDs differ",
            )
        if (
            named_revision is not None
            and claim_revision is not None
            and named_revision != claim_revision
        ):
            _diagnostic(
                diagnostics,
                "revision_mismatch",
                "projection",
                "$",
                "$.join.revision",
                "projection revisions differ",
            )
        _validate_join(named_candidates, claim_candidates, diagnostics)
    if diagnostics:
        raise BoundScalarError(diagnostics)

    claims_by_parameter = {
        row.scalar_name: row for row in source_claim_projection.parameters
    }
    claims_by_connector = {
        row.scalar_name: row for row in source_claim_projection.connectors
    }
    parameters = tuple(
        BoundScalarParameterRow(
            scalar_name=_detached_text(row.scalar_name),
            parameter_id=_detached_text(row.parameter_id),
            coordinates=_copy_coordinates(row.coordinates),
            type=_copy_type(row.type),
            source=_detached_text(row.source),
            value=_copy_value(row.value),
            source_claim=_copy_source_claim(claims_by_parameter[row.scalar_name]),
        )
        for row in named_projection.parameters
    )
    connectors = tuple(
        BoundScalarConnectorRow(
            scalar_name=_detached_text(row.scalar_name),
            connector_id=_detached_text(row.connector_id),
            coordinates=_copy_coordinates(row.coordinates),
            type=_copy_type(row.type),
            direction=_detached_text(row.direction),
            source_claim=_copy_source_claim(claims_by_connector[row.scalar_name]),
        )
        for row in named_projection.connectors
    )
    return BoundScalarProjection(
        canonical_id=_detached_text(named_projection.canonical_id),
        revision=int(named_projection.revision),
        parameters=parameters,
        connectors=connectors,
    )
