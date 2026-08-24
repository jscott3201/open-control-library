//! Typed in-memory projection from resolved routine leaves to a scalar ABI.
//!
//! Enum mappings are caller-reviewed claims. This module copies their class paths
//! and source-member ordinals without reading or verifying source files.

use std::collections::{HashMap, HashSet};
use std::fmt;

use num_bigint::BigInt;

use crate::resolution::{
    ConnectorDirection, Coordinate, FiniteReal, ParameterSource, PrimitiveType, ResolvedDimension,
    ResolvedEnumMember, ResolvedParameter, ResolvedScalarValue, ResolvedSpecialization,
    ResolvedType,
};

/// Maps one local enum member to a literal in [`EnumAbiMapping::source_members`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumAbiMemberMapping {
    pub member_id: String,
    pub source_literal: String,
}

/// Caller-reviewed mapping from one local enum to a source enum class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumAbiMapping {
    pub type_id: String,
    /// Class path copied into projected enum types without source verification.
    pub canonical_class_path: String,
    /// Source literals in ABI order. Positions become one-based ordinals.
    pub source_members: Vec<String>,
    /// Complete mapping of local members; order has no ABI meaning.
    pub member_mappings: Vec<EnumAbiMemberMapping>,
}

/// One coordinate copied from a resolved row-major leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarCoordinate {
    pub dimension_id: String,
    pub member_id: String,
    /// Zero-based authored position within the dimension.
    pub ordinal: usize,
}

/// Type attached to one projected scalar row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalarAbiType {
    Primitive(PrimitiveType),
    /// Named primitive alias with detached engineering metadata.
    Alias {
        type_id: String,
        primitive: PrimitiveType,
        quantity: Option<String>,
        unit: Option<String>,
        display_unit: Option<String>,
    },
    /// Enum projected through a caller-supplied, unverified mapping.
    Enum {
        canonical_class_path: String,
    },
}

/// Value attached to one projected parameter row.
#[derive(Clone, Debug, PartialEq)]
pub enum ScalarAbiValue {
    Boolean(bool),
    /// Arbitrary-precision integer, including integer-valued `Real` inputs.
    Integer(BigInt),
    /// Original binary64 bits, including signed zero.
    Real(FiniteReal),
    /// One-based position in [`EnumAbiMapping::source_members`].
    Enum {
        ordinal: usize,
    },
}

/// One projected parameter leaf.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarParameterAbiRow {
    pub parameter_id: String,
    /// Empty for a scalar; otherwise the resolved row-major coordinates.
    pub coordinates: Vec<ScalarCoordinate>,
    pub abi_type: ScalarAbiType,
    pub source: ParameterSource,
    pub value: ScalarAbiValue,
}

/// One projected active-connector leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarConnectorAbiRow {
    pub connector_id: String,
    /// Empty for a scalar; otherwise the resolved row-major coordinates.
    pub coordinates: Vec<ScalarCoordinate>,
    pub abi_type: ScalarAbiType,
    pub direction: ConnectorDirection,
}

/// Detached scalar rows for one resolved specialization.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarAbiProjection {
    pub canonical_id: String,
    /// Arbitrary-precision positive revision copied without narrowing.
    pub revision: BigInt,
    /// Parameter rows in authored parameter and resolved leaf order.
    pub parameters: Vec<ScalarParameterAbiRow>,
    /// Active connector rows in authored connector and resolved leaf order.
    pub connectors: Vec<ScalarConnectorAbiRow>,
}

/// One sortable projection or enum-mapping failure.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScalarAbiDiagnostic {
    /// Stable machine-readable category.
    pub code: String,
    /// `projection`, `mapping`, `dimension`, `parameter`, or `connector`.
    pub owner_kind: String,
    /// Stable owner ID, or `$` when no valid ID is available.
    pub owner_id: String,
    /// Related local type ID, empty when the failure has no type owner.
    pub type_id: String,
    pub message: String,
}

impl fmt::Display for ScalarAbiDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} {}: {}",
            self.code, self.owner_kind, self.owner_id, self.message
        )
    }
}

/// Atomic projection failure with diagnostics sorted by all public fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarAbiError {
    pub diagnostics: Vec<ScalarAbiDiagnostic>,
}

impl ScalarAbiError {
    fn new(mut diagnostics: Vec<ScalarAbiDiagnostic>) -> Self {
        diagnostics.sort();
        Self { diagnostics }
    }
}

