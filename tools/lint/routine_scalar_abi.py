"""Project resolved routine leaves into an internal scalar ABI."""

from dataclasses import dataclass
from typing import cast

from tools.lint import routine_resolution


@dataclass(frozen=True, order=True)
class ScalarAbiDiagnostic:
    code: str
    owner_kind: str
    owner_id: str
    type_id: str
    message: str

    def __str__(self):
        return f"{self.code}: {self.owner_kind} {self.owner_id}: {self.message}"


class ScalarAbiError(ValueError):
    def __init__(self, diagnostics):
        self.diagnostics = tuple(sorted(diagnostics))
        super().__init__("\n".join(str(diagnostic) for diagnostic in self.diagnostics))


@dataclass(frozen=True)
class ScalarCoordinate:
    dimension_id: str
    member_id: str
    ordinal: int


@dataclass(frozen=True)
class ScalarAbiType:
    primitive: str
    alias_type_id: str | None = None
    quantity: str | None = None
    unit: str | None = None
    display_unit: str | None = None


@dataclass(frozen=True)
class ScalarParameterAbiRow:
    parameter_id: str
    coordinates: tuple[ScalarCoordinate, ...]
    type: ScalarAbiType
    source: str
    value: bool | int | float


@dataclass(frozen=True)
class ScalarConnectorAbiRow:
    connector_id: str
    coordinates: tuple[ScalarCoordinate, ...]
    type: ScalarAbiType
    direction: str


@dataclass(frozen=True)
class ScalarAbiProjection:
    canonical_id: str
    revision: int
    parameters: tuple[ScalarParameterAbiRow, ...]
    connectors: tuple[ScalarConnectorAbiRow, ...]


def _coordinates(
    coordinates: tuple[routine_resolution.Coordinate, ...],
) -> tuple[ScalarCoordinate, ...]:
    return tuple(
        ScalarCoordinate(
            coordinate.dimension_id,
            coordinate.member_id,
            coordinate.ordinal,
        )
        for coordinate in coordinates
    )


def _type(type_info: routine_resolution.ResolvedType) -> ScalarAbiType:
    is_alias = type_info.kind == "alias"
    return ScalarAbiType(
        primitive=cast(str, type_info.primitive),
        alias_type_id=type_info.type_id if is_alias else None,
        quantity=type_info.quantity if is_alias else None,
        unit=type_info.unit if is_alias else None,
        display_unit=type_info.display_unit if is_alias else None,
    )


def _enum_diagnostics(
    resolved: routine_resolution.ResolvedSpecialization,
) -> list[ScalarAbiDiagnostic]:
    diagnostics = []
    for parameter in resolved.parameters:
        if parameter.type.kind == "enum":
            diagnostics.append(
                ScalarAbiDiagnostic(
                    "unsupported_enum",
                    "parameter",
                    parameter.parameter_id,
                    cast(str, parameter.type.type_id),
                    f"enum type {parameter.type.type_id!r} has no scalar ABI mapping",
                )
            )
    for connector in resolved.connectors:
        if connector.active and connector.type.kind == "enum":
            diagnostics.append(
                ScalarAbiDiagnostic(
                    "unsupported_enum",
                    "connector",
                    connector.connector_id,
                    cast(str, connector.type.type_id),
                    f"enum type {connector.type.type_id!r} has no scalar ABI mapping",
                )
            )
    return diagnostics


def project_scalar_abi(
    resolved: routine_resolution.ResolvedSpecialization,
) -> ScalarAbiProjection:
    """Flatten primitive and alias leaves without assigning runtime identities."""
    if not isinstance(resolved, routine_resolution.ResolvedSpecialization):
        raise ScalarAbiError(
            (
                ScalarAbiDiagnostic(
                    "invalid_input",
                    "projection",
                    "$",
                    "",
                    "input must be a ResolvedSpecialization",
                ),
            )
        )

    diagnostics = _enum_diagnostics(resolved)
    if diagnostics:
        raise ScalarAbiError(diagnostics)

    parameters = tuple(
        ScalarParameterAbiRow(
            parameter.parameter_id,
            _coordinates(leaf.coordinates),
            _type(parameter.type),
            parameter.source,
            cast(bool | int | float, leaf.value),
        )
        for parameter in resolved.parameters
        for leaf in parameter.leaves
    )
    connectors = tuple(
        ScalarConnectorAbiRow(
            connector.connector_id,
            _coordinates(leaf.coordinates),
            _type(connector.type),
            connector.direction,
        )
        for connector in resolved.connectors
        if connector.active
        for leaf in connector.leaves
    )
    return ScalarAbiProjection(
        resolved.canonical_id,
        resolved.revision,
        parameters,
        connectors,
    )
