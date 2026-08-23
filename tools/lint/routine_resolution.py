"""Resolve declarative routine contract inputs for internal tests."""

from collections.abc import Mapping
from dataclasses import dataclass, fields
from typing import NoReturn

from tools.lint import routine_schemas


@dataclass(frozen=True, order=True)
class ResolutionDiagnostic:
    code: str
    document: str
    path: str
    message: str

    def __str__(self):
        return f"{self.code}: {self.document}: {self.path}: {self.message}"


class ResolutionError(ValueError):
    def __init__(self, diagnostics):
        self.diagnostics = tuple(sorted(diagnostics))
        super().__init__("\n".join(str(diagnostic) for diagnostic in self.diagnostics))


@dataclass(frozen=True)
class ResolutionLimits:
    max_guard_depth: int = 32
    max_guard_nodes: int = 2048
    max_scalar_leaves: int = 100_000


@dataclass(frozen=True)
class Coordinate:
    dimension_id: str
    member_id: str
    ordinal: int


@dataclass(frozen=True)
class EnumValue:
    type_id: str
    member_id: str
    symbol: str


@dataclass(frozen=True)
class ScalarParameterLeaf:
    coordinates: tuple[Coordinate, ...]
    value: bool | int | float | EnumValue


@dataclass(frozen=True)
class ScalarConnectorLeaf:
    coordinates: tuple[Coordinate, ...]


@dataclass(frozen=True)
class ResolvedEnumMember:
    member_id: str
    symbol: str


@dataclass(frozen=True)
class ResolvedType:
    kind: str
    primitive: str | None = None
    type_id: str | None = None
    quantity: str | None = None
    unit: str | None = None
    display_unit: str | None = None
    enum_members: tuple[ResolvedEnumMember, ...] = ()


@dataclass(frozen=True)
class ResolvedDimension:
    dimension_id: str
    kind: str
    extent: int
    members: tuple[str, ...]


@dataclass(frozen=True)
class ResolvedParameter:
    parameter_id: str
    type: ResolvedType
    dimension_ids: tuple[str, ...]
    source: str
    leaves: tuple[ScalarParameterLeaf, ...]


@dataclass(frozen=True)
class ResolvedConnector:
    connector_id: str
    direction: str
    type: ResolvedType
    dimension_ids: tuple[str, ...]
    active: bool
    guard_result: bool | None
    leaves: tuple[ScalarConnectorLeaf, ...]


@dataclass(frozen=True)
class ResolvedSpecialization:
    canonical_id: str
    revision: int
    dimensions: tuple[ResolvedDimension, ...]
    parameters: tuple[ResolvedParameter, ...]
    connectors: tuple[ResolvedConnector, ...]


def _raise(code, document, path, message) -> NoReturn:
    raise ResolutionError((ResolutionDiagnostic(code, document, path, message),))


def _validate_limits(limits):
    if not isinstance(limits, ResolutionLimits):
        _raise(
            "invalid_limit",
            "resolution",
            "$.limits",
            "limits must be a ResolutionLimits value",
        )
    diagnostics = []
    for field in fields(ResolutionLimits):
        value = getattr(limits, field.name)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            diagnostics.append(
                ResolutionDiagnostic(
                    "invalid_limit",
                    "resolution",
                    f"$.limits.{field.name}",
                    "limit must be a non-negative Integer",
                )
            )
    if diagnostics:
        raise ResolutionError(diagnostics)


def _guard_roots(interface):
    if not isinstance(interface, Mapping):
        return
    connectors = interface.get("connectors")
    if not isinstance(connectors, list):
        return
    for connector_index, connector in enumerate(connectors):
        if not isinstance(connector, Mapping):
            continue
        presence = connector.get("presence")
        if not isinstance(presence, Mapping) or "guard" not in presence:
            continue
        yield (
            presence["guard"],
            f"$.connectors[{connector_index}].presence.guard",
        )


def _preflight_guards(interface, limits):
    node_count = 0
    for guard, root_path in _guard_roots(interface):
        stack = [(guard, 1, root_path)]
        while stack:
            node, depth, path = stack.pop()
            node_count += 1
            if node_count > limits.max_guard_nodes:
                return ResolutionDiagnostic(
                    "resource_limit",
                    "interface",
                    path,
                    f"guard node count exceeds limit {limits.max_guard_nodes}",
                )
            if depth > limits.max_guard_depth:
                return ResolutionDiagnostic(
                    "resource_limit",
                    "interface",
                    path,
                    f"guard depth {depth} exceeds limit {limits.max_guard_depth}",
                )
            if not isinstance(node, Mapping):
                continue
            operator = node.get("op")
            children = []
            if operator in ("and", "or") and isinstance(node.get("operands"), list):
                operands = node["operands"]
                if (
                    node_count + len(stack) + len(operands)
                    > limits.max_guard_nodes
                ):
                    return ResolutionDiagnostic(
                        "resource_limit",
                        "interface",
                        path,
                        f"guard node count exceeds limit {limits.max_guard_nodes}",
                    )
                children = [
                    (operands[index], f"{path}.operands[{index}]")
                    for index in range(len(operands))
                ]
            elif operator == "not" and "operand" in node:
                children = [(node["operand"], f"{path}.operand")]
            if node_count + len(stack) + len(children) > limits.max_guard_nodes:
                return ResolutionDiagnostic(
                    "resource_limit",
                    "interface",
                    path,
                    f"guard node count exceeds limit {limits.max_guard_nodes}",
                )
            for child, child_path in reversed(children):
                stack.append((child, depth + 1, child_path))
    return None