impl fmt::Display for ScalarAbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            write!(formatter, "{diagnostic}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ScalarAbiError {}

fn diagnostic(
    code: &str,
    owner_kind: &str,
    owner_id: &str,
    type_id: &str,
    message: impl Into<String>,
) -> ScalarAbiDiagnostic {
    ScalarAbiDiagnostic {
        code: code.to_owned(),
        owner_kind: owner_kind.to_owned(),
        owner_id: if owner_id.is_empty() {
            "$".to_owned()
        } else {
            owner_id.to_owned()
        },
        type_id: type_id.to_owned(),
        message: message.into(),
    }
}

fn invalid_resolved(
    owner_kind: &str,
    owner_id: &str,
    type_id: &str,
    message: impl Into<String>,
) -> ScalarAbiDiagnostic {
    diagnostic(
        "invalid_resolved_input",
        owner_kind,
        owner_id,
        type_id,
        message,
    )
}

fn mapping_diagnostic(
    code: &str,
    type_id: &str,
    message: impl Into<String>,
) -> ScalarAbiDiagnostic {
    diagnostic(code, "mapping", type_id, type_id, message)
}

fn validate_optional_metadata(
    value: &Option<String>,
    label: &str,
    owner_kind: &str,
    owner_id: &str,
    type_id: &str,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    if let Some(value) = value
        && (value.is_empty() || value.trim() != value)
    {
        diagnostics.push(invalid_resolved(
            owner_kind,
            owner_id,
            type_id,
            format!("{label} must be nonempty and trimmed"),
        ));
    }
}

fn validate_enum_members(
    type_id: &str,
    members: &[ResolvedEnumMember],
    owner_kind: &str,
    owner_id: &str,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    if members.is_empty() {
        diagnostics.push(invalid_resolved(
            owner_kind,
            owner_id,
            type_id,
            format!("enum `{type_id}` must have at least one member"),
        ));
    }
    let mut member_ids = HashSet::new();
    let mut symbols = HashSet::new();
    for member in members {
        if member.member_id.is_empty() {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                "enum member ID must not be empty",
            ));
        } else if !member_ids.insert(member.member_id.as_str()) {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!(
                    "enum `{type_id}` contains duplicate member ID `{}`",
                    member.member_id
                ),
            ));
        }
        if member.symbol.is_empty() {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                "enum member symbol must not be empty",
            ));
        } else if !symbols.insert(member.symbol.as_str()) {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!(
                    "enum `{type_id}` contains duplicate symbol `{}`",
                    member.symbol
                ),
            ));
        }
    }
}

fn validate_named_type(
    resolved_type: &ResolvedType,
    owner_kind: &str,
    owner_id: &str,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    match resolved_type {
        ResolvedType::Primitive(_) => {}
        ResolvedType::Alias {
            type_id,
            quantity,
            unit,
            display_unit,
            ..
        } => {
            if type_id.is_empty() {
                diagnostics.push(invalid_resolved(
                    owner_kind,
                    owner_id,
                    type_id,
                    "alias type ID must not be empty",
                ));
            }
            validate_optional_metadata(
                quantity,
                "alias quantity",
                owner_kind,
                owner_id,
                type_id,
                diagnostics,
            );
            validate_optional_metadata(
                unit,
                "alias unit",
                owner_kind,
                owner_id,
                type_id,
                diagnostics,
            );
            validate_optional_metadata(
                display_unit,
                "alias display unit",
                owner_kind,
                owner_id,
                type_id,
                diagnostics,
            );
        }
        ResolvedType::Enum { type_id, members } => {
            if type_id.is_empty() {
                diagnostics.push(invalid_resolved(
                    owner_kind,
                    owner_id,
                    type_id,
                    "enum type ID must not be empty",
                ));
            }
            validate_enum_members(type_id, members, owner_kind, owner_id, diagnostics);
        }
    }
}

fn named_type_id(resolved_type: &ResolvedType) -> Option<&str> {
    match resolved_type {
        ResolvedType::Primitive(_) => None,
        ResolvedType::Alias { type_id, .. } | ResolvedType::Enum { type_id, .. } => Some(type_id),
    }
}

fn register_named_type<'a>(
    resolved_type: &'a ResolvedType,
    owner_kind: &str,
    owner_id: &str,
    named_types: &mut HashMap<&'a str, &'a ResolvedType>,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    let Some(type_id) = named_type_id(resolved_type) else {
        return;
    };
    if type_id.is_empty() {
        validate_named_type(resolved_type, owner_kind, owner_id, diagnostics);
        return;
    }
    if let Some(previous) = named_types.get(type_id) {
        if *previous != resolved_type {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!("named type `{type_id}` has inconsistent resolved definitions"),
            ));
            validate_named_type(resolved_type, owner_kind, owner_id, diagnostics);
        }
    } else {
        named_types.insert(type_id, resolved_type);
        validate_named_type(resolved_type, owner_kind, owner_id, diagnostics);
    }
}

