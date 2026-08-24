"""Allocate projection-scoped scalar tokens for the internal routine compiler.

Names from this module are transient compiler labels. They are not Engine paths,
IRIs, CXF identifiers, or persisted compatibility data.
"""

from dataclasses import dataclass
from itertools import groupby
from typing import Iterable, NoReturn, cast

from tools.lint import routine_scalar_abi


@dataclass(frozen=True, order=True)
class ScalarNameDiagnostic:
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


class ScalarNameError(ValueError):
    def __init__(self, diagnostics: Iterable[ScalarNameDiagnostic]):
        self.diagnostics = tuple(sorted(diagnostics))
        super().__init__("\n".join(str(diagnostic) for diagnostic in self.diagnostics))


@dataclass(frozen=True)
class NamedScalarParameterRow:
    scalar_name: str
    parameter_id: str
    coordinates: tuple[routine_scalar_abi.ScalarCoordinate, ...]
    type: routine_scalar_abi.ScalarAbiType | routine_scalar_abi.ScalarEnumAbiType
    source: str
    value: bool | int | float | routine_scalar_abi.ScalarEnumAbiValue


@dataclass(frozen=True)
class NamedScalarConnectorRow:
    scalar_name: str
    connector_id: str
    coordinates: tuple[routine_scalar_abi.ScalarCoordinate, ...]
    type: routine_scalar_abi.ScalarAbiType | routine_scalar_abi.ScalarEnumAbiType
    direction: str


@dataclass(frozen=True)
class NamedScalarProjection:
    canonical_id: str
    revision: int
    parameters: tuple[NamedScalarParameterRow, ...]
    connectors: tuple[NamedScalarConnectorRow, ...]


def _detached_text(value: str) -> str:
    return str.encode(value, "utf-8", "surrogatepass").decode(
        "utf-8", "surrogatepass"
    )


def _diagnostic_owner(value) -> str:
    if isinstance(value, str) and value:
        return _detached_text(value)
    return "$"


def _diagnostic(
    diagnostics: list[ScalarNameDiagnostic],
    code: str,
    owner_kind: str,
    owner_id,
    location: str,
    message: str,
) -> None:
    diagnostics.append(
        ScalarNameDiagnostic(
            code,
            owner_kind,
            _diagnostic_owner(owner_id),
            location,
            message,
        )
    )


def _invalid_input() -> NoReturn:
    raise ScalarNameError(
        (
            ScalarNameDiagnostic(
                "invalid_input",
                "projection",
                "$",
                "$",
                "input must be a ScalarAbiProjection",
            ),
        )
    )


def _name_component(
    value,
    *,
    invalid_code: str,
    label: str,
    owner_kind: str,
    owner_id,
    location: str,
    diagnostics: list[ScalarNameDiagnostic],
) -> str | None:
    if not isinstance(value, str) or not value:
        _diagnostic(
            diagnostics,
            invalid_code,
            owner_kind,
            owner_id,
            location,
            f"{label} must be a non-empty string",
        )
        return None
    try:
        return str.encode(value, "utf-8").hex()
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


def _validate_type(
    value,
    owner_kind: str,
    owner_id,
    location: str,
    diagnostics: list[ScalarNameDiagnostic],
) -> None:
    if type(value) is routine_scalar_abi.ScalarAbiType:
        primitive = getattr(value, "primitive", None)
        optional_fields = (
            getattr(value, "alias_type_id", None),
            getattr(value, "quantity", None),
            getattr(value, "unit", None),
            getattr(value, "display_unit", None),
        )
        if isinstance(primitive, str) and all(
            item is None or isinstance(item, str) for item in optional_fields
        ):
            return
    elif type(value) is routine_scalar_abi.ScalarEnumAbiType and isinstance(
        getattr(value, "canonical_class_path", None), str
    ):
        return
    _diagnostic(
        diagnostics,
        "invalid_abi_payload",
        owner_kind,
        owner_id,
        location,
        "type must be a scalar ABI type dataclass with string payload",
    )


def _validate_parameter_value(
    value,
    owner_id,
    location: str,
    diagnostics: list[ScalarNameDiagnostic],
) -> None:
    if type(value) in (bool, int, float):
        return
    if type(value) is routine_scalar_abi.ScalarEnumAbiValue:
        ordinal = getattr(value, "ordinal", None)
        if isinstance(ordinal, int) and not isinstance(ordinal, bool):
            return
    _diagnostic(
        diagnostics,
        "invalid_abi_payload",
        "parameter",
        owner_id,
        location,
        "value must be a scalar ABI value",
    )


