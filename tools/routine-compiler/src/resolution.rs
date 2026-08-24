//! Deterministic resolution for typed routine contracts.
//!
//! Callers must perform raw-document schema and interface/specialization agreement
//! validation before constructing this model. The resolver still checks every
//! reference and expansion invariant needed to avoid panics when a malformed typed
//! value reaches the boundary. It performs no I/O and returns owned output.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;

use num_bigint::BigInt;
use num_rational::BigRational;

/// Bounds work that scales with guard structure or scalar expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolutionLimits {
    /// Maximum depth of one guard root, where the root has depth one.
    pub max_guard_depth: usize,
    /// Maximum guard nodes aggregated across all connectors.
    pub max_guard_nodes: usize,
    /// Maximum parameter and active-connector scalar leaves combined.
    pub max_scalar_leaves: usize,
}

impl Default for ResolutionLimits {
    fn default() -> Self {
        Self {
            max_guard_depth: 32,
            max_guard_nodes: 2_048,
            max_scalar_leaves: 100_000,
        }
    }
}

/// A finite IEEE-754 binary64 value. Construction rejects NaN and infinities.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteReal(f64);

impl FiniteReal {
    /// Preserves the input bits, including the sign of zero.
    pub fn new(value: f64) -> Result<Self, ResolutionError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(invalid("real value must be finite"))
        }
    }

    /// Returns the stored binary64 value without conversion.
    pub fn get(self) -> f64 {
        self.0
    }
}

/// Primitive types admitted by the validated routine boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    /// Finite real values; integer-valued inputs retain their integer variant.
    Real,
    /// Arbitrary-precision integers.
    Integer,
    /// Boolean values.
    Boolean,
}

/// One stable member of a local enum definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumMemberDefinition {
    pub member_id: String,
    pub symbol: String,
}

/// The body of a named local type definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamedTypeDefinition {
    /// A primitive alias with optional engineering display metadata.
    Alias {
        primitive: PrimitiveType,
        quantity: Option<String>,
        unit: Option<String>,
        display_unit: Option<String>,
    },
    /// An enum whose authored member order is significant.
    Enum { members: Vec<EnumMemberDefinition> },
}

/// A named local type in authored order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeDefinition {
    pub type_id: String,
    pub definition: NamedTypeDefinition,
}

/// A primitive use or reference to a local named type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeUse {
    Primitive(PrimitiveType),
    Named(String),
}

/// The concrete members and extent source of one dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DimensionKind {
    /// Interface-owned members; their count is the concrete extent.
    Fixed { members: Vec<String> },
    /// Specialization-owned members, checked against a scalar Integer parameter.
    ParameterDriven {
        parameter_id: String,
        members: Vec<String>,
    },
}

impl DimensionKind {
    fn members(&self) -> &[String] {
        match self {
            Self::Fixed { members } | Self::ParameterDriven { members, .. } => members,
        }
    }
}

/// One dimension in authored order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DimensionDefinition {
    pub dimension_id: String,
    pub kind: DimensionKind,
}

/// Scalar, rank-one, or rank-two shape with authored dimension order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Scalar,
    Rank1 {
        dimension_id: String,
    },
    Rank2 {
        first_dimension_id: String,
        second_dimension_id: String,
    },
}

/// Stable enum identity carried by a validated input value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumInputValue {
    pub type_id: String,
    pub member_id: String,
}

/// A typed scalar. Integer and real variants remain distinct in resolved leaves.
#[derive(Clone, Debug, PartialEq)]
pub enum ScalarValue {
    Boolean(bool),
    Integer(BigInt),
    Real(FiniteReal),
    Enum(EnumInputValue),
}

/// An effective parameter value whose container rank is explicit.
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValue {
    Scalar(ScalarValue),
    Rank1(Vec<ScalarValue>),
    Rank2(Vec<Vec<ScalarValue>>),
}

impl ParameterValue {
    fn scalar(&self) -> Option<&ScalarValue> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::Rank1(_) | Self::Rank2(_) => None,
        }
    }
}

/// Whether an effective value came from the interface default or specialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterSource {
    Default,
    Assignment,
}

/// One effective parameter in interface-authored order.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterDefinition {
    pub parameter_id: String,
    pub type_use: TypeUse,
    pub shape: Shape,
    pub source: ParameterSource,
    pub value: ParameterValue,
}

/// Connector dataflow direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectorDirection {
    Input,
    Output,
}

/// A scalar guard operand.
#[derive(Clone, Debug, PartialEq)]
pub enum GuardOperand {
    /// References an effective scalar parameter by stable ID.
    Parameter(String),
    /// Carries an explicit type and validated scalar literal.
    Literal {
        type_use: TypeUse,
        value: ScalarValue,
    },
}

/// Comparison operators supported by connector guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

/// Typed connector-presence expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Guard {
    And(Vec<Guard>),
    Or(Vec<Guard>),
    Not(Box<Guard>),
    Compare {
        operator: ComparisonOperator,
        left: GuardOperand,
        right: GuardOperand,
    },
}

/// Unconditional or guard-controlled connector presence.
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectorPresence {
    Always,
    Guarded(Guard),
}

/// One connector in interface-authored order.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectorDefinition {
    pub connector_id: String,
    pub direction: ConnectorDirection,
    pub type_use: TypeUse,
    pub shape: Shape,
    pub presence: ConnectorPresence,
}

/// Already-validated and normalized input to deterministic resolution.
///
/// Vectors retain contract order. This type is an in-memory boundary, not a raw
/// document or persisted format.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedResolutionInput {
    pub canonical_id: String,
    pub revision: BigInt,
    pub types: Vec<TypeDefinition>,
    pub dimensions: Vec<DimensionDefinition>,
    pub parameters: Vec<ParameterDefinition>,
    pub connectors: Vec<ConnectorDefinition>,
}