fn validate_dimensions<'a>(
    resolved: &'a ResolvedSpecialization,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) -> HashMap<&'a str, &'a ResolvedDimension> {
    let mut dimensions = HashMap::new();
    let mut stable_members = HashMap::new();
    for dimension in &resolved.dimensions {
        let dimension_id = dimension.dimension_id.as_str();
        if dimension_id.is_empty() {
            diagnostics.push(invalid_resolved(
                "dimension",
                dimension_id,
                "",
                "dimension ID must not be empty",
            ));
        } else if dimensions.insert(dimension_id, dimension).is_some() {
            diagnostics.push(invalid_resolved(
                "dimension",
                dimension_id,
                "",
                format!("duplicate dimension ID `{dimension_id}`"),
            ));
        }
        if dimension.extent != dimension.members.len() {
            diagnostics.push(invalid_resolved(
                "dimension",
                dimension_id,
                "",
                format!(
                    "extent {} does not match {} members",
                    dimension.extent,
                    dimension.members.len()
                ),
            ));
        }
        if dimension.members.is_empty() {
            diagnostics.push(invalid_resolved(
                "dimension",
                dimension_id,
                "",
                "dimension must have at least one member",
            ));
        }
        let mut local_members = HashSet::new();
        for member_id in &dimension.members {
            if member_id.is_empty() {
                diagnostics.push(invalid_resolved(
                    "dimension",
                    dimension_id,
                    "",
                    "dimension member ID must not be empty",
                ));
                continue;
            }
            if !local_members.insert(member_id.as_str()) {
                diagnostics.push(invalid_resolved(
                    "dimension",
                    dimension_id,
                    "",
                    format!("dimension contains duplicate member ID `{member_id}`"),
                ));
            }
            if let Some(previous_dimension) =
                stable_members.insert(member_id.as_str(), dimension_id)
            {
                diagnostics.push(invalid_resolved(
                    "dimension",
                    dimension_id,
                    "",
                    format!(
                        "stable member ID `{member_id}` is also used by dimension `{previous_dimension}`"
                    ),
                ));
            }
        }
    }
    dimensions
}

struct ValidatedShape<'a> {
    dimensions: Vec<&'a ResolvedDimension>,
    leaf_count: usize,
}

fn validate_shape<'a>(
    owner_kind: &str,
    owner_id: &str,
    type_id: &str,
    dimension_ids: &[String],
    dimensions: &HashMap<&'a str, &'a ResolvedDimension>,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) -> Option<ValidatedShape<'a>> {
    if dimension_ids.len() > 2 {
        diagnostics.push(invalid_resolved(
            owner_kind,
            owner_id,
            type_id,
            format!(
                "resolved rank {} exceeds the supported rank of two",
                dimension_ids.len()
            ),
        ));
    }
    let mut resolved_dimensions = Vec::new();
    let mut valid = dimension_ids.len() <= 2;
    for dimension_id in dimension_ids {
        if dimension_id.is_empty() {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                "shape dimension ID must not be empty",
            ));
            valid = false;
        } else if let Some(dimension) = dimensions.get(dimension_id.as_str()) {
            if dimension.extent == 0 || dimension.extent != dimension.members.len() {
                valid = false;
            }
            resolved_dimensions.push(*dimension);
        } else {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!("shape references unknown dimension `{dimension_id}`"),
            ));
            valid = false;
        }
    }
    if !valid {
        return None;
    }
    let mut leaf_count = 1_usize;
    for dimension in &resolved_dimensions {
        let Some(count) = leaf_count.checked_mul(dimension.extent) else {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                "resolved scalar leaf count overflows usize",
            ));
            return None;
        };
        leaf_count = count;
    }
    Some(ValidatedShape {
        dimensions: resolved_dimensions,
        leaf_count,
    })
}

fn expected_ordinal(shape: &ValidatedShape<'_>, leaf_index: usize, axis: usize) -> Option<usize> {
    if leaf_index >= shape.leaf_count {
        return None;
    }
    match shape.dimensions.len() {
        0 => None,
        1 => Some(leaf_index),
        2 => {
            let columns = shape.dimensions.get(1)?.extent;
            if columns == 0 {
                return None;
            }
            match axis {
                0 => Some(leaf_index / columns),
                1 => Some(leaf_index % columns),
                _ => None,
            }
        }
        _ => None,
    }
}