def _messages_to_diagnostics(code, messages):
    diagnostics = []
    for message in messages:
        document = "contract"
        path = "$"
        detail = message
        for label in ("interface", "specialization"):
            prefix = f"{label}: "
            if not message.startswith(prefix):
                continue
            document = label
            detail = message[len(prefix) :]
            if detail.startswith("$") and ": " in detail:
                path, detail = detail.split(": ", 1)
            break
        diagnostics.append(ResolutionDiagnostic(code, document, path, detail))
    return diagnostics


def _validated_model(interface, specialization):
    contract_errors = []
    schemas_by_id, registry = routine_schemas._load_schemas(
        routine_schemas.REPO_ROOT, contract_errors
    )
    if schemas_by_id is None or registry is None:
        raise ResolutionError(
            _messages_to_diagnostics("schema_contract", sorted(contract_errors))
        )

    schema_errors = []
    routine_schemas._check_schema_instance(
        interface,
        routine_schemas.INTERFACE_ID,
        "interface",
        schemas_by_id,
        registry,
        schema_errors,
    )
    routine_schemas._check_schema_instance(
        specialization,
        routine_schemas.SPECIALIZATION_ID,
        "specialization",
        schemas_by_id,
        registry,
        schema_errors,
    )
    if schema_errors:
        raise ResolutionError(
            _messages_to_diagnostics("schema", sorted(schema_errors))
        )

    semantic_errors = []
    routine_schemas._check_interface_specialization_agreement(
        interface, specialization, semantic_errors
    )
    model = routine_schemas._check_interface_and_specialization(
        interface,
        specialization,
        semantic_errors,
        interface_label="interface",
        specialization_label="specialization",
    )
    if semantic_errors:
        raise ResolutionError(
            _messages_to_diagnostics("semantic", sorted(semantic_errors))
        )
    return model


def _dimension_members(interface, model):
    members = {}
    for dimension in interface["dimensions"]:
        dimension_id = dimension["id"]
        if dimension["extent"]["kind"] == "fixed":
            members[dimension_id] = dimension["members"]
        else:
            members[dimension_id] = model.specialization_members[dimension_id]
    return members


def _resolved_type(type_use, model):
    if type_use["kind"] == "primitive":
        return ResolvedType(kind="primitive", primitive=type_use["primitive"])
    type_id = type_use["type"]
    definition = model.types[type_id]
    if definition["kind"] == "alias":
        return ResolvedType(
            kind="alias",
            primitive=definition["primitive"],
            type_id=type_id,
            quantity=definition.get("quantity"),
            unit=definition.get("unit"),
            display_unit=definition.get("display_unit"),
        )
    return ResolvedType(
        kind="enum",
        type_id=type_id,
        enum_members=tuple(
            ResolvedEnumMember(member["id"], member["symbol"])
            for member in definition["members"]
        ),
    )


def _enum_value(value, type_info, model):
    if type_info[0] != "enum":
        return value
    type_id = type_info[1]
    definition = model.types[type_id]
    member = next(member for member in definition["members"] if member["id"] == value)
    return EnumValue(type_id, member["id"], member["symbol"])


def _coordinates(shape, dimension_members):
    if shape["kind"] == "scalar":
        yield ()
        return
    dimension_ids = shape["dimensions"]
    first_id = dimension_ids[0]
    for first_ordinal, first_member in enumerate(dimension_members[first_id]):
        first = Coordinate(first_id, first_member, first_ordinal)
        if len(dimension_ids) == 1:
            yield (first,)
            continue
        second_id = dimension_ids[1]
        for second_ordinal, second_member in enumerate(dimension_members[second_id]):
            yield (
                first,
                Coordinate(second_id, second_member, second_ordinal),
            )


def _parameter_leaves(parameter, value, type_info, dimension_members, model):
    shape = parameter["shape"]
    if shape["kind"] == "scalar":
        return (ScalarParameterLeaf((), _enum_value(value, type_info, model)),)
    coordinates = _coordinates(shape, dimension_members)
    if len(shape["dimensions"]) == 1:
        return tuple(
            ScalarParameterLeaf(coordinate, _enum_value(item, type_info, model))
            for coordinate, item in zip(coordinates, value, strict=True)
        )
    flat_values = (item for row in value for item in row)
    return tuple(
        ScalarParameterLeaf(coordinate, _enum_value(item, type_info, model))
        for coordinate, item in zip(coordinates, flat_values, strict=True)
    )