/// One coordinate in a resolved row-major scalar expansion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coordinate {
    pub dimension_id: String,
    pub member_id: String,
    pub ordinal: usize,
}

/// Enum identity and symbol detached from the input definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedEnumValue {
    pub type_id: String,
    pub member_id: String,
    pub symbol: String,
}

/// Value stored in one resolved parameter leaf.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedScalarValue {
    Boolean(bool),
    Integer(BigInt),
    Real(FiniteReal),
    Enum(ResolvedEnumValue),
}

/// One resolved parameter scalar in row-major order.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarParameterLeaf {
    pub coordinates: Vec<Coordinate>,
    pub value: ResolvedScalarValue,
}

/// One active connector scalar in row-major order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarConnectorLeaf {
    pub coordinates: Vec<Coordinate>,
}

/// One detached member in a resolved enum type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedEnumMember {
    pub member_id: String,
    pub symbol: String,
}

/// Primitive, alias, or enum information materialized for one type use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedType {
    Primitive(PrimitiveType),
    Alias {
        type_id: String,
        primitive: PrimitiveType,
        quantity: Option<String>,
        unit: Option<String>,
        display_unit: Option<String>,
    },
    Enum {
        type_id: String,
        members: Vec<ResolvedEnumMember>,
    },
}

/// Fixed or parameter-driven dimension kind in resolved output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedDimensionKind {
    Fixed,
    Parameter,
}

/// One concrete dimension in authored order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedDimension {
    pub dimension_id: String,
    pub kind: ResolvedDimensionKind,
    pub extent: usize,
    pub members: Vec<String>,
}

/// One effective parameter and all of its detached scalar leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedParameter {
    pub parameter_id: String,
    pub resolved_type: ResolvedType,
    pub dimension_ids: Vec<String>,
    pub source: ParameterSource,
    pub leaves: Vec<ScalarParameterLeaf>,
}

/// One connector and its presence result. Inactive connectors keep dimensions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedConnector {
    pub connector_id: String,
    pub direction: ConnectorDirection,
    pub resolved_type: ResolvedType,
    pub dimension_ids: Vec<String>,
    pub active: bool,
    pub guard_result: Option<bool>,
    pub leaves: Vec<ScalarConnectorLeaf>,
}

/// Owned result of resolving one typed specialization.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSpecialization {
    pub canonical_id: String,
    pub revision: BigInt,
    pub dimensions: Vec<ResolvedDimension>,
    pub parameters: Vec<ResolvedParameter>,
    pub connectors: Vec<ResolvedConnector>,
}

/// Deterministic failure category for the typed boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolutionError {
    /// A typed reference, shape, value, or invariant is inconsistent.
    InvalidInput { detail: String },
    /// Guard or scalar work exceeds a configured or machine-sized bound.
    ResourceLimit { detail: String },
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { detail } => write!(formatter, "invalid input: {detail}"),
            Self::ResourceLimit { detail } => write!(formatter, "resource limit: {detail}"),
        }
    }
}

impl std::error::Error for ResolutionError {}

fn invalid(detail: impl Into<String>) -> ResolutionError {
    ResolutionError::InvalidInput {
        detail: detail.into(),
    }
}

fn resource(detail: impl Into<String>) -> ResolutionError {
    ResolutionError::ResourceLimit {
        detail: detail.into(),
    }
}

struct ValidatedInput<'a> {
    input: &'a ValidatedResolutionInput,
    types: HashMap<&'a str, usize>,
    dimensions: HashMap<&'a str, usize>,
    parameters: HashMap<&'a str, usize>,
    dimension_extents: Vec<usize>,
}

impl<'a> ValidatedInput<'a> {
    fn type_definition(&self, type_id: &str) -> Result<&'a TypeDefinition, ResolutionError> {
        let index = self
            .types
            .get(type_id)
            .ok_or_else(|| invalid(format!("unknown named type `{type_id}`")))?;
        self.input
            .types
            .get(*index)
            .ok_or_else(|| invalid(format!("unknown named type `{type_id}`")))
    }

    fn parameter(&self, parameter_id: &str) -> Result<&'a ParameterDefinition, ResolutionError> {
        let index = self
            .parameters
            .get(parameter_id)
            .ok_or_else(|| invalid(format!("unknown parameter `{parameter_id}`")))?;
        self.input
            .parameters
            .get(*index)
            .ok_or_else(|| invalid(format!("unknown parameter `{parameter_id}`")))
    }

    fn dimension(&self, dimension_id: &str) -> Result<&'a DimensionDefinition, ResolutionError> {
        let index = self
            .dimensions
            .get(dimension_id)
            .ok_or_else(|| invalid(format!("unknown dimension `{dimension_id}`")))?;
        self.input
            .dimensions
            .get(*index)
            .ok_or_else(|| invalid(format!("unknown dimension `{dimension_id}`")))
    }

    fn dimension_extent(&self, dimension_id: &str) -> Result<usize, ResolutionError> {
        let index = self
            .dimensions
            .get(dimension_id)
            .ok_or_else(|| invalid(format!("unknown dimension `{dimension_id}`")))?;
        self.dimension_extents
            .get(*index)
            .copied()
            .ok_or_else(|| invalid(format!("unknown dimension `{dimension_id}`")))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueType<'a> {
    Primitive(PrimitiveType),
    Enum(&'a str),
}

fn value_type<'a>(
    type_use: &'a TypeUse,
    validated: &'a ValidatedInput<'a>,
) -> Result<ValueType<'a>, ResolutionError> {
    match type_use {
        TypeUse::Primitive(primitive) => Ok(ValueType::Primitive(*primitive)),
        TypeUse::Named(type_id) => {
            let definition = validated.type_definition(type_id)?;
            match &definition.definition {
                NamedTypeDefinition::Alias { primitive, .. } => {
                    Ok(ValueType::Primitive(*primitive))
                }
                NamedTypeDefinition::Enum { .. } => Ok(ValueType::Enum(&definition.type_id)),
            }
        }
    }
}