fn validate_coordinates(
    owner_kind: &str,
    owner_id: &str,
    type_id: &str,
    leaf_index: usize,
    coordinates: &[Coordinate],
    shape: Option<&ValidatedShape<'_>>,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    let Some(shape) = shape else {
        for coordinate in coordinates {
            if coordinate.dimension_id.is_empty() || coordinate.member_id.is_empty() {
                diagnostics.push(invalid_resolved(
                    owner_kind,
                    owner_id,
                    type_id,
                    format!("leaf {leaf_index} contains an empty coordinate identity"),
                ));
            }
        }
        return;
    };
    if coordinates.len() != shape.dimensions.len() {
        diagnostics.push(invalid_resolved(
            owner_kind,
            owner_id,
            type_id,
            format!(
                "leaf {leaf_index} has {} coordinates, expected {}",
                coordinates.len(),
                shape.dimensions.len()
            ),
        ));
    }
    for (axis, coordinate) in coordinates.iter().enumerate() {
        if coordinate.dimension_id.is_empty() || coordinate.member_id.is_empty() {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!("leaf {leaf_index} contains an empty coordinate identity"),
            ));
        }
        let Some(dimension) = shape.dimensions.get(axis) else {
            continue;
        };
        if coordinate.dimension_id != dimension.dimension_id {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!(
                    "leaf {leaf_index} axis {axis} names dimension `{}`, expected `{}`",
                    coordinate.dimension_id, dimension.dimension_id
                ),
            ));
        }
        match dimension.members.get(coordinate.ordinal) {
            Some(member_id) if member_id == &coordinate.member_id => {}
            Some(member_id) => diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!(
                    "leaf {leaf_index} axis {axis} names member `{}`, expected `{member_id}` at ordinal {}",
                    coordinate.member_id, coordinate.ordinal
                ),
            )),
            None => diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!(
                    "leaf {leaf_index} axis {axis} ordinal {} is outside dimension `{}`",
                    coordinate.ordinal, dimension.dimension_id
                ),
            )),
        }
        if let Some(expected) = expected_ordinal(shape, leaf_index, axis)
            && coordinate.ordinal != expected
        {
            diagnostics.push(invalid_resolved(
                owner_kind,
                owner_id,
                type_id,
                format!(
                    "leaf {leaf_index} axis {axis} has ordinal {}, expected {expected} for row-major order",
                    coordinate.ordinal
                ),
            ));
        }
    }
}

fn primitive_accepts_value(primitive: PrimitiveType, value: &ResolvedScalarValue) -> bool {
    matches!(
        (primitive, value),
        (PrimitiveType::Boolean, ResolvedScalarValue::Boolean(_))
            | (PrimitiveType::Integer, ResolvedScalarValue::Integer(_))
            | (
                PrimitiveType::Real,
                ResolvedScalarValue::Integer(_) | ResolvedScalarValue::Real(_)
            )
    )
}

fn validate_parameter_value(
    parameter: &ResolvedParameter,
    leaf_index: usize,
    value: &ResolvedScalarValue,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    let owner_id = parameter.parameter_id.as_str();
    let type_id = named_type_id(&parameter.resolved_type).unwrap_or("");
    match &parameter.resolved_type {
        ResolvedType::Primitive(primitive) | ResolvedType::Alias { primitive, .. } => {
            if !primitive_accepts_value(*primitive, value) {
                diagnostics.push(invalid_resolved(
                    "parameter",
                    owner_id,
                    type_id,
                    format!(
                        "leaf {leaf_index} value does not match resolved primitive {primitive:?}"
                    ),
                ));
            }
        }
        ResolvedType::Enum {
            type_id: expected_type,
            members,
        } => {
            let ResolvedScalarValue::Enum(value) = value else {
                diagnostics.push(invalid_resolved(
                    "parameter",
                    owner_id,
                    expected_type,
                    format!("leaf {leaf_index} value is not an enum"),
                ));
                return;
            };
            if value.type_id != *expected_type {
                diagnostics.push(invalid_resolved(
                    "parameter",
                    owner_id,
                    expected_type,
                    format!(
                        "leaf {leaf_index} enum type `{}` does not match `{expected_type}`",
                        value.type_id
                    ),
                ));
            }
            if value.member_id.is_empty() || value.symbol.is_empty() {
                diagnostics.push(invalid_resolved(
                    "parameter",
                    owner_id,
                    expected_type,
                    format!("leaf {leaf_index} enum identity must not be empty"),
                ));
            }
            match members
                .iter()
                .find(|member| member.member_id == value.member_id)
            {
                Some(member) if member.symbol == value.symbol => {}
                Some(member) => diagnostics.push(invalid_resolved(
                    "parameter",
                    owner_id,
                    expected_type,
                    format!(
                        "leaf {leaf_index} enum symbol `{}` does not match `{}` for member `{}`",
                        value.symbol, member.symbol, value.member_id
                    ),
                )),
                None => diagnostics.push(invalid_resolved(
                    "parameter",
                    owner_id,
                    expected_type,
                    format!(
                        "leaf {leaf_index} names unknown member `{}` for enum `{expected_type}`",
                        value.member_id
                    ),
                )),
            }
        }
    }
}