def _validate_coordinates(
    coordinates,
    *,
    prefix: str,
    owner_kind: str,
    owner_id,
    location: str,
    diagnostics: list[ScalarNameDiagnostic],
) -> str | None:
    owner_component = _name_component(
        owner_id,
        invalid_code="invalid_owner_id",
        label=f"{owner_kind}_id",
        owner_kind=owner_kind,
        owner_id=owner_id,
        location=f"{location}.{owner_kind}_id",
        diagnostics=diagnostics,
    )
    if not isinstance(coordinates, tuple):
        _diagnostic(
            diagnostics,
            "invalid_coordinates",
            owner_kind,
            owner_id,
            f"{location}.coordinates",
            "coordinates must be a tuple",
        )
        return None

    components = [] if owner_component is None else [owner_component]
    valid = owner_component is not None
    for index, coordinate in enumerate(coordinates):
        coordinate_location = f"{location}.coordinates[{index}]"
        if type(coordinate) is not routine_scalar_abi.ScalarCoordinate:
            _diagnostic(
                diagnostics,
                "invalid_coordinate",
                owner_kind,
                owner_id,
                coordinate_location,
                "coordinates must contain only ScalarCoordinate values",
            )
            valid = False
            continue

        dimension_component = _name_component(
            getattr(coordinate, "dimension_id", None),
            invalid_code="invalid_dimension_id",
            label="dimension_id",
            owner_kind=owner_kind,
            owner_id=owner_id,
            location=f"{coordinate_location}.dimension_id",
            diagnostics=diagnostics,
        )
        member_component = _name_component(
            getattr(coordinate, "member_id", None),
            invalid_code="invalid_member_id",
            label="member_id",
            owner_kind=owner_kind,
            owner_id=owner_id,
            location=f"{coordinate_location}.member_id",
            diagnostics=diagnostics,
        )
        ordinal = getattr(coordinate, "ordinal", None)
        if not isinstance(ordinal, int) or isinstance(ordinal, bool) or ordinal < 0:
            _diagnostic(
                diagnostics,
                "invalid_ordinal",
                owner_kind,
                owner_id,
                f"{coordinate_location}.ordinal",
                "ordinal must be a non-negative non-Boolean integer",
            )
            valid = False
        if dimension_component is None or member_component is None:
            valid = False
        else:
            components.extend((dimension_component, member_component))

    if not valid:
        return None
    return prefix + "_".join(components)


def _validate_parameter_row(
    row,
    index: int,
    diagnostics: list[ScalarNameDiagnostic],
) -> tuple[str, str] | None:
    location = f"$.parameters[{index}]"
    if type(row) is not routine_scalar_abi.ScalarParameterAbiRow:
        _diagnostic(
            diagnostics,
            "invalid_row",
            "parameter",
            "$",
            location,
            "parameters must contain only ScalarParameterAbiRow values",
        )
        return None

    owner_id = getattr(row, "parameter_id", None)
    scalar_name = _validate_coordinates(
        getattr(row, "coordinates", None),
        prefix="p_",
        owner_kind="parameter",
        owner_id=owner_id,
        location=location,
        diagnostics=diagnostics,
    )
    _validate_type(
        getattr(row, "type", None),
        "parameter",
        owner_id,
        f"{location}.type",
        diagnostics,
    )
    if not isinstance(getattr(row, "source", None), str):
        _diagnostic(
            diagnostics,
            "invalid_abi_payload",
            "parameter",
            owner_id,
            f"{location}.source",
            "source must be a string",
        )
    _validate_parameter_value(
        getattr(row, "value", None),
        owner_id,
        f"{location}.value",
        diagnostics,
    )
    if scalar_name is None:
        return None
    return scalar_name, _detached_text(cast(str, owner_id))


def _validate_connector_row(
    row,
    index: int,
    diagnostics: list[ScalarNameDiagnostic],
) -> tuple[str, str] | None:
    location = f"$.connectors[{index}]"
    if type(row) is not routine_scalar_abi.ScalarConnectorAbiRow:
        _diagnostic(
            diagnostics,
            "invalid_row",
            "connector",
            "$",
            location,
            "connectors must contain only ScalarConnectorAbiRow values",
        )
        return None

    owner_id = getattr(row, "connector_id", None)
    scalar_name = _validate_coordinates(
        getattr(row, "coordinates", None),
        prefix="c_",
        owner_kind="connector",
        owner_id=owner_id,
        location=location,
        diagnostics=diagnostics,
    )
    _validate_type(
        getattr(row, "type", None),
        "connector",
        owner_id,
        f"{location}.type",
        diagnostics,
    )
    if not isinstance(getattr(row, "direction", None), str):
        _diagnostic(
            diagnostics,
            "invalid_abi_payload",
            "connector",
            owner_id,
            f"{location}.direction",
            "direction must be a string",
        )
    if scalar_name is None:
        return None
    return scalar_name, _detached_text(cast(str, owner_id))


