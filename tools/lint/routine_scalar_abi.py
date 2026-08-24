"""Project resolved routine leaves into an internal scalar ABI."""

from collections import Counter
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
class EnumAbiMemberMapping:
    member_id: str
    source_literal: str


@dataclass(frozen=True)
class EnumAbiMapping:
    type_id: str
    canonical_class_path: str
    source_members: tuple[str, ...]
    member_mappings: tuple[EnumAbiMemberMapping, ...]


@dataclass(frozen=True)
class ScalarAbiType:
    primitive: str
    alias_type_id: str | None = None
    quantity: str | None = None
    unit: str | None = None
    display_unit: str | None = None


@dataclass(frozen=True)
class ScalarEnumAbiType:
    canonical_class_path: str


@dataclass(frozen=True)
class ScalarEnumAbiValue:
    ordinal: int


@dataclass(frozen=True)
class ScalarParameterAbiRow:
    parameter_id: str
    coordinates: tuple[ScalarCoordinate, ...]
    type: ScalarAbiType | ScalarEnumAbiType
    source: str
    value: bool | int | float | ScalarEnumAbiValue


@dataclass(frozen=True)
class ScalarConnectorAbiRow:
    connector_id: str
    coordinates: tuple[ScalarCoordinate, ...]
    type: ScalarAbiType | ScalarEnumAbiType
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


def _mapping_diagnostic(code, type_id, message):
    diagnostic_type_id = type_id if isinstance(type_id, str) else ""
    return ScalarAbiDiagnostic(
        code,
        "mapping",
        diagnostic_type_id or "$",
        diagnostic_type_id,
        message,
    )


def _named_types(resolved: routine_resolution.ResolvedSpecialization):
    named_types = {}
    for parameter in resolved.parameters:
        if parameter.type.type_id is not None:
            named_types.setdefault(parameter.type.type_id, parameter.type)
    for connector in resolved.connectors:
        if connector.type.type_id is not None:
            named_types.setdefault(connector.type.type_id, connector.type)
    return named_types


def _mapping_diagnostics(
    resolved: routine_resolution.ResolvedSpecialization,
    enum_mappings,
):
    if not isinstance(enum_mappings, tuple):
        return (
            [
                _mapping_diagnostic(
                    "invalid_enum_mappings",
                    "",
                    "enum_mappings must be a tuple",
                )
            ],
            {},
        )

    diagnostics = []
    mappings = [
        mapping for mapping in enum_mappings if isinstance(mapping, EnumAbiMapping)
    ]
    if len(mappings) != len(enum_mappings):
        diagnostics.append(
            _mapping_diagnostic(
                "invalid_enum_mappings",
                "",
                "enum_mappings must contain only EnumAbiMapping values",
            )
        )

    named_types = _named_types(resolved)
    mappings_by_type = {}
    for mapping in mappings:
        type_id = mapping.type_id
        valid_type_id = isinstance(type_id, str) and bool(type_id)
        if not valid_type_id:
            diagnostics.append(
                _mapping_diagnostic(
                    "invalid_enum_mapping",
                    type_id,
                    "type_id must be a non-empty string",
                )
            )
        else:
            mappings_by_type.setdefault(type_id, []).append(mapping)

        if not isinstance(mapping.canonical_class_path, str) or not (
            mapping.canonical_class_path
        ):
            diagnostics.append(
                _mapping_diagnostic(
                    "invalid_enum_mapping",
                    type_id,
                    "canonical_class_path must be a non-empty string",
                )
            )

        valid_source_members = []
        if not isinstance(mapping.source_members, tuple):
            diagnostics.append(
                _mapping_diagnostic(
                    "invalid_enum_mapping",
                    type_id,
                    "source_members must be a tuple",
                )
            )
        elif not mapping.source_members:
            diagnostics.append(
                _mapping_diagnostic(
                    "invalid_enum_mapping",
                    type_id,
                    "source_members must not be empty",
                )
            )
        else:
            for source_literal in mapping.source_members:
                if not isinstance(source_literal, str) or not source_literal:
                    diagnostics.append(
                        _mapping_diagnostic(
                            "invalid_enum_mapping",
                            type_id,
                            "source_members must contain only non-empty strings",
                        )
                    )
                else:
                    valid_source_members.append(source_literal)
            for source_literal, count in sorted(Counter(valid_source_members).items()):
                if count > 1:
                    diagnostics.append(
                        _mapping_diagnostic(
                            "duplicate_enum_source_member",
                            type_id,
                            f"source_members contains duplicate literal {source_literal!r}",
                        )
                    )

        valid_member_ids = []
        valid_source_literals = []
        member_mappings_are_tuple = isinstance(mapping.member_mappings, tuple)
        if not member_mappings_are_tuple:
            diagnostics.append(
                _mapping_diagnostic(
                    "invalid_enum_mapping",
                    type_id,
                    "member_mappings must be a tuple",
                )
            )
        else:
            malformed_member_mapping = False
            for member_mapping in mapping.member_mappings:
                if not isinstance(member_mapping, EnumAbiMemberMapping):
                    malformed_member_mapping = True
                    continue
                if not isinstance(member_mapping.member_id, str) or not (
                    member_mapping.member_id
                ):
                    diagnostics.append(
                        _mapping_diagnostic(
                            "invalid_enum_mapping",
                            type_id,
                            "member_id must be a non-empty string",
                        )
                    )
                else:
                    valid_member_ids.append(member_mapping.member_id)
                if not isinstance(member_mapping.source_literal, str) or not (
                    member_mapping.source_literal
                ):
                    diagnostics.append(
                        _mapping_diagnostic(
                            "invalid_enum_mapping",
                            type_id,
                            "source_literal must be a non-empty string",
                        )
                    )
                else:
                    valid_source_literals.append(member_mapping.source_literal)
            if malformed_member_mapping:
                diagnostics.append(
                    _mapping_diagnostic(
                        "invalid_enum_mapping",
                        type_id,
                        "member_mappings must contain only EnumAbiMemberMapping values",
                    )
                )

            for member_id, count in sorted(Counter(valid_member_ids).items()):
                if count > 1:
                    diagnostics.append(
                        _mapping_diagnostic(
                            "duplicate_enum_local_member",
                            type_id,
                            f"member_mappings contains duplicate local member {member_id!r}",
                        )
                    )
            for source_literal, count in sorted(
                Counter(valid_source_literals).items()
            ):
                if count > 1:
                    diagnostics.append(
                        _mapping_diagnostic(
                            "duplicate_enum_source_literal",
                            type_id,
                            (
                                "member_mappings contains duplicate source literal "
                                f"{source_literal!r}"
                            ),
                        )
                    )

        local_type = named_types.get(type_id) if valid_type_id else None
        if valid_type_id and local_type is None:
            diagnostics.append(
                _mapping_diagnostic(
                    "unknown_enum_mapping_type",
                    type_id,
                    f"local type {type_id!r} is not present in the resolved input",
                )
            )
        elif local_type is not None and local_type.kind != "enum":
            diagnostics.append(
                _mapping_diagnostic(
                    "non_enum_mapping_type",
                    type_id,
                    f"local type {type_id!r} is not an enum",
                )
            )
        elif local_type is not None and member_mappings_are_tuple:
            local_member_ids = {
                member.member_id for member in local_type.enum_members
            }
            mapped_member_ids = set(valid_member_ids)
            missing = sorted(local_member_ids - mapped_member_ids)
            extra = sorted(mapped_member_ids - local_member_ids)
            if missing:
                diagnostics.append(
                    _mapping_diagnostic(
                        "missing_enum_local_member",
                        type_id,
                        "member_mappings is missing local members: "
                        + ", ".join(repr(member_id) for member_id in missing),
                    )
                )
            if extra:
                diagnostics.append(
                    _mapping_diagnostic(
                        "extra_enum_local_member",
                        type_id,
                        "member_mappings has extra local members: "
                        + ", ".join(repr(member_id) for member_id in extra),
                    )
                )

        if isinstance(mapping.source_members, tuple) and member_mappings_are_tuple:
            source_member_set = set(valid_source_members)
            for source_literal in sorted(
                set(valid_source_literals) - source_member_set
            ):
                diagnostics.append(
                    _mapping_diagnostic(
                        "unknown_enum_source_literal",
                        type_id,
                        f"source literal {source_literal!r} is absent from source_members",
                    )
                )

    for type_id, type_mappings in sorted(mappings_by_type.items()):
        if len(type_mappings) > 1:
            diagnostics.append(
                _mapping_diagnostic(
                    "duplicate_enum_mapping",
                    type_id,
                    f"enum type {type_id!r} has multiple mappings",
                )
            )

    validated = {
        type_id: type_mappings[0]
        for type_id, type_mappings in mappings_by_type.items()
    }
    return diagnostics, validated