fn validate_identifier(value: &str, label: &str) -> Result<(), ResolutionError> {
    if value.is_empty() {
        Err(invalid(format!("{label} must not be empty")))
    } else {
        Ok(())
    }
}

fn validate_optional_metadata(value: &Option<String>, label: &str) -> Result<(), ResolutionError> {
    if let Some(value) = value
        && (value.is_empty() || value.trim() != value)
    {
        return Err(invalid(format!("{label} must be nonempty and trimmed")));
    }
    Ok(())
}

fn shape_dimension_ids(shape: &Shape) -> Vec<String> {
    match shape {
        Shape::Scalar => Vec::new(),
        Shape::Rank1 { dimension_id } => vec![dimension_id.clone()],
        Shape::Rank2 {
            first_dimension_id,
            second_dimension_id,
        } => vec![first_dimension_id.clone(), second_dimension_id.clone()],
    }
}

fn validate_shape(shape: &Shape, validated: &ValidatedInput<'_>) -> Result<(), ResolutionError> {
    match shape {
        Shape::Scalar => Ok(()),
        Shape::Rank1 { dimension_id } => {
            validated.dimension(dimension_id)?;
            Ok(())
        }
        Shape::Rank2 {
            first_dimension_id,
            second_dimension_id,
        } => {
            validated.dimension(first_dimension_id)?;
            validated.dimension(second_dimension_id)?;
            Ok(())
        }
    }
}

fn validate_scalar(
    value: &ScalarValue,
    expected: ValueType<'_>,
    validated: &ValidatedInput<'_>,
) -> Result<(), ResolutionError> {
    match (expected, value) {
        (ValueType::Primitive(PrimitiveType::Boolean), ScalarValue::Boolean(_))
        | (ValueType::Primitive(PrimitiveType::Integer), ScalarValue::Integer(_))
        | (ValueType::Primitive(PrimitiveType::Real), ScalarValue::Integer(_))
        | (ValueType::Primitive(PrimitiveType::Real), ScalarValue::Real(_)) => Ok(()),
        (ValueType::Enum(expected_type), ScalarValue::Enum(value)) => {
            if value.type_id != expected_type {
                return Err(invalid(format!(
                    "enum value type `{}` does not match `{expected_type}`",
                    value.type_id
                )));
            }
            let definition = validated.type_definition(expected_type)?;
            let NamedTypeDefinition::Enum { members } = &definition.definition else {
                return Err(invalid(format!(
                    "named type `{expected_type}` is not an enum"
                )));
            };
            if members
                .iter()
                .any(|member| member.member_id == value.member_id)
            {
                Ok(())
            } else {
                Err(invalid(format!(
                    "unknown member `{}` for enum `{expected_type}`",
                    value.member_id
                )))
            }
        }
        _ => Err(invalid("scalar value does not match its declared type")),
    }
}

fn validate_parameter_value(
    parameter: &ParameterDefinition,
    validated: &ValidatedInput<'_>,
) -> Result<(), ResolutionError> {
    let expected_type = value_type(&parameter.type_use, validated)?;
    match (&parameter.shape, &parameter.value) {
        (Shape::Scalar, ParameterValue::Scalar(value)) => {
            validate_scalar(value, expected_type, validated)
        }
        (Shape::Rank1 { dimension_id }, ParameterValue::Rank1(values)) => {
            let expected = validated.dimension_extent(dimension_id)?;
            if values.len() != expected {
                return Err(invalid(format!(
                    "parameter `{}` rank-one length is {}, expected {expected}",
                    parameter.parameter_id,
                    values.len()
                )));
            }
            for value in values {
                validate_scalar(value, expected_type, validated)?;
            }
            Ok(())
        }
        (
            Shape::Rank2 {
                first_dimension_id,
                second_dimension_id,
            },
            ParameterValue::Rank2(rows),
        ) => {
            let expected_rows = validated.dimension_extent(first_dimension_id)?;
            let expected_columns = validated.dimension_extent(second_dimension_id)?;
            if rows.len() != expected_rows {
                return Err(invalid(format!(
                    "parameter `{}` matrix row count is {}, expected {expected_rows}",
                    parameter.parameter_id,
                    rows.len()
                )));
            }
            for row in rows {
                if row.len() != expected_columns {
                    return Err(invalid(format!(
                        "parameter `{}` matrix column count is {}, expected {expected_columns}",
                        parameter.parameter_id,
                        row.len()
                    )));
                }
                for value in row {
                    validate_scalar(value, expected_type, validated)?;
                }
            }
            Ok(())
        }
        _ => Err(invalid(format!(
            "parameter `{}` value rank does not match its shape",
            parameter.parameter_id
        ))),
    }
}

fn compatible_guard_types(left: ValueType<'_>, right: ValueType<'_>) -> bool {
    if left == right {
        return true;
    }
    matches!(
        (left, right),
        (
            ValueType::Primitive(PrimitiveType::Integer | PrimitiveType::Real),
            ValueType::Primitive(PrimitiveType::Integer | PrimitiveType::Real)
        )
    )
}

fn is_numeric(value_type: ValueType<'_>) -> bool {
    matches!(
        value_type,
        ValueType::Primitive(PrimitiveType::Integer | PrimitiveType::Real)
    )
}