def _guard_operand(operand, model):
    if operand["kind"] == "parameter":
        parameter_id = operand["parameter"]
        return model.effective_values[parameter_id]
    return operand["value"]


def _evaluate_guard(guard, model):
    operator = guard["op"]
    if operator == "and":
        return all(_evaluate_guard(operand, model) for operand in guard["operands"])
    if operator == "or":
        return any(_evaluate_guard(operand, model) for operand in guard["operands"])
    if operator == "not":
        return not _evaluate_guard(guard["operand"], model)
    left = _guard_operand(guard["left"], model)
    right = _guard_operand(guard["right"], model)
    if operator == "eq":
        return left == right
    if operator == "ne":
        return left != right
    if operator == "lt":
        return left < right
    if operator == "lte":
        return left <= right
    if operator == "gt":
        return left > right
    return left >= right


def _leaf_count(shape, model):
    if shape["kind"] == "scalar":
        return 1
    count = 1
    for dimension_id in shape["dimensions"]:
        count *= model.concrete_dimensions[dimension_id]
    return count


def _dimension_ids(shape):
    if shape["kind"] == "scalar":
        return ()
    return tuple(shape["dimensions"])


def _connector_states(interface, model):
    states = []
    for connector in interface["connectors"]:
        presence = connector["presence"]
        if presence["kind"] == "always":
            states.append((True, None))
        else:
            result = _evaluate_guard(presence["guard"], model)
            states.append((result, result))
    return states


def _preflight_scalar_leaves(interface, model, connector_states, limit):
    count = sum(_leaf_count(parameter["shape"], model) for parameter in interface["parameters"])
    count += sum(
        _leaf_count(connector["shape"], model)
        for connector, (active, _) in zip(
            interface["connectors"], connector_states, strict=True
        )
        if active
    )
    if count > limit:
        _raise(
            "resource_limit",
            "resolution",
            "$",
            f"scalar leaf expansion {count} exceeds limit {limit}",
        )


def _resolve(interface, specialization, limits):
    model = _validated_model(interface, specialization)
    dimension_members = _dimension_members(interface, model)
    connector_states = _connector_states(interface, model)
    _preflight_scalar_leaves(interface, model, connector_states, limits.max_scalar_leaves)

    dimensions = tuple(
        ResolvedDimension(
            dimension["id"],
            dimension["extent"]["kind"],
            model.concrete_dimensions[dimension["id"]],
            tuple(dimension_members[dimension["id"]]),
        )
        for dimension in interface["dimensions"]
    )
    parameters = tuple(
        ResolvedParameter(
            parameter["id"],
            _resolved_type(parameter["type"], model),
            _dimension_ids(parameter["shape"]),
            "assignment" if parameter["id"] in model.assignments else "default",
            _parameter_leaves(
                parameter,
                model.effective_values[parameter["id"]],
                model.parameter_types[parameter["id"]],
                dimension_members,
                model,
            ),
        )
        for parameter in interface["parameters"]
    )
    connectors = tuple(
        ResolvedConnector(
            connector["id"],
            connector["direction"],
            _resolved_type(connector["type"], model),
            _dimension_ids(connector["shape"]),
            active,
            guard_result,
            (
                tuple(
                    ScalarConnectorLeaf(coordinate)
                    for coordinate in _coordinates(
                        connector["shape"], dimension_members
                    )
                )
                if active
                else ()
            ),
        )
        for connector, (active, guard_result) in zip(
            interface["connectors"], connector_states, strict=True
        )
    )
    return ResolvedSpecialization(
        interface["canonical_id"],
        interface["revision"],
        dimensions,
        parameters,
        connectors,
    )


def resolve_specialization(
    interface, specialization, *, limits=ResolutionLimits()
) -> ResolvedSpecialization:
    """Resolve one governed interface-v3 and specialization-v1 pair."""
    _validate_limits(limits)
    guard_limit = _preflight_guards(interface, limits)
    if guard_limit is not None:
        raise ResolutionError((guard_limit,))
    try:
        return _resolve(interface, specialization, limits)
    except ResolutionError:
        raise
    except RecursionError:
        _raise(
            "resource_limit",
            "interface",
            "$",
            "guard nesting exceeds safe validation depth",
        )
    except (
        AttributeError,
        IndexError,
        KeyError,
        OverflowError,
        StopIteration,
        TypeError,
        ValueError,
    ):
        _raise(
            "invalid_input",
            "resolution",
            "$",
            "input could not be resolved after validation",
        )