def _missing_enum_diagnostics(
    resolved: routine_resolution.ResolvedSpecialization,
    mapped_type_ids,
) -> list[ScalarAbiDiagnostic]:
    diagnostics = []
    for parameter in resolved.parameters:
        if (
            parameter.type.kind == "enum"
            and parameter.type.type_id not in mapped_type_ids
        ):
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
        if (
            connector.active
            and connector.type.kind == "enum"
            and connector.type.type_id not in mapped_type_ids
        ):
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


def _projected_type(type_info, enum_mappings):
    if type_info.kind == "enum":
        mapping = enum_mappings[type_info.type_id]
        return ScalarEnumAbiType(mapping.canonical_class_path)
    return _type(type_info)


def _projected_value(value, type_info, enum_mappings):
    if type_info.kind != "enum":
        return cast(bool | int | float, value)
    enum_value = cast(routine_resolution.EnumValue, value)
    mapping = enum_mappings[type_info.type_id]
    member_mapping = next(
        member_mapping
        for member_mapping in mapping.member_mappings
        if member_mapping.member_id == enum_value.member_id
    )
    return ScalarEnumAbiValue(
        mapping.source_members.index(member_mapping.source_literal) + 1
    )


def project_scalar_abi(
    resolved: routine_resolution.ResolvedSpecialization,
    *,
    enum_mappings=(),
) -> ScalarAbiProjection:
    """Project scalar leaves using optional reviewed enum mappings.

    Caller mappings are reviewed internal input. They do not verify source or make a
    public compatibility promise.
    """
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

    diagnostics, validated_mappings = _mapping_diagnostics(resolved, enum_mappings)
    diagnostics.extend(_missing_enum_diagnostics(resolved, validated_mappings))
    if diagnostics:
        raise ScalarAbiError(diagnostics)

    parameters = tuple(
        ScalarParameterAbiRow(
            parameter.parameter_id,
            _coordinates(leaf.coordinates),
            _projected_type(parameter.type, validated_mappings),
            parameter.source,
            _projected_value(leaf.value, parameter.type, validated_mappings),
        )
        for parameter in resolved.parameters
        for leaf in parameter.leaves
    )
    connectors = tuple(
        ScalarConnectorAbiRow(
            connector.connector_id,
            _coordinates(leaf.coordinates),
            _projected_type(connector.type, validated_mappings),
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