fn guard_operand_type<'a>(
    operand: &'a GuardOperand,
    validated: &'a ValidatedInput<'a>,
) -> Result<ValueType<'a>, ResolutionError> {
    match operand {
        GuardOperand::Parameter(parameter_id) => {
            let parameter = validated.parameter(parameter_id)?;
            if parameter.shape != Shape::Scalar {
                return Err(invalid(format!(
                    "guard parameter `{parameter_id}` must be scalar"
                )));
            }
            value_type(&parameter.type_use, validated)
        }
        GuardOperand::Literal { type_use, value } => {
            let literal_type = value_type(type_use, validated)?;
            validate_scalar(value, literal_type, validated)?;
            Ok(literal_type)
        }
    }
}

fn validate_guard(guard: &Guard, validated: &ValidatedInput<'_>) -> Result<(), ResolutionError> {
    let mut stack = vec![guard];
    while let Some(node) = stack.pop() {
        match node {
            Guard::And(operands) | Guard::Or(operands) => {
                if operands.is_empty() {
                    return Err(invalid("and/or guards require at least one operand"));
                }
                stack.extend(operands.iter().rev());
            }
            Guard::Not(operand) => stack.push(operand),
            Guard::Compare {
                operator,
                left,
                right,
            } => {
                let left_type = guard_operand_type(left, validated)?;
                let right_type = guard_operand_type(right, validated)?;
                if !compatible_guard_types(left_type, right_type) {
                    return Err(invalid("guard operands have incompatible types"));
                }
                if matches!(
                    operator,
                    ComparisonOperator::Lt
                        | ComparisonOperator::Lte
                        | ComparisonOperator::Gt
                        | ComparisonOperator::Gte
                ) && (!is_numeric(left_type) || !is_numeric(right_type))
                {
                    return Err(invalid("ordering comparison requires numeric operands"));
                }
            }
        }
    }
    Ok(())
}

fn validate_input(input: &ValidatedResolutionInput) -> Result<ValidatedInput<'_>, ResolutionError> {
    validate_identifier(&input.canonical_id, "canonical ID")?;
    if input.revision <= BigInt::from(0_u8) {
        return Err(invalid("revision must be positive"));
    }

    let mut types = HashMap::new();
    for (index, definition) in input.types.iter().enumerate() {
        validate_identifier(&definition.type_id, "type ID")?;
        if types.insert(definition.type_id.as_str(), index).is_some() {
            return Err(invalid(format!(
                "duplicate type ID `{}`",
                definition.type_id
            )));
        }
    }

    let mut parameters = HashMap::new();
    for (index, parameter) in input.parameters.iter().enumerate() {
        validate_identifier(&parameter.parameter_id, "parameter ID")?;
        if parameters
            .insert(parameter.parameter_id.as_str(), index)
            .is_some()
        {
            return Err(invalid(format!(
                "duplicate parameter ID `{}`",
                parameter.parameter_id
            )));
        }
    }

    let mut dimensions = HashMap::new();
    for (index, dimension) in input.dimensions.iter().enumerate() {
        validate_identifier(&dimension.dimension_id, "dimension ID")?;
        if dimensions
            .insert(dimension.dimension_id.as_str(), index)
            .is_some()
        {
            return Err(invalid(format!(
                "duplicate dimension ID `{}`",
                dimension.dimension_id
            )));
        }
    }

    let mut connector_ids = HashSet::new();
    for connector in &input.connectors {
        validate_identifier(&connector.connector_id, "connector ID")?;
        if !connector_ids.insert(connector.connector_id.as_str()) {
            return Err(invalid(format!(
                "duplicate connector ID `{}`",
                connector.connector_id
            )));
        }
    }

    let mut validated = ValidatedInput {
        input,
        types,
        dimensions,
        parameters,
        dimension_extents: vec![0; input.dimensions.len()],
    };

    for definition in &input.types {
        match &definition.definition {
            NamedTypeDefinition::Alias {
                quantity,
                unit,
                display_unit,
                ..
            } => {
                validate_optional_metadata(quantity, "alias quantity")?;
                validate_optional_metadata(unit, "alias unit")?;
                validate_optional_metadata(display_unit, "alias display unit")?;
            }
            NamedTypeDefinition::Enum { members } => {
                if members.is_empty() {
                    return Err(invalid(format!(
                        "enum `{}` must have at least one member",
                        definition.type_id
                    )));
                }
                let mut member_ids = HashSet::new();
                let mut symbols = HashSet::new();
                for member in members {
                    validate_identifier(&member.member_id, "enum member ID")?;
                    validate_identifier(&member.symbol, "enum member symbol")?;
                    if !member_ids.insert(member.member_id.as_str()) {
                        return Err(invalid(format!(
                            "duplicate member ID `{}` in enum `{}`",
                            member.member_id, definition.type_id
                        )));
                    }
                    if !symbols.insert(member.symbol.as_str()) {
                        return Err(invalid(format!(
                            "duplicate symbol `{}` in enum `{}`",
                            member.symbol, definition.type_id
                        )));
                    }
                }
            }
        }
    }

    for parameter in &input.parameters {
        value_type(&parameter.type_use, &validated)?;
        validate_shape(&parameter.shape, &validated)?;
    }

    let mut dimension_member_ids = HashSet::new();
    for (index, dimension) in input.dimensions.iter().enumerate() {
        let members = dimension.kind.members();
        if members.is_empty() {
            return Err(invalid(format!(
                "dimension `{}` must have at least one member",
                dimension.dimension_id
            )));
        }
        for member_id in members {
            validate_identifier(member_id, "dimension member ID")?;
            if !dimension_member_ids.insert(member_id.as_str()) {
                return Err(invalid(format!(
                    "duplicate stable dimension member `{member_id}`"
                )));
            }
        }
        if let DimensionKind::ParameterDriven { parameter_id, .. } = &dimension.kind {
            let parameter = validated.parameter(parameter_id)?;
            if parameter.shape != Shape::Scalar {
                return Err(invalid(format!(
                    "dimension parameter `{parameter_id}` must be scalar"
                )));
            }
            if value_type(&parameter.type_use, &validated)?
                != ValueType::Primitive(PrimitiveType::Integer)
            {
                return Err(invalid(format!(
                    "dimension parameter `{parameter_id}` must resolve to Integer"
                )));
            }
            let Some(ScalarValue::Integer(extent)) = parameter.value.scalar() else {
                return Err(invalid(format!(
                    "dimension parameter `{parameter_id}` must have a scalar Integer value"
                )));
            };
            if extent <= &BigInt::from(0_u8) {
                return Err(invalid(format!(
                    "dimension parameter `{parameter_id}` must be positive"
                )));
            }
            if extent != &BigInt::from(members.len()) {
                return Err(invalid(format!(
                    "dimension `{}` has {} members but parameter `{parameter_id}` resolves to {extent}",
                    dimension.dimension_id,
                    members.len()
                )));
            }
        }
        if let Some(slot) = validated.dimension_extents.get_mut(index) {
            *slot = members.len();
        } else {
            return Err(invalid(format!(
                "unknown dimension `{}`",
                dimension.dimension_id
            )));
        }
    }

    for parameter in &input.parameters {
        validate_parameter_value(parameter, &validated)?;
    }

    for connector in &input.connectors {
        value_type(&connector.type_use, &validated)?;
        validate_shape(&connector.shape, &validated)?;
        if let ConnectorPresence::Guarded(guard) = &connector.presence {
            validate_guard(guard, &validated)?;
        }
    }

    Ok(validated)
}