fn validate_parameters<'a>(
    resolved: &'a ResolvedSpecialization,
    dimensions: &HashMap<&str, &ResolvedDimension>,
    named_types: &mut HashMap<&'a str, &'a ResolvedType>,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    let mut parameter_ids = HashSet::new();
    for parameter in &resolved.parameters {
        let owner_id = parameter.parameter_id.as_str();
        if owner_id.is_empty() {
            diagnostics.push(invalid_resolved(
                "parameter",
                owner_id,
                "",
                "parameter ID must not be empty",
            ));
        } else if !parameter_ids.insert(owner_id) {
            diagnostics.push(invalid_resolved(
                "parameter",
                owner_id,
                "",
                format!("duplicate parameter ID `{owner_id}`"),
            ));
        }
        register_named_type(
            &parameter.resolved_type,
            "parameter",
            owner_id,
            named_types,
            diagnostics,
        );
        let type_id = named_type_id(&parameter.resolved_type).unwrap_or("");
        let shape = validate_shape(
            "parameter",
            owner_id,
            type_id,
            &parameter.dimension_ids,
            dimensions,
            diagnostics,
        );
        if let Some(shape) = &shape
            && parameter.leaves.len() != shape.leaf_count
        {
            diagnostics.push(invalid_resolved(
                "parameter",
                owner_id,
                type_id,
                format!(
                    "has {} leaves, expected {} from its dimensions",
                    parameter.leaves.len(),
                    shape.leaf_count
                ),
            ));
        }
        for (leaf_index, leaf) in parameter.leaves.iter().enumerate() {
            validate_coordinates(
                "parameter",
                owner_id,
                type_id,
                leaf_index,
                &leaf.coordinates,
                shape.as_ref(),
                diagnostics,
            );
            validate_parameter_value(parameter, leaf_index, &leaf.value, diagnostics);
        }
    }
}

fn validate_connectors<'a>(
    resolved: &'a ResolvedSpecialization,
    dimensions: &HashMap<&str, &ResolvedDimension>,
    named_types: &mut HashMap<&'a str, &'a ResolvedType>,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    let mut connector_ids = HashSet::new();
    for connector in &resolved.connectors {
        let owner_id = connector.connector_id.as_str();
        if owner_id.is_empty() {
            diagnostics.push(invalid_resolved(
                "connector",
                owner_id,
                "",
                "connector ID must not be empty",
            ));
        } else if !connector_ids.insert(owner_id) {
            diagnostics.push(invalid_resolved(
                "connector",
                owner_id,
                "",
                format!("duplicate connector ID `{owner_id}`"),
            ));
        }
        register_named_type(
            &connector.resolved_type,
            "connector",
            owner_id,
            named_types,
            diagnostics,
        );
        let type_id = named_type_id(&connector.resolved_type).unwrap_or("");
        if connector
            .guard_result
            .is_some_and(|result| result != connector.active)
        {
            diagnostics.push(invalid_resolved(
                "connector",
                owner_id,
                type_id,
                "guard result contradicts active state",
            ));
        }
        if !connector.active && connector.guard_result.is_none() {
            diagnostics.push(invalid_resolved(
                "connector",
                owner_id,
                type_id,
                "inactive connector has no false guard result",
            ));
        }
        let shape = validate_shape(
            "connector",
            owner_id,
            type_id,
            &connector.dimension_ids,
            dimensions,
            diagnostics,
        );
        if connector.active {
            if let Some(shape) = &shape
                && connector.leaves.len() != shape.leaf_count
            {
                diagnostics.push(invalid_resolved(
                    "connector",
                    owner_id,
                    type_id,
                    format!(
                        "has {} leaves, expected {} from its dimensions",
                        connector.leaves.len(),
                        shape.leaf_count
                    ),
                ));
            }
        } else if !connector.leaves.is_empty() {
            diagnostics.push(invalid_resolved(
                "connector",
                owner_id,
                type_id,
                format!(
                    "inactive connector has {} leaves, expected none",
                    connector.leaves.len()
                ),
            ));
        }
        for (leaf_index, leaf) in connector.leaves.iter().enumerate() {
            validate_coordinates(
                "connector",
                owner_id,
                type_id,
                leaf_index,
                &leaf.coordinates,
                shape.as_ref(),
                diagnostics,
            );
        }
    }
}

