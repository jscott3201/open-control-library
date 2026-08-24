"""Project scalar caller claims into requirements for future declaration checks.

The frozen output is an internal, in-memory handoff. It does not parse source,
verify declarations, or define a public or persisted contract.
"""

from dataclasses import dataclass
import re
from typing import Iterable, cast

from tools.lint import routine_scalar_abi, routine_scalar_source_claims


_CANONICAL_ID_RE = re.compile(
    r"G36-05-(0[1-9]|1[0-9]|2[0-2])-[A-Z]+(?:-[A-Z]+)*"
)
_LOCAL_ID_RE = re.compile(r"[a-z][a-z0-9]*(?:_[a-z0-9]+)*")
_STABLE_MEMBER_ID_RE = re.compile(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*")
_SCALAR_NAME_RE = re.compile(r"[pc]_[0-9a-f]+(?:_[0-9a-f]+)*")
_MAX_CANONICAL_ID_LENGTH = 128
_MAX_LOCAL_ID_LENGTH = 64
_MAX_SCALAR_NAME_LENGTH = 646
_MAX_DIAGNOSTIC_TEXT_LENGTH = 160


@dataclass(frozen=True, order=True)
class DeclarationRequirementDiagnostic:
    """One deterministic refusal from declaration-requirement projection."""

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


class DeclarationRequirementError(ValueError):
    """Complete, sorted diagnostics raised before any requirement exists."""

    def __init__(self, diagnostics: Iterable[DeclarationRequirementDiagnostic]):
        self.diagnostics = tuple(sorted(diagnostics))
        super().__init__("\n".join(str(diagnostic) for diagnostic in self.diagnostics))


@dataclass(frozen=True)
class ParameterDeclarationRequirement:
    """One parameter owner that a future declaration check must examine."""

    parameter_id: str
    canonical_class_path: str
    source_member: str
    snapshot: str
    revision: str
    file: routine_scalar_source_claims.SourceFileLocator
    scalar_names: tuple[str, ...]


@dataclass(frozen=True)
class ConnectorDeclarationRequirement:
    """One connector owner that a future declaration check must examine."""

    connector_id: str
    canonical_class_path: str
    source_member: str
    snapshot: str
    revision: str
    file: routine_scalar_source_claims.SourceFileLocator
    scalar_names: tuple[str, ...]


@dataclass(frozen=True)
class DeclarationRequirementProjection:
    """Owner-level requirements that do not assert declaration evidence."""

    canonical_id: str
    revision: int
    parameters: tuple[ParameterDeclarationRequirement, ...]
    connectors: tuple[ConnectorDeclarationRequirement, ...]


def _detached_text(value: str) -> str:
    return str.encode(value, "utf-8").decode("utf-8")


def _bounded_text(value: str) -> str:
    if len(value) <= _MAX_DIAGNOSTIC_TEXT_LENGTH:
        return value
    return value[:_MAX_DIAGNOSTIC_TEXT_LENGTH] + "..."


def _diagnostic_owner(value) -> str:
    if isinstance(value, str) and value:
        try:
            return _bounded_text(_detached_text(value))
        except UnicodeEncodeError:
            pass
    return "$"


def _diagnostic(
    diagnostics: list[DeclarationRequirementDiagnostic],
    code: str,
    owner_kind: str,
    owner_id,
    location: str,
    message: str,
) -> None:
    diagnostics.append(
        DeclarationRequirementDiagnostic(
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
    diagnostics: list[DeclarationRequirementDiagnostic],
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


def _canonical_id(value, diagnostics) -> str | None:
    canonical_id = _text(
        value,
        diagnostics=diagnostics,
        code="invalid_metadata",
        owner_kind="projection",
        owner_id="$",
        location="$.source_claim_projection.canonical_id",
        label="canonical_id",
    )
    if canonical_id is not None and (
        len(canonical_id) > _MAX_CANONICAL_ID_LENGTH
        or _CANONICAL_ID_RE.fullmatch(canonical_id) is None
    ):
        _diagnostic(
            diagnostics,
            "invalid_metadata",
            "projection",
            "$",
            "$.source_claim_projection.canonical_id",
            "canonical_id must be a bounded canonical G36 class ID",
        )
        return None
    return canonical_id


def _local_id(
    value,
    *,
    diagnostics,
    code,
    owner_kind,
    owner_id,
    location,
    label,
    pattern=_LOCAL_ID_RE,
) -> str | None:
    text = _text(
        value,
        diagnostics=diagnostics,
        code=code,
        owner_kind=owner_kind,
        owner_id=owner_id,
        location=location,
        label=label,
    )
    if text is not None and (
        len(text) > _MAX_LOCAL_ID_LENGTH or pattern.fullmatch(text) is None
    ):
        _diagnostic(
            diagnostics,
            code,
            owner_kind,
            owner_id,
            location,
            f"{label} must be a bounded stable local ID",
        )
        return None
    return text


def _scalar_name(value, owner_kind, owner_id, location, diagnostics) -> str | None:
    scalar_name = _text(
        value,
        diagnostics=diagnostics,
        code="invalid_scalar_name",
        owner_kind=owner_kind,
        owner_id=owner_id,
        location=location,
        label="scalar_name",
    )
    if scalar_name is not None and (
        len(scalar_name) > _MAX_SCALAR_NAME_LENGTH
        or _SCALAR_NAME_RE.fullmatch(scalar_name) is None
    ):
        _diagnostic(
            diagnostics,
            "invalid_scalar_name",
            owner_kind,
            owner_id,
            location,
            "scalar_name must be a bounded internal scalar token",
        )
        return None
    return scalar_name


def _valid_nonnegative_integer(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _validate_coordinates(
    coordinates,
    *,
    diagnostics,
    owner_kind,
    owner_id,
    location,
):
    coordinate_location = f"{location}.coordinates"
    if type(coordinates) is not tuple:
        _diagnostic(
            diagnostics,
            "invalid_coordinates",
            owner_kind,
            owner_id,
            coordinate_location,
            "coordinates must be a tuple",
        )
        return None
    valid = True
    if len(coordinates) > 2:
        _diagnostic(
            diagnostics,
            "invalid_coordinates",
            owner_kind,
            owner_id,
            coordinate_location,
            "coordinates must have scalar, vector, or matrix rank",
        )
        valid = False
    copied = []
    for index, coordinate in enumerate(coordinates):
        item_location = f"{coordinate_location}[{index}]"
        if type(coordinate) is not routine_scalar_abi.ScalarCoordinate:
            _diagnostic(
                diagnostics,
                "invalid_coordinate",
                owner_kind,
                owner_id,
                item_location,
                "coordinates must contain exact ScalarCoordinate values",
            )
            valid = False
            continue
        dimension_id = _local_id(
            getattr(coordinate, "dimension_id", None),
            diagnostics=diagnostics,
            code="invalid_dimension_id",
            owner_kind=owner_kind,
            owner_id=owner_id,
            location=f"{item_location}.dimension_id",
            label="dimension_id",
        )
        member_id = _local_id(
            getattr(coordinate, "member_id", None),
            diagnostics=diagnostics,
            code="invalid_member_id",
            owner_kind=owner_kind,
            owner_id=owner_id,
            location=f"{item_location}.member_id",
            label="member_id",
            pattern=_STABLE_MEMBER_ID_RE,
        )
        ordinal = getattr(coordinate, "ordinal", None)
        if not _valid_nonnegative_integer(ordinal):
            _diagnostic(
                diagnostics,
                "invalid_ordinal",
                owner_kind,
                owner_id,
                f"{item_location}.ordinal",
                "ordinal must be a non-negative non-Boolean integer",
            )
            valid = False
        if dimension_id is None or member_id is None:
            valid = False
        elif _valid_nonnegative_integer(ordinal):
            copied.append((dimension_id, member_id, int(cast(int, ordinal))))
    return tuple(copied) if valid else None


def _expected_scalar_name(prefix: str, owner_id: str, coordinates) -> str:
    components = [owner_id.encode("utf-8").hex()]
    for dimension_id, member_id, _ in coordinates:
        components.extend(
            (dimension_id.encode("utf-8").hex(), member_id.encode("utf-8").hex())
        )
    return prefix + "_".join(components)


def _validate_source_payload(row, owner_kind, owner_id, location, diagnostics):
    raw_class_path = getattr(row, "canonical_class_path", None)
    class_path = routine_scalar_source_claims._class_path(raw_class_path)
    if class_path is None:
        _diagnostic(
            diagnostics,
            "invalid_source_class",
            owner_kind,
            owner_id,
            f"{location}.canonical_class_path",
            "canonical_class_path must be a bounded class below the G36 package",
        )

    raw_member = getattr(row, "source_member", None)
    source_member = routine_scalar_source_claims._source_member(raw_member)
    if source_member is None:
        _diagnostic(
            diagnostics,
            "invalid_source_member",
            owner_kind,
            owner_id,
            f"{location}.source_member",
            "source_member must be a bounded Modelica identifier",
        )

    raw_snapshot = getattr(row, "snapshot", None)
    snapshot = None
    if (
        not isinstance(raw_snapshot, str)
        or raw_snapshot not in routine_scalar_source_claims._ROLES
    ):
        _diagnostic(
            diagnostics,
            "invalid_source_snapshot",
            owner_kind,
            owner_id,
            f"{location}.snapshot",
            "snapshot must be 'release' or 'development'",
        )
    else:
        snapshot = _detached_text(raw_snapshot)

    raw_revision = getattr(row, "revision", None)
    revision = None
    if (
        not isinstance(raw_revision, str)
        or routine_scalar_source_claims._PIN_RE.fullmatch(raw_revision) is None
    ):
        _diagnostic(
            diagnostics,
            "invalid_source_revision",
            owner_kind,
            owner_id,
            f"{location}.revision",
            "revision must be 40 lowercase hexadecimal characters",
        )
    else:
        revision = _detached_text(raw_revision)

    locator = getattr(row, "file", None)
    path = None
    blob = None
    if type(locator) is not routine_scalar_source_claims.SourceFileLocator:
        _diagnostic(
            diagnostics,
            "invalid_file_locator",
            owner_kind,
            owner_id,
            f"{location}.file",
            "file must be an exact SourceFileLocator",
        )
    else:
        raw_path = getattr(locator, "path", None)
        problem = routine_scalar_source_claims._safe_source_path(raw_path)
        if problem is not None:
            _diagnostic(
                diagnostics,
                "invalid_source_path",
                owner_kind,
                owner_id,
                f"{location}.file.path",
                problem,
            )
        elif not isinstance(raw_path, str) or not raw_path.endswith(".mo"):
            _diagnostic(
                diagnostics,
                "invalid_source_path",
                owner_kind,
                owner_id,
                f"{location}.file.path",
                "source file path must end in '.mo'",
            )
        else:
            path = _detached_text(raw_path)

        raw_blob = getattr(locator, "git_blob_sha1", None)
        if (
            not isinstance(raw_blob, str)
            or routine_scalar_source_claims._SHA1_RE.fullmatch(raw_blob) is None
        ):
            _diagnostic(
                diagnostics,
                "invalid_source_blob",
                owner_kind,
                owner_id,
                f"{location}.file.git_blob_sha1",
                "git_blob_sha1 must match sha1:<40 lowercase hex>",
            )
        else:
            blob = _detached_text(raw_blob)

    values = (class_path, source_member, snapshot, revision, path, blob)
    return values if all(value is not None for value in values) else None


def _validate_row(row, index, owner_kind, diagnostics, scalar_candidates):
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

    raw_owner_id = getattr(row, owner_field, None)
    owner_id = _local_id(
        raw_owner_id,
        diagnostics=diagnostics,
        code="invalid_owner_id",
        owner_kind=owner_kind,
        owner_id=raw_owner_id,
        location=f"{location}.{owner_field}",
        label=owner_field,
    )
    scalar_name = _scalar_name(
        getattr(row, "scalar_name", None),
        owner_kind,
        owner_id,
        f"{location}.scalar_name",
        diagnostics,
    )
    if scalar_name is not None:
        scalar_candidates.append((owner_kind, owner_id or "$", scalar_name))
    coordinates = _validate_coordinates(
        getattr(row, "coordinates", None),
        diagnostics=diagnostics,
        owner_kind=owner_kind,
        owner_id=owner_id,
        location=location,
    )

    valid_name = scalar_name is not None
    if scalar_name is not None and not scalar_name.startswith(prefix):
        _diagnostic(
            diagnostics,
            "scalar_name_namespace",
            owner_kind,
            owner_id,
            f"{location}.scalar_name",
            f"{owner_kind} scalar names must start with {prefix!r}",
        )
        valid_name = False
    if owner_id is not None and coordinates is not None and scalar_name is not None:
        expected = _expected_scalar_name(prefix, owner_id, coordinates)
        if scalar_name != expected:
            _diagnostic(
                diagnostics,
                "scalar_name_mismatch",
                owner_kind,
                owner_id,
                f"{location}.scalar_name",
                "scalar name does not match its owner and stable coordinates",
            )
            valid_name = False

    source_identity = _validate_source_payload(
        row, owner_kind, owner_id, location, diagnostics
    )
    if (
        owner_id is None
        or coordinates is None
        or not valid_name
        or source_identity is None
    ):
        return None
    return owner_id, scalar_name, source_identity


def _validate_scalar_uniqueness(scalar_candidates, diagnostics):
    groups = {}
    for owner_kind in ("parameter", "connector"):
        by_name = {}
        for candidate_kind, owner_id, scalar_name in scalar_candidates:
            if candidate_kind != owner_kind:
                continue
            by_name.setdefault(scalar_name, []).append(owner_id)
        groups[owner_kind] = by_name
        for scalar_name, owner_ids in sorted(by_name.items()):
            if len(owner_ids) > 1:
                _diagnostic(
                    diagnostics,
                    "duplicate_scalar_name",
                    owner_kind,
                    sorted(owner_ids)[0],
                    f"$.source_claim_projection.{owner_kind}s",
                    f"scalar name {_bounded_text(scalar_name)!r} occurs {len(owner_ids)} times",
                )
    for scalar_name in sorted(set(groups["parameter"]) & set(groups["connector"])):
        _diagnostic(
            diagnostics,
            "cross_kind_collision",
            "projection",
            "$",
            "$.source_claim_projection",
            f"scalar name {_bounded_text(scalar_name)!r} occurs in both namespaces",
        )


def _validate_owner_sources(candidates, diagnostics):
    for owner_kind in ("parameter", "connector"):
        by_owner = {}
        by_source = {}
        for owner_id, _, source_identity in candidates[owner_kind]:
            by_owner.setdefault(owner_id, []).append(source_identity)
            source_key = source_identity[:2]
            by_source.setdefault(source_key, set()).add(owner_id)

        for owner_id, source_identities in sorted(by_owner.items()):
            if len(set(source_identities)) > 1:
                _diagnostic(
                    diagnostics,
                    "inconsistent_owner_source",
                    owner_kind,
                    owner_id,
                    f"$.source_claim_projection.{owner_kind}s",
                    "owner rows must carry one coherent source identity",
                )
        for source_key, owner_ids in sorted(by_source.items()):
            if len(owner_ids) > 1:
                _diagnostic(
                    diagnostics,
                    "duplicate_source_key",
                    owner_kind,
                    sorted(owner_ids)[0],
                    f"$.source_claim_projection.{owner_kind}s",
                    "distinct owners claim the same class and source member",
                )


def _validate_projection(source_claim_projection, diagnostics):
    if (
        type(source_claim_projection)
        is not routine_scalar_source_claims.ScalarSourceClaimProjection
    ):
        _diagnostic(
            diagnostics,
            "invalid_source_claim_projection",
            "projection",
            "$",
            "$.source_claim_projection",
            "source_claim_projection must be an exact ScalarSourceClaimProjection",
        )
        return None

    canonical_id = _canonical_id(
        getattr(source_claim_projection, "canonical_id", None), diagnostics
    )
    revision = getattr(source_claim_projection, "revision", None)
    if not isinstance(revision, int) or isinstance(revision, bool) or revision < 1:
        _diagnostic(
            diagnostics,
            "invalid_metadata",
            "projection",
            "$",
            "$.source_claim_projection.revision",
            "revision must be a positive non-Boolean integer",
        )
        revision = None

    candidates = {"parameter": [], "connector": []}
    scalar_candidates = []
    for owner_kind, container in (
        ("parameter", "parameters"),
        ("connector", "connectors"),
    ):
        rows = getattr(source_claim_projection, container, None)
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
            candidate = _validate_row(
                row, index, owner_kind, diagnostics, scalar_candidates
            )
            if candidate is not None:
                candidates[owner_kind].append(candidate)

    _validate_scalar_uniqueness(scalar_candidates, diagnostics)
    _validate_owner_sources(candidates, diagnostics)
    return canonical_id, revision, candidates


def _requirements(candidates, requirement_type):
    owners = {}
    for owner_id, scalar_name, source_identity in candidates:
        if owner_id not in owners:
            owners[owner_id] = [source_identity, []]
        owners[owner_id][1].append(scalar_name)

    return tuple(
        requirement_type(
            _detached_text(owner_id),
            _detached_text(source_identity[0]),
            _detached_text(source_identity[1]),
            _detached_text(source_identity[2]),
            _detached_text(source_identity[3]),
            routine_scalar_source_claims.SourceFileLocator(
                _detached_text(source_identity[4]),
                _detached_text(source_identity[5]),
            ),
            tuple(_detached_text(name) for name in scalar_names),
        )
        for owner_id, (source_identity, scalar_names) in owners.items()
    )


def project_declaration_requirements(
    source_claim_projection: routine_scalar_source_claims.ScalarSourceClaimProjection,
) -> DeclarationRequirementProjection:
    """Collapse scalar caller claims into owner-level future-check requirements.

    All rows are validated before output allocation. The result carries no
    declaration-verification evidence and performs no parser or source access.
    """

    diagnostics: list[DeclarationRequirementDiagnostic] = []
    validated = _validate_projection(source_claim_projection, diagnostics)
    if diagnostics:
        raise DeclarationRequirementError(diagnostics)
    if validated is None:
        raise AssertionError("validated projection is unavailable")

    canonical_id, revision, candidates = validated
    if canonical_id is None or revision is None:
        raise AssertionError("validated metadata is unavailable")
    parameters = _requirements(
        candidates["parameter"], ParameterDeclarationRequirement
    )
    connectors = _requirements(
        candidates["connector"], ConnectorDeclarationRequirement
    )
    return DeclarationRequirementProjection(
        _detached_text(canonical_id),
        int(revision),
        parameters,
        connectors,
    )