fn preflight_guards(
    input: &ValidatedResolutionInput,
    limits: ResolutionLimits,
) -> Result<(), ResolutionError> {
    let mut node_count = 0_usize;
    for connector in &input.connectors {
        let ConnectorPresence::Guarded(root) = &connector.presence else {
            continue;
        };
        let mut stack = vec![(root, 1_usize)];
        while let Some((node, depth)) = stack.pop() {
            node_count = node_count
                .checked_add(1)
                .ok_or_else(|| resource("guard node count overflow"))?;
            if node_count > limits.max_guard_nodes {
                return Err(resource(format!(
                    "guard node count exceeds limit {}",
                    limits.max_guard_nodes
                )));
            }
            if depth > limits.max_guard_depth {
                return Err(resource(format!(
                    "guard depth {depth} exceeds limit {}",
                    limits.max_guard_depth
                )));
            }

            let child_count = match node {
                Guard::And(operands) | Guard::Or(operands) => operands.len(),
                Guard::Not(_) => 1,
                Guard::Compare { .. } => 0,
            };
            let projected = node_count
                .checked_add(stack.len())
                .and_then(|count| count.checked_add(child_count))
                .ok_or_else(|| resource("guard node count overflow"))?;
            if projected > limits.max_guard_nodes {
                return Err(resource(format!(
                    "guard node count exceeds limit {}",
                    limits.max_guard_nodes
                )));
            }
            let child_depth = depth
                .checked_add(1)
                .ok_or_else(|| resource("guard depth overflow"))?;
            match node {
                Guard::And(operands) | Guard::Or(operands) => {
                    for child in operands.iter().rev() {
                        stack.push((child, child_depth));
                    }
                }
                Guard::Not(operand) => stack.push((operand, child_depth)),
                Guard::Compare { .. } => {}
            }
        }
    }
    Ok(())
}

fn numeric_order(left: &ScalarValue, right: &ScalarValue) -> Result<Ordering, ResolutionError> {
    match (left, right) {
        (ScalarValue::Integer(left), ScalarValue::Integer(right)) => Ok(left.cmp(right)),
        (ScalarValue::Real(left), ScalarValue::Real(right)) => left
            .get()
            .partial_cmp(&right.get())
            .ok_or_else(|| invalid("real comparison requires finite operands")),
        (ScalarValue::Integer(integer), ScalarValue::Real(real)) => {
            let integer = BigRational::from_integer(integer.clone());
            let real = BigRational::from_float(real.get())
                .ok_or_else(|| invalid("real comparison requires finite operands"))?;
            Ok(integer.cmp(&real))
        }
        (ScalarValue::Real(real), ScalarValue::Integer(integer)) => {
            let real = BigRational::from_float(real.get())
                .ok_or_else(|| invalid("real comparison requires finite operands"))?;
            let integer = BigRational::from_integer(integer.clone());
            Ok(real.cmp(&integer))
        }
        _ => Err(invalid("numeric comparison requires numeric operands")),
    }
}

fn compare_values(
    operator: ComparisonOperator,
    left: &ScalarValue,
    right: &ScalarValue,
) -> Result<bool, ResolutionError> {
    let equality = match (left, right) {
        (ScalarValue::Boolean(left), ScalarValue::Boolean(right)) => Some(left == right),
        (ScalarValue::Enum(left), ScalarValue::Enum(right)) => Some(left == right),
        (
            ScalarValue::Integer(_) | ScalarValue::Real(_),
            ScalarValue::Integer(_) | ScalarValue::Real(_),
        ) => Some(numeric_order(left, right)? == Ordering::Equal),
        _ => None,
    };
    match operator {
        ComparisonOperator::Eq => equality.ok_or_else(|| invalid("incompatible equality operands")),
        ComparisonOperator::Ne => equality
            .map(|equal| !equal)
            .ok_or_else(|| invalid("incompatible equality operands")),
        ComparisonOperator::Lt => Ok(numeric_order(left, right)? == Ordering::Less),
        ComparisonOperator::Lte => Ok(numeric_order(left, right)? != Ordering::Greater),
        ComparisonOperator::Gt => Ok(numeric_order(left, right)? == Ordering::Greater),
        ComparisonOperator::Gte => Ok(numeric_order(left, right)? != Ordering::Less),
    }
}