fn validate_mapping(
    mapping: &EnumAbiMapping,
    named_types: &HashMap<&str, &ResolvedType>,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    let type_id = mapping.type_id.as_str();
    let valid_type_id = !type_id.is_empty();
    if !valid_type_id {
        diagnostics.push(mapping_diagnostic(
            "invalid_enum_mapping",
            type_id,
            "type_id must be a non-empty string",
        ));
    }
    if mapping.canonical_class_path.is_empty() {
        diagnostics.push(mapping_diagnostic(
            "invalid_enum_mapping",
            type_id,
            "canonical_class_path must be a non-empty string",
        ));
    }

    if mapping.source_members.is_empty() {
        diagnostics.push(mapping_diagnostic(
            "invalid_enum_mapping",
            type_id,
            "source_members must not be empty",
        ));
    }
    let mut source_member_counts = HashMap::new();
    for source_literal in &mapping.source_members {
        if source_literal.is_empty() {
            diagnostics.push(mapping_diagnostic(
                "invalid_enum_mapping",
                type_id,
                "source_members must contain only non-empty strings",
            ));
        } else {
            *source_member_counts
                .entry(source_literal.as_str())
                .or_insert(0) += 1;
        }
    }
    for (source_literal, count) in source_member_counts {
        if count > 1 {
            diagnostics.push(mapping_diagnostic(
                "duplicate_enum_source_member",
                type_id,
                format!("source_members contains duplicate literal `{source_literal}`"),
            ));
        }
    }

    let mut member_id_counts = HashMap::new();
    let mut source_literal_counts = HashMap::new();
    for member_mapping in &mapping.member_mappings {
        if member_mapping.member_id.is_empty() {
            diagnostics.push(mapping_diagnostic(
                "invalid_enum_mapping",
                type_id,
                "member_id must be a non-empty string",
            ));
        } else {
            *member_id_counts
                .entry(member_mapping.member_id.as_str())
                .or_insert(0) += 1;
        }
        if member_mapping.source_literal.is_empty() {
            diagnostics.push(mapping_diagnostic(
                "invalid_enum_mapping",
                type_id,
                "source_literal must be a non-empty string",
            ));
        } else {
            *source_literal_counts
                .entry(member_mapping.source_literal.as_str())
                .or_insert(0) += 1;
        }
    }
    for (member_id, count) in &member_id_counts {
        if *count > 1 {
            diagnostics.push(mapping_diagnostic(
                "duplicate_enum_local_member",
                type_id,
                format!("member_mappings contains duplicate local member `{member_id}`"),
            ));
        }
    }
    for (source_literal, count) in &source_literal_counts {
        if *count > 1 {
            diagnostics.push(mapping_diagnostic(
                "duplicate_enum_source_literal",
                type_id,
                format!("member_mappings contains duplicate source literal `{source_literal}`"),
            ));
        }
    }

    let local_type = valid_type_id.then(|| named_types.get(type_id)).flatten();
    match local_type {
        None if valid_type_id => diagnostics.push(mapping_diagnostic(
            "unknown_enum_mapping_type",
            type_id,
            format!("local type `{type_id}` is not present in the resolved input"),
        )),
        Some(ResolvedType::Alias { .. }) | Some(ResolvedType::Primitive(_)) => {
            diagnostics.push(mapping_diagnostic(
                "non_enum_mapping_type",
                type_id,
                format!("local type `{type_id}` is not an enum"),
            ));
        }
        Some(ResolvedType::Enum { members, .. }) => {
            let local_member_ids = members
                .iter()
                .filter_map(|member| {
                    (!member.member_id.is_empty()).then_some(member.member_id.as_str())
                })
                .collect::<HashSet<_>>();
            let mapped_member_ids = member_id_counts.keys().copied().collect::<HashSet<_>>();
            let mut missing = local_member_ids
                .difference(&mapped_member_ids)
                .copied()
                .collect::<Vec<_>>();
            let mut extra = mapped_member_ids
                .difference(&local_member_ids)
                .copied()
                .collect::<Vec<_>>();
            missing.sort_unstable();
            extra.sort_unstable();
            if !missing.is_empty() {
                diagnostics.push(mapping_diagnostic(
                    "missing_enum_local_member",
                    type_id,
                    format!(
                        "member_mappings is missing local members: {}",
                        missing
                            .iter()
                            .map(|member_id| format!("`{member_id}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
            if !extra.is_empty() {
                diagnostics.push(mapping_diagnostic(
                    "extra_enum_local_member",
                    type_id,
                    format!(
                        "member_mappings has extra local members: {}",
                        extra
                            .iter()
                            .map(|member_id| format!("`{member_id}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
        None => {}
    }

    let source_members = mapping
        .source_members
        .iter()
        .filter_map(|literal| (!literal.is_empty()).then_some(literal.as_str()))
        .collect::<HashSet<_>>();
    let mut unknown_source_literals = source_literal_counts
        .keys()
        .filter(|literal| !source_members.contains(**literal))
        .copied()
        .collect::<Vec<_>>();
    unknown_source_literals.sort_unstable();
    for source_literal in unknown_source_literals {
        diagnostics.push(mapping_diagnostic(
            "unknown_enum_source_literal",
            type_id,
            format!("source literal `{source_literal}` is absent from source_members"),
        ));
    }
}

fn validate_mappings(
    resolved: &ResolvedSpecialization,
    enum_mappings: &[EnumAbiMapping],
    named_types: &HashMap<&str, &ResolvedType>,
    diagnostics: &mut Vec<ScalarAbiDiagnostic>,
) {
    let mut mapping_counts = HashMap::new();
    for mapping in enum_mappings {
        if !mapping.type_id.is_empty() {
            *mapping_counts.entry(mapping.type_id.as_str()).or_insert(0) += 1;
        }
        validate_mapping(mapping, named_types, diagnostics);
    }
    for (type_id, count) in &mapping_counts {
        if *count > 1 {
            diagnostics.push(mapping_diagnostic(
                "duplicate_enum_mapping",
                type_id,
                format!("enum type `{type_id}` has multiple mappings"),
            ));
        }
    }
    for parameter in &resolved.parameters {
        if let ResolvedType::Enum { type_id, .. } = &parameter.resolved_type
            && !mapping_counts.contains_key(type_id.as_str())
        {
            diagnostics.push(diagnostic(
                "unsupported_enum",
                "parameter",
                &parameter.parameter_id,
                type_id,
                format!("enum type `{type_id}` has no scalar ABI mapping"),
            ));
        }
    }
    for connector in &resolved.connectors {
        if connector.active
            && let ResolvedType::Enum { type_id, .. } = &connector.resolved_type
            && !mapping_counts.contains_key(type_id.as_str())
        {
            diagnostics.push(diagnostic(
                "unsupported_enum",
                "connector",
                &connector.connector_id,
                type_id,
                format!("enum type `{type_id}` has no scalar ABI mapping"),
            ));
        }
    }
}

struct ValidatedEnumMapping<'a> {
    canonical_class_path: &'a str,
    ordinals: HashMap<&'a str, usize>,
}

fn validated_mapping_index<'a>(
    enum_mappings: &'a [EnumAbiMapping],
) -> HashMap<&'a str, ValidatedEnumMapping<'a>> {
    let mut mappings = HashMap::new();
    for mapping in enum_mappings {
        let source_ordinals = mapping
            .source_members
            .iter()
            .enumerate()
            .map(|(index, literal)| (literal.as_str(), index + 1))
            .collect::<HashMap<_, _>>();
        let ordinals = mapping
            .member_mappings
            .iter()
            .filter_map(|member_mapping| {
                source_ordinals
                    .get(member_mapping.source_literal.as_str())
                    .copied()
                    .map(|ordinal| (member_mapping.member_id.as_str(), ordinal))
            })
            .collect();
        mappings.insert(
            mapping.type_id.as_str(),
            ValidatedEnumMapping {
                canonical_class_path: &mapping.canonical_class_path,
                ordinals,
            },
        );
    }
    mappings
}

fn projected_type(
    resolved_type: &ResolvedType,
    mappings: &HashMap<&str, ValidatedEnumMapping<'_>>,
) -> Option<ScalarAbiType> {
    match resolved_type {
        ResolvedType::Primitive(primitive) => Some(ScalarAbiType::Primitive(*primitive)),
        ResolvedType::Alias {
            type_id,
            primitive,
            quantity,
            unit,
            display_unit,
        } => Some(ScalarAbiType::Alias {
            type_id: type_id.clone(),
            primitive: *primitive,
            quantity: quantity.clone(),
            unit: unit.clone(),
            display_unit: display_unit.clone(),
        }),
        ResolvedType::Enum { type_id, .. } => {
            mappings
                .get(type_id.as_str())
                .map(|mapping| ScalarAbiType::Enum {
                    canonical_class_path: mapping.canonical_class_path.to_owned(),
                })
        }
    }
}

fn projected_value(
    value: &ResolvedScalarValue,
    resolved_type: &ResolvedType,
    mappings: &HashMap<&str, ValidatedEnumMapping<'_>>,
) -> Option<ScalarAbiValue> {
    match value {
        ResolvedScalarValue::Boolean(value) => Some(ScalarAbiValue::Boolean(*value)),
        ResolvedScalarValue::Integer(value) => Some(ScalarAbiValue::Integer(value.clone())),
        ResolvedScalarValue::Real(value) => Some(ScalarAbiValue::Real(*value)),
        ResolvedScalarValue::Enum(value) => {
            let ResolvedType::Enum { type_id, .. } = resolved_type else {
                return None;
            };
            mappings
                .get(type_id.as_str())?
                .ordinals
                .get(value.member_id.as_str())
                .copied()
                .map(|ordinal| ScalarAbiValue::Enum { ordinal })
        }
    }
}

struct PreparedParameter {
    abi_type: ScalarAbiType,
    values: Vec<ScalarAbiValue>,
}

struct PreparedConnector {
    abi_type: ScalarAbiType,
}

fn prepare_projection(
    resolved: &ResolvedSpecialization,
    mappings: &HashMap<&str, ValidatedEnumMapping<'_>>,
) -> Result<(Vec<PreparedParameter>, Vec<Option<PreparedConnector>>), ScalarAbiError> {
    let mut diagnostics = Vec::new();
    let mut parameters = Vec::new();
    for parameter in &resolved.parameters {
        let Some(abi_type) = projected_type(&parameter.resolved_type, mappings) else {
            diagnostics.push(invalid_resolved(
                "parameter",
                &parameter.parameter_id,
                named_type_id(&parameter.resolved_type).unwrap_or(""),
                "validated enum mapping is unavailable during projection",
            ));
            continue;
        };
        let mut values = Vec::new();
        for leaf in &parameter.leaves {
            if let Some(value) = projected_value(&leaf.value, &parameter.resolved_type, mappings) {
                values.push(value);
            } else {
                diagnostics.push(invalid_resolved(
                    "parameter",
                    &parameter.parameter_id,
                    named_type_id(&parameter.resolved_type).unwrap_or(""),
                    "validated scalar value cannot be projected",
                ));
            }
        }
        parameters.push(PreparedParameter { abi_type, values });
    }

    let mut connectors = Vec::new();
    for connector in &resolved.connectors {
        if !connector.active {
            connectors.push(None);
            continue;
        }
        if let Some(abi_type) = projected_type(&connector.resolved_type, mappings) {
            connectors.push(Some(PreparedConnector { abi_type }));
        } else {
            diagnostics.push(invalid_resolved(
                "connector",
                &connector.connector_id,
                named_type_id(&connector.resolved_type).unwrap_or(""),
                "validated enum mapping is unavailable during projection",
            ));
            connectors.push(None);
        }
    }
    if diagnostics.is_empty() {
        Ok((parameters, connectors))
    } else {
        Err(ScalarAbiError::new(diagnostics))
    }
}

fn projected_coordinates(coordinates: &[Coordinate]) -> Vec<ScalarCoordinate> {
    coordinates
        .iter()
        .map(|coordinate| ScalarCoordinate {
            dimension_id: coordinate.dimension_id.clone(),
            member_id: coordinate.member_id.clone(),
            ordinal: coordinate.ordinal,
        })
        .collect()
}

fn checked_row_count(
    lengths: impl IntoIterator<Item = usize>,
    owner_kind: &str,
) -> Result<usize, ScalarAbiError> {
    let mut count = 0_usize;
    for length in lengths {
        let Some(next) = count.checked_add(length) else {
            return Err(ScalarAbiError::new(vec![diagnostic(
                "resource_limit",
                "projection",
                "$",
                "",
                format!("{owner_kind} scalar row count overflows usize"),
            )]));
        };
        count = next;
    }
    Ok(count)
}

/// Projects resolved scalar leaves without I/O or guard reevaluation.
///
/// All mapping and resolved-input diagnostics are collected and sorted before any
/// output row is allocated. Enum class paths are copied but not verified.
pub fn project_scalar_abi(
    resolved: &ResolvedSpecialization,
    enum_mappings: &[EnumAbiMapping],
) -> Result<ScalarAbiProjection, ScalarAbiError> {
    let mut diagnostics = Vec::new();
    if resolved.canonical_id.is_empty() {
        diagnostics.push(invalid_resolved(
            "projection",
            "$",
            "",
            "canonical ID must not be empty",
        ));
    }
    if resolved.revision <= BigInt::from(0_u8) {
        diagnostics.push(invalid_resolved(
            "projection",
            "$",
            "",
            "revision must be positive",
        ));
    }
    let dimensions = validate_dimensions(resolved, &mut diagnostics);
    let mut named_types = HashMap::new();
    validate_parameters(resolved, &dimensions, &mut named_types, &mut diagnostics);
    validate_connectors(resolved, &dimensions, &mut named_types, &mut diagnostics);
    validate_mappings(resolved, enum_mappings, &named_types, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(ScalarAbiError::new(diagnostics));
    }

    let mapping_index = validated_mapping_index(enum_mappings);
    let (prepared_parameters, prepared_connectors) = prepare_projection(resolved, &mapping_index)?;
    let parameter_count = checked_row_count(
        resolved
            .parameters
            .iter()
            .map(|parameter| parameter.leaves.len()),
        "parameter",
    )?;
    let connector_count = checked_row_count(
        resolved
            .connectors
            .iter()
            .filter(|connector| connector.active)
            .map(|connector| connector.leaves.len()),
        "connector",
    )?;

    let mut parameters = Vec::new();
    if parameters.try_reserve_exact(parameter_count).is_err() {
        return Err(ScalarAbiError::new(vec![diagnostic(
            "resource_limit",
            "projection",
            "$",
            "",
            "parameter row allocation failed",
        )]));
    }
    let mut connectors = Vec::new();
    if connectors.try_reserve_exact(connector_count).is_err() {
        return Err(ScalarAbiError::new(vec![diagnostic(
            "resource_limit",
            "projection",
            "$",
            "",
            "connector row allocation failed",
        )]));
    }
    for (parameter, prepared) in resolved.parameters.iter().zip(prepared_parameters) {
        for (leaf, value) in parameter.leaves.iter().zip(prepared.values) {
            parameters.push(ScalarParameterAbiRow {
                parameter_id: parameter.parameter_id.clone(),
                coordinates: projected_coordinates(&leaf.coordinates),
                abi_type: prepared.abi_type.clone(),
                source: parameter.source,
                value,
            });
        }
    }

    for (connector, prepared) in resolved.connectors.iter().zip(prepared_connectors) {
        let Some(prepared) = prepared else {
            continue;
        };
        for leaf in &connector.leaves {
            connectors.push(ScalarConnectorAbiRow {
                connector_id: connector.connector_id.clone(),
                coordinates: projected_coordinates(&leaf.coordinates),
                abi_type: prepared.abi_type.clone(),
                direction: connector.direction,
            });
        }
    }

    Ok(ScalarAbiProjection {
        canonical_id: resolved.canonical_id.clone(),
        revision: resolved.revision.clone(),
        parameters,
        connectors,
    })
}

#[cfg(test)]
mod tests;