def _duplicate_diagnostics(
    candidates: list[tuple[str, str]],
    owner_kind: str,
) -> list[ScalarNameDiagnostic]:
    diagnostics = []
    for scalar_name, group in groupby(sorted(candidates), key=lambda item: item[0]):
        matches = tuple(group)
        if len(matches) > 1:
            diagnostics.append(
                ScalarNameDiagnostic(
                    "duplicate_scalar_name",
                    owner_kind,
                    matches[0][1],
                    f"$.{owner_kind}s",
                    f"generated scalar name {scalar_name!r} occurs {len(matches)} times",
                )
            )
    return diagnostics


def _validate_projection(projection):
    diagnostics: list[ScalarNameDiagnostic] = []
    canonical_id = getattr(projection, "canonical_id", None)
    if not isinstance(canonical_id, str) or not canonical_id:
        _diagnostic(
            diagnostics,
            "invalid_metadata",
            "projection",
            "$",
            "$.canonical_id",
            "canonical_id must be a non-empty string",
        )
    else:
        try:
            str.encode(canonical_id, "utf-8")
        except UnicodeEncodeError:
            _diagnostic(
                diagnostics,
                "utf8_encoding",
                "projection",
                "$",
                "$.canonical_id",
                "canonical_id must be UTF-8 encodable",
            )
    revision = getattr(projection, "revision", None)
    if not isinstance(revision, int) or isinstance(revision, bool) or revision < 1:
        _diagnostic(
            diagnostics,
            "invalid_metadata",
            "projection",
            "$",
            "$.revision",
            "revision must be a positive non-Boolean integer",
        )

    parameters = getattr(projection, "parameters", None)
    parameter_candidates = []
    if not isinstance(parameters, tuple):
        _diagnostic(
            diagnostics,
            "invalid_container",
            "projection",
            "$",
            "$.parameters",
            "parameters must be a tuple",
        )
    else:
        for index, row in enumerate(parameters):
            candidate = _validate_parameter_row(row, index, diagnostics)
            if candidate is not None:
                parameter_candidates.append(candidate)

    connectors = getattr(projection, "connectors", None)
    connector_candidates = []
    if not isinstance(connectors, tuple):
        _diagnostic(
            diagnostics,
            "invalid_container",
            "projection",
            "$",
            "$.connectors",
            "connectors must be a tuple",
        )
    else:
        for index, row in enumerate(connectors):
            candidate = _validate_connector_row(row, index, diagnostics)
            if candidate is not None:
                connector_candidates.append(candidate)

    diagnostics.extend(_duplicate_diagnostics(parameter_candidates, "parameter"))
    diagnostics.extend(_duplicate_diagnostics(connector_candidates, "connector"))
    return diagnostics, parameter_candidates, connector_candidates


def _copy_coordinate(
    coordinate: routine_scalar_abi.ScalarCoordinate,
) -> routine_scalar_abi.ScalarCoordinate:
    return routine_scalar_abi.ScalarCoordinate(
        _detached_text(coordinate.dimension_id),
        _detached_text(coordinate.member_id),
        int(coordinate.ordinal),
    )


def _copy_type(
    value: routine_scalar_abi.ScalarAbiType | routine_scalar_abi.ScalarEnumAbiType,
) -> routine_scalar_abi.ScalarAbiType | routine_scalar_abi.ScalarEnumAbiType:
    if isinstance(value, routine_scalar_abi.ScalarEnumAbiType):
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
        quantity=_detached_text(value.quantity) if value.quantity is not None else None,
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
    return value


def allocate_scalar_names(
    projection: routine_scalar_abi.ScalarAbiProjection,
) -> NamedScalarProjection:
    """Assign internal names after validating the complete scalar ABI projection.

    Coordinate ordinals stay in the copied ABI payload but do not participate in
    naming. Invalid input raises ``ScalarNameError`` before any named row exists.
    """
    if type(projection) is not routine_scalar_abi.ScalarAbiProjection:
        _invalid_input()

    diagnostics, parameter_candidates, connector_candidates = _validate_projection(
        projection
    )
    if diagnostics:
        raise ScalarNameError(diagnostics)

    parameters = tuple(
        NamedScalarParameterRow(
            scalar_name=candidate[0],
            parameter_id=_detached_text(row.parameter_id),
            coordinates=tuple(_copy_coordinate(item) for item in row.coordinates),
            type=_copy_type(row.type),
            source=_detached_text(row.source),
            value=_copy_value(row.value),
        )
        for row, candidate in zip(
            projection.parameters, parameter_candidates, strict=True
        )
    )
    connectors = tuple(
        NamedScalarConnectorRow(
            scalar_name=candidate[0],
            connector_id=_detached_text(row.connector_id),
            coordinates=tuple(_copy_coordinate(item) for item in row.coordinates),
            type=_copy_type(row.type),
            direction=_detached_text(row.direction),
        )
        for row, candidate in zip(
            projection.connectors, connector_candidates, strict=True
        )
    )
    return NamedScalarProjection(
        _detached_text(projection.canonical_id),
        int(projection.revision),
        parameters,
        connectors,
    )