fn guard_operand_value<'a>(
    operand: &'a GuardOperand,
    validated: &'a ValidatedInput<'a>,
) -> Result<&'a ScalarValue, ResolutionError> {
    match operand {
        GuardOperand::Parameter(parameter_id) => validated
            .parameter(parameter_id)?
            .value
            .scalar()
            .ok_or_else(|| invalid(format!("guard parameter `{parameter_id}` must be scalar"))),
        GuardOperand::Literal { value, .. } => Ok(value),
    }
}

enum EvaluationFrame<'a> {
    Evaluate(&'a Guard),
    Not,
    And { operands: &'a [Guard], next: usize },
    Or { operands: &'a [Guard], next: usize },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct EvaluationCounts {
    guard_roots: usize,
    comparisons: usize,
}

fn evaluate_guard(
    root: &Guard,
    validated: &ValidatedInput<'_>,
    counts: &mut EvaluationCounts,
) -> Result<bool, ResolutionError> {
    counts.guard_roots = counts
        .guard_roots
        .checked_add(1)
        .ok_or_else(|| resource("guard evaluation count overflow"))?;
    let mut frames = vec![EvaluationFrame::Evaluate(root)];
    let mut values = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            EvaluationFrame::Evaluate(Guard::Compare {
                operator,
                left,
                right,
            }) => {
                counts.comparisons = counts
                    .comparisons
                    .checked_add(1)
                    .ok_or_else(|| resource("guard comparison count overflow"))?;
                values.push(compare_values(
                    *operator,
                    guard_operand_value(left, validated)?,
                    guard_operand_value(right, validated)?,
                )?);
            }
            EvaluationFrame::Evaluate(Guard::Not(operand)) => {
                frames.push(EvaluationFrame::Not);
                frames.push(EvaluationFrame::Evaluate(operand));
            }
            EvaluationFrame::Evaluate(Guard::And(operands)) => {
                let first = operands
                    .first()
                    .ok_or_else(|| invalid("and guard requires at least one operand"))?;
                frames.push(EvaluationFrame::And { operands, next: 1 });
                frames.push(EvaluationFrame::Evaluate(first));
            }
            EvaluationFrame::Evaluate(Guard::Or(operands)) => {
                let first = operands
                    .first()
                    .ok_or_else(|| invalid("or guard requires at least one operand"))?;
                frames.push(EvaluationFrame::Or { operands, next: 1 });
                frames.push(EvaluationFrame::Evaluate(first));
            }
            EvaluationFrame::Not => {
                let value = values
                    .pop()
                    .ok_or_else(|| invalid("guard evaluation state is incomplete"))?;
                values.push(!value);
            }
            EvaluationFrame::And { operands, next } => {
                let value = values
                    .pop()
                    .ok_or_else(|| invalid("guard evaluation state is incomplete"))?;
                if !value {
                    values.push(false);
                } else if let Some(operand) = operands.get(next) {
                    frames.push(EvaluationFrame::And {
                        operands,
                        next: next
                            .checked_add(1)
                            .ok_or_else(|| resource("guard operand index overflow"))?,
                    });
                    frames.push(EvaluationFrame::Evaluate(operand));
                } else {
                    values.push(true);
                }
            }
            EvaluationFrame::Or { operands, next } => {
                let value = values
                    .pop()
                    .ok_or_else(|| invalid("guard evaluation state is incomplete"))?;
                if value {
                    values.push(true);
                } else if let Some(operand) = operands.get(next) {
                    frames.push(EvaluationFrame::Or {
                        operands,
                        next: next
                            .checked_add(1)
                            .ok_or_else(|| resource("guard operand index overflow"))?,
                    });
                    frames.push(EvaluationFrame::Evaluate(operand));
                } else {
                    values.push(false);
                }
            }
        }
    }
    if values.len() != 1 {
        return Err(invalid("guard evaluation state is incomplete"));
    }
    values
        .pop()
        .ok_or_else(|| invalid("guard evaluation state is incomplete"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConnectorState {
    active: bool,
    guard_result: Option<bool>,
}

struct ConnectorStateEvaluation {
    states: Vec<ConnectorState>,
    counts: EvaluationCounts,
}

fn connector_states(
    validated: &ValidatedInput<'_>,
) -> Result<ConnectorStateEvaluation, ResolutionError> {
    let mut states = Vec::new();
    states
        .try_reserve_exact(validated.input.connectors.len())
        .map_err(|_| resource("connector state allocation failed"))?;
    let mut counts = EvaluationCounts::default();
    for connector in &validated.input.connectors {
        match &connector.presence {
            ConnectorPresence::Always => states.push(ConnectorState {
                active: true,
                guard_result: None,
            }),
            ConnectorPresence::Guarded(guard) => {
                let result = evaluate_guard(guard, validated, &mut counts)?;
                states.push(ConnectorState {
                    active: result,
                    guard_result: Some(result),
                });
            }
        }
    }
    Ok(ConnectorStateEvaluation { states, counts })
}

fn checked_leaf_product(left: usize, right: usize) -> Result<usize, ResolutionError> {
    left.checked_mul(right)
        .ok_or_else(|| resource("scalar leaf count overflow"))
}

fn leaf_count(shape: &Shape, validated: &ValidatedInput<'_>) -> Result<usize, ResolutionError> {
    match shape {
        Shape::Scalar => Ok(1),
        Shape::Rank1 { dimension_id } => validated.dimension_extent(dimension_id),
        Shape::Rank2 {
            first_dimension_id,
            second_dimension_id,
        } => checked_leaf_product(
            validated.dimension_extent(first_dimension_id)?,
            validated.dimension_extent(second_dimension_id)?,
        ),
    }
}

fn preflight_scalar_leaves(
    validated: &ValidatedInput<'_>,
    states: &[ConnectorState],
    limit: usize,
) -> Result<usize, ResolutionError> {
    let mut count = 0_usize;
    for parameter in &validated.input.parameters {
        count = count
            .checked_add(leaf_count(&parameter.shape, validated)?)
            .ok_or_else(|| resource("scalar leaf count overflow"))?;
    }
    for (index, connector) in validated.input.connectors.iter().enumerate() {
        let state = states
            .get(index)
            .ok_or_else(|| invalid("connector state count does not match connectors"))?;
        if state.active {
            count = count
                .checked_add(leaf_count(&connector.shape, validated)?)
                .ok_or_else(|| resource("scalar leaf count overflow"))?;
        }
    }
    if count > limit {
        return Err(resource(format!(
            "scalar leaf expansion {count} exceeds limit {limit}"
        )));
    }
    Ok(count)
}

fn resolved_type(
    type_use: &TypeUse,
    validated: &ValidatedInput<'_>,
) -> Result<ResolvedType, ResolutionError> {
    match type_use {
        TypeUse::Primitive(primitive) => Ok(ResolvedType::Primitive(*primitive)),
        TypeUse::Named(type_id) => {
            let definition = validated.type_definition(type_id)?;
            match &definition.definition {
                NamedTypeDefinition::Alias {
                    primitive,
                    quantity,
                    unit,
                    display_unit,
                } => Ok(ResolvedType::Alias {
                    type_id: definition.type_id.clone(),
                    primitive: *primitive,
                    quantity: quantity.clone(),
                    unit: unit.clone(),
                    display_unit: display_unit.clone(),
                }),
                NamedTypeDefinition::Enum { members } => Ok(ResolvedType::Enum {
                    type_id: definition.type_id.clone(),
                    members: members
                        .iter()
                        .map(|member| ResolvedEnumMember {
                            member_id: member.member_id.clone(),
                            symbol: member.symbol.clone(),
                        })
                        .collect(),
                }),
            }
        }
    }
}

fn resolved_scalar(
    value: &ScalarValue,
    type_use: &TypeUse,
    validated: &ValidatedInput<'_>,
) -> Result<ResolvedScalarValue, ResolutionError> {
    match value {
        ScalarValue::Boolean(value) => Ok(ResolvedScalarValue::Boolean(*value)),
        ScalarValue::Integer(value) => Ok(ResolvedScalarValue::Integer(value.clone())),
        ScalarValue::Real(value) => Ok(ResolvedScalarValue::Real(*value)),
        ScalarValue::Enum(value) => {
            let ValueType::Enum(expected_type) = value_type(type_use, validated)? else {
                return Err(invalid("enum value requires an enum type"));
            };
            if value.type_id != expected_type {
                return Err(invalid(format!(
                    "enum value type `{}` does not match `{expected_type}`",
                    value.type_id
                )));
            }
            let definition = validated.type_definition(expected_type)?;
            let NamedTypeDefinition::Enum { members } = &definition.definition else {
                return Err(invalid(format!(
                    "named type `{expected_type}` is not an enum"
                )));
            };
            let member = members
                .iter()
                .find(|member| member.member_id == value.member_id)
                .ok_or_else(|| {
                    invalid(format!(
                        "unknown member `{}` for enum `{expected_type}`",
                        value.member_id
                    ))
                })?;
            Ok(ResolvedScalarValue::Enum(ResolvedEnumValue {
                type_id: value.type_id.clone(),
                member_id: value.member_id.clone(),
                symbol: member.symbol.clone(),
            }))
        }
    }
}

fn coordinate(dimension_id: &str, member_id: &str, ordinal: usize) -> Coordinate {
    Coordinate {
        dimension_id: dimension_id.to_owned(),
        member_id: member_id.to_owned(),
        ordinal,
    }
}

fn parameter_leaves(
    parameter: &ParameterDefinition,
    validated: &ValidatedInput<'_>,
) -> Result<Vec<ScalarParameterLeaf>, ResolutionError> {
    let count = leaf_count(&parameter.shape, validated)?;
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(count)
        .map_err(|_| resource("parameter leaf allocation failed"))?;
    match (&parameter.shape, &parameter.value) {
        (Shape::Scalar, ParameterValue::Scalar(value)) => {
            leaves.push(ScalarParameterLeaf {
                coordinates: Vec::new(),
                value: resolved_scalar(value, &parameter.type_use, validated)?,
            });
        }
        (Shape::Rank1 { dimension_id }, ParameterValue::Rank1(values)) => {
            let dimension = validated.dimension(dimension_id)?;
            for (ordinal, (member_id, value)) in
                dimension.kind.members().iter().zip(values).enumerate()
            {
                leaves.push(ScalarParameterLeaf {
                    coordinates: vec![coordinate(dimension_id, member_id, ordinal)],
                    value: resolved_scalar(value, &parameter.type_use, validated)?,
                });
            }
        }
        (
            Shape::Rank2 {
                first_dimension_id,
                second_dimension_id,
            },
            ParameterValue::Rank2(rows),
        ) => {
            let first = validated.dimension(first_dimension_id)?;
            let second = validated.dimension(second_dimension_id)?;
            for (first_ordinal, (first_member, row)) in
                first.kind.members().iter().zip(rows).enumerate()
            {
                for (second_ordinal, (second_member, value)) in
                    second.kind.members().iter().zip(row).enumerate()
                {
                    leaves.push(ScalarParameterLeaf {
                        coordinates: vec![
                            coordinate(first_dimension_id, first_member, first_ordinal),
                            coordinate(second_dimension_id, second_member, second_ordinal),
                        ],
                        value: resolved_scalar(value, &parameter.type_use, validated)?,
                    });
                }
            }
        }
        _ => {
            return Err(invalid(format!(
                "parameter `{}` value rank does not match its shape",
                parameter.parameter_id
            )));
        }
    }
    if leaves.len() != count {
        return Err(invalid(format!(
            "parameter `{}` leaf count does not match its shape",
            parameter.parameter_id
        )));
    }
    Ok(leaves)
}

fn connector_leaves(
    connector: &ConnectorDefinition,
    validated: &ValidatedInput<'_>,
) -> Result<Vec<ScalarConnectorLeaf>, ResolutionError> {
    let count = leaf_count(&connector.shape, validated)?;
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(count)
        .map_err(|_| resource("connector leaf allocation failed"))?;
    match &connector.shape {
        Shape::Scalar => leaves.push(ScalarConnectorLeaf {
            coordinates: Vec::new(),
        }),
        Shape::Rank1 { dimension_id } => {
            let dimension = validated.dimension(dimension_id)?;
            for (ordinal, member_id) in dimension.kind.members().iter().enumerate() {
                leaves.push(ScalarConnectorLeaf {
                    coordinates: vec![coordinate(dimension_id, member_id, ordinal)],
                });
            }
        }
        Shape::Rank2 {
            first_dimension_id,
            second_dimension_id,
        } => {
            let first = validated.dimension(first_dimension_id)?;
            let second = validated.dimension(second_dimension_id)?;
            for (first_ordinal, first_member) in first.kind.members().iter().enumerate() {
                for (second_ordinal, second_member) in second.kind.members().iter().enumerate() {
                    leaves.push(ScalarConnectorLeaf {
                        coordinates: vec![
                            coordinate(first_dimension_id, first_member, first_ordinal),
                            coordinate(second_dimension_id, second_member, second_ordinal),
                        ],
                    });
                }
            }
        }
    }
    if leaves.len() != count {
        return Err(invalid(format!(
            "connector `{}` leaf count does not match its shape",
            connector.connector_id
        )));
    }
    Ok(leaves)
}

/// Resolves one already-validated typed model without mutating or retaining it.
///
/// Guard resources are checked before typed validation. Scalar resources are
/// checked after each connector guard has been evaluated once and before any
/// resolved leaf vector is allocated. Malformed typed input returns
/// [`ResolutionError::InvalidInput`]; checked count overflow and configured bounds
/// return [`ResolutionError::ResourceLimit`].
pub fn resolve_validated(
    input: &ValidatedResolutionInput,
    limits: ResolutionLimits,
) -> Result<ResolvedSpecialization, ResolutionError> {
    preflight_guards(input, limits)?;
    let validated = validate_input(input)?;
    let ConnectorStateEvaluation { states, counts } = connector_states(&validated)?;
    let _evaluation_counts = counts;
    preflight_scalar_leaves(&validated, &states, limits.max_scalar_leaves)?;

    let mut dimensions = Vec::new();
    dimensions
        .try_reserve_exact(input.dimensions.len())
        .map_err(|_| resource("resolved dimension allocation failed"))?;
    for dimension in &input.dimensions {
        dimensions.push(ResolvedDimension {
            dimension_id: dimension.dimension_id.clone(),
            kind: match dimension.kind {
                DimensionKind::Fixed { .. } => ResolvedDimensionKind::Fixed,
                DimensionKind::ParameterDriven { .. } => ResolvedDimensionKind::Parameter,
            },
            extent: dimension.kind.members().len(),
            members: dimension.kind.members().to_vec(),
        });
    }

    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(input.parameters.len())
        .map_err(|_| resource("resolved parameter allocation failed"))?;
    for parameter in &input.parameters {
        parameters.push(ResolvedParameter {
            parameter_id: parameter.parameter_id.clone(),
            resolved_type: resolved_type(&parameter.type_use, &validated)?,
            dimension_ids: shape_dimension_ids(&parameter.shape),
            source: parameter.source,
            leaves: parameter_leaves(parameter, &validated)?,
        });
    }

    let mut connectors = Vec::new();
    connectors
        .try_reserve_exact(input.connectors.len())
        .map_err(|_| resource("resolved connector allocation failed"))?;
    for (index, connector) in input.connectors.iter().enumerate() {
        let state = states
            .get(index)
            .ok_or_else(|| invalid("connector state count does not match connectors"))?;
        connectors.push(ResolvedConnector {
            connector_id: connector.connector_id.clone(),
            direction: connector.direction,
            resolved_type: resolved_type(&connector.type_use, &validated)?,
            dimension_ids: shape_dimension_ids(&connector.shape),
            active: state.active,
            guard_result: state.guard_result,
            leaves: if state.active {
                connector_leaves(connector, &validated)?
            } else {
                Vec::new()
            },
        });
    }

    Ok(ResolvedSpecialization {
        canonical_id: input.canonical_id.clone(),
        revision: input.revision.clone(),
        dimensions,
        parameters,
        connectors,
    })
}

#[cfg(test)]
mod tests;
