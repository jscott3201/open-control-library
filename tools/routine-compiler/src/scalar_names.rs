//! Projection-scoped scalar-name allocation for the routine compiler.
//!
//! These names are transient labels. They are not Engine paths, IRIs, CXF
//! identifiers, persisted compatibility IDs, source maps, or deployment identities.

use std::fmt;

use num_bigint::BigInt;

use crate::resolution::{ConnectorDirection, ParameterSource, PrimitiveType};
use crate::scalar_abi::{
    ScalarAbiProjection, ScalarAbiType, ScalarAbiValue, ScalarConnectorAbiRow, ScalarCoordinate,
    ScalarParameterAbiRow,
};

/// One parameter ABI row with its transient compiler label.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedScalarParameterRow {
    pub scalar_name: String,
    pub parameter_id: String,
    pub coordinates: Vec<ScalarCoordinate>,
    pub abi_type: ScalarAbiType,
    pub source: ParameterSource,
    pub value: ScalarAbiValue,
}

/// One connector ABI row with its transient compiler label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedScalarConnectorRow {
    pub scalar_name: String,
    pub connector_id: String,
    pub coordinates: Vec<ScalarCoordinate>,
    pub abi_type: ScalarAbiType,
    pub direction: ConnectorDirection,
}

/// Detached named rows for one scalar ABI projection.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedScalarProjection {
    pub canonical_id: String,
    pub revision: BigInt,
    /// Parameter rows remain in scalar ABI order.
    pub parameters: Vec<NamedScalarParameterRow>,
    /// Connector rows remain in scalar ABI order.
    pub connectors: Vec<NamedScalarConnectorRow>,
}

/// One sortable scalar-name validation failure.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScalarNameDiagnostic {
    /// Stable machine-readable category.
    pub code: String,
    /// `projection`, `parameter`, or `connector`.
    pub owner_kind: String,
    /// Stable owner ID, or `$` when no valid ID is available.
    pub owner_id: String,
    /// Projection-relative field or collection location.
    pub location: String,
    pub message: String,
}

impl fmt::Display for ScalarNameDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} {}: {}: {}",
            self.code, self.owner_kind, self.owner_id, self.location, self.message
        )
    }
}

/// Atomic allocation failure with diagnostics sorted by all public fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarNameError {
    pub diagnostics: Vec<ScalarNameDiagnostic>,
}

impl ScalarNameError {
    fn new(mut diagnostics: Vec<ScalarNameDiagnostic>) -> Self {
        diagnostics.sort_unstable();
        Self { diagnostics }
    }
}

impl fmt::Display for ScalarNameError {
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

impl std::error::Error for ScalarNameError {}

fn diagnostic(
    code: &str,
    owner_kind: &str,
    owner_id: &str,
    location: &str,
    message: impl Into<String>,
) -> ScalarNameDiagnostic {
    ScalarNameDiagnostic {
        code: code.to_owned(),
        owner_kind: owner_kind.to_owned(),
        owner_id: if owner_id.is_empty() {
            "$".to_owned()
        } else {
            owner_id.to_owned()
        },
        location: location.to_owned(),
        message: message.into(),
    }
}

fn resource_diagnostic(
    owner_kind: &str,
    owner_id: &str,
    location: &str,
    message: &str,
) -> ScalarNameDiagnostic {
    diagnostic("resource_limit", owner_kind, owner_id, location, message)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NameResourceFailure {
    LengthOverflow,
    AllocationFailed,
}

impl NameResourceFailure {
    fn message(self) -> &'static str {
        match self {
            Self::LengthOverflow => "scalar name length overflows usize",
            Self::AllocationFailed => "scalar name allocation failed",
        }
    }
}

fn checked_encoded_name_length(
    prefix_len: usize,
    component_byte_lengths: impl IntoIterator<Item = usize>,
) -> Option<usize> {
    let mut length = prefix_len;
    let mut component_count = 0_usize;
    for byte_length in component_byte_lengths {
        if component_count != 0 {
            length = length.checked_add(1)?;
        }
        length = length.checked_add(byte_length.checked_mul(2)?)?;
        component_count = component_count.checked_add(1)?;
    }
    Some(length)
}

fn reserve_name_buffer(buffer: &mut String, length: usize) -> Result<(), NameResourceFailure> {
    buffer
        .try_reserve_exact(length)
        .map_err(|_| NameResourceFailure::AllocationFailed)
}

fn push_hex_component(name: &mut String, component: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in component.as_bytes() {
        name.push(HEX[(byte >> 4) as usize] as char);
        name.push(HEX[(byte & 0x0f) as usize] as char);
    }
}

fn build_scalar_name(
    prefix: &str,
    owner_id: &str,
    coordinates: &[ScalarCoordinate],
) -> Result<String, NameResourceFailure> {
    let component_lengths = std::iter::once(owner_id.len()).chain(
        coordinates
            .iter()
            .flat_map(|coordinate| [coordinate.dimension_id.len(), coordinate.member_id.len()]),
    );
    let length = checked_encoded_name_length(prefix.len(), component_lengths)
        .ok_or(NameResourceFailure::LengthOverflow)?;
    let mut name = String::new();
    reserve_name_buffer(&mut name, length)?;
    name.push_str(prefix);
    push_hex_component(&mut name, owner_id);
    for coordinate in coordinates {
        name.push('_');
        push_hex_component(&mut name, &coordinate.dimension_id);
        name.push('_');
        push_hex_component(&mut name, &coordinate.member_id);
    }
    debug_assert_eq!(name.len(), length);
    Ok(name)
}

fn validate_name_components(
    prefix: &str,
    owner_kind: &str,
    owner_id: &str,
    owner_field: &str,
    coordinates: &[ScalarCoordinate],
    location: &str,
    diagnostics: &mut Vec<ScalarNameDiagnostic>,
) -> Option<String> {
    let mut valid = true;
    if owner_id.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_owner_id",
            owner_kind,
            owner_id,
            &format!("{location}.{owner_field}"),
            format!("{owner_field} must not be empty"),
        ));
        valid = false;
    }
    for (index, coordinate) in coordinates.iter().enumerate() {
        let coordinate_location = format!("{location}.coordinates[{index}]");
        if coordinate.dimension_id.is_empty() {
            diagnostics.push(diagnostic(
                "invalid_dimension_id",
                owner_kind,
                owner_id,
                &format!("{coordinate_location}.dimension_id"),
                "dimension_id must not be empty",
            ));
            valid = false;
        }
        if coordinate.member_id.is_empty() {
            diagnostics.push(diagnostic(
                "invalid_member_id",
                owner_kind,
                owner_id,
                &format!("{coordinate_location}.member_id"),
                "member_id must not be empty",
            ));
            valid = false;
        }
    }
    if !valid {
        return None;
    }
    match build_scalar_name(prefix, owner_id, coordinates) {
        Ok(name) => Some(name),
        Err(failure) => {
            diagnostics.push(resource_diagnostic(
                owner_kind,
                owner_id,
                location,
                failure.message(),
            ));
            None
        }
    }
}

fn validate_abi_type(
    abi_type: &ScalarAbiType,
    owner_kind: &str,
    owner_id: &str,
    location: &str,
    diagnostics: &mut Vec<ScalarNameDiagnostic>,
) {
    match abi_type {
        ScalarAbiType::Primitive(_) => {}
        ScalarAbiType::Alias { type_id, .. } if type_id.is_empty() => {
            diagnostics.push(diagnostic(
                "invalid_abi_payload",
                owner_kind,
                owner_id,
                &format!("{location}.abi_type.type_id"),
                "alias type ID must not be empty",
            ));
        }
        ScalarAbiType::Enum {
            canonical_class_path,
        } if canonical_class_path.is_empty() => {
            diagnostics.push(diagnostic(
                "invalid_abi_payload",
                owner_kind,
                owner_id,
                &format!("{location}.abi_type.canonical_class_path"),
                "enum canonical class path must not be empty",
            ));
        }
        ScalarAbiType::Alias { .. } | ScalarAbiType::Enum { .. } => {}
    }
}

fn primitive_accepts_value(primitive: PrimitiveType, value: &ScalarAbiValue) -> bool {
    matches!(
        (primitive, value),
        (PrimitiveType::Boolean, ScalarAbiValue::Boolean(_))
            | (PrimitiveType::Integer, ScalarAbiValue::Integer(_))
            | (
                PrimitiveType::Real,
                ScalarAbiValue::Integer(_) | ScalarAbiValue::Real(_)
            )
    )
}

fn abi_type_accepts_value(abi_type: &ScalarAbiType, value: &ScalarAbiValue) -> bool {
    match abi_type {
        ScalarAbiType::Primitive(primitive) | ScalarAbiType::Alias { primitive, .. } => {
            primitive_accepts_value(*primitive, value)
        }
        ScalarAbiType::Enum { .. } => matches!(value, ScalarAbiValue::Enum { .. }),
    }
}

fn validate_parameter_value(
    row: &ScalarParameterAbiRow,
    location: &str,
    diagnostics: &mut Vec<ScalarNameDiagnostic>,
) {
    if let ScalarAbiValue::Enum { ordinal: 0 } = row.value {
        diagnostics.push(diagnostic(
            "invalid_abi_payload",
            "parameter",
            &row.parameter_id,
            &format!("{location}.value.ordinal"),
            "enum ordinal must be one-based",
        ));
    }
    if !abi_type_accepts_value(&row.abi_type, &row.value) {
        diagnostics.push(diagnostic(
            "invalid_abi_payload",
            "parameter",
            &row.parameter_id,
            &format!("{location}.value"),
            "parameter value does not match its scalar ABI type",
        ));
    }
}

#[derive(Debug)]
struct NameCandidate<'a> {
    row_index: usize,
    owner_id: &'a str,
    scalar_name: String,
}

fn parameter_candidates<'a>(
    rows: &'a [ScalarParameterAbiRow],
    diagnostics: &mut Vec<ScalarNameDiagnostic>,
) -> Vec<NameCandidate<'a>> {
    let mut candidates = Vec::new();
    let retain_names = if candidates.try_reserve_exact(rows.len()).is_ok() {
        true
    } else {
        diagnostics.push(resource_diagnostic(
            "projection",
            "$",
            "$.parameters",
            "parameter name candidate allocation failed",
        ));
        false
    };
    for (index, row) in rows.iter().enumerate() {
        let location = format!("$.parameters[{index}]");
        let scalar_name = validate_name_components(
            "p_",
            "parameter",
            &row.parameter_id,
            "parameter_id",
            &row.coordinates,
            &location,
            diagnostics,
        );
        validate_abi_type(
            &row.abi_type,
            "parameter",
            &row.parameter_id,
            &location,
            diagnostics,
        );
        validate_parameter_value(row, &location, diagnostics);
        if retain_names && let Some(scalar_name) = scalar_name {
            candidates.push(NameCandidate {
                row_index: index,
                owner_id: &row.parameter_id,
                scalar_name,
            });
        }
    }
    candidates
}

fn connector_candidates<'a>(
    rows: &'a [ScalarConnectorAbiRow],
    diagnostics: &mut Vec<ScalarNameDiagnostic>,
) -> Vec<NameCandidate<'a>> {
    let mut candidates = Vec::new();
    let retain_names = if candidates.try_reserve_exact(rows.len()).is_ok() {
        true
    } else {
        diagnostics.push(resource_diagnostic(
            "projection",
            "$",
            "$.connectors",
            "connector name candidate allocation failed",
        ));
        false
    };
    for (index, row) in rows.iter().enumerate() {
        let location = format!("$.connectors[{index}]");
        let scalar_name = validate_name_components(
            "c_",
            "connector",
            &row.connector_id,
            "connector_id",
            &row.coordinates,
            &location,
            diagnostics,
        );
        validate_abi_type(
            &row.abi_type,
            "connector",
            &row.connector_id,
            &location,
            diagnostics,
        );
        if retain_names && let Some(scalar_name) = scalar_name {
            candidates.push(NameCandidate {
                row_index: index,
                owner_id: &row.connector_id,
                scalar_name,
            });
        }
    }
    candidates
}

fn add_duplicate_diagnostics(
    candidates: &mut [NameCandidate<'_>],
    owner_kind: &str,
    location: &str,
    diagnostics: &mut Vec<ScalarNameDiagnostic>,
) {
    candidates.sort_unstable_by(|left, right| {
        left.scalar_name
            .cmp(&right.scalar_name)
            .then_with(|| left.owner_id.cmp(right.owner_id))
            .then_with(|| left.row_index.cmp(&right.row_index))
    });
    let mut start = 0_usize;
    while start < candidates.len() {
        let mut end = start + 1;
        while end < candidates.len() && candidates[end].scalar_name == candidates[start].scalar_name
        {
            end += 1;
        }
        if end - start > 1 {
            diagnostics.push(diagnostic(
                "duplicate_scalar_name",
                owner_kind,
                candidates[start].owner_id,
                location,
                format!(
                    "generated scalar name `{}` occurs {} times",
                    candidates[start].scalar_name,
                    end - start
                ),
            ));
        }
        start = end;
    }
}

fn reserve_output_rows<T>(
    rows: &mut Vec<T>,
    count: usize,
    location: &str,
    label: &str,
) -> Option<ScalarNameDiagnostic> {
    rows.try_reserve_exact(count).err().map(|_| {
        resource_diagnostic(
            "projection",
            "$",
            location,
            &format!("{label} row allocation failed"),
        )
    })
}

/// Assigns transient names without I/O or mutation of the scalar ABI projection.
///
/// The complete projection is validated, including duplicate names within each
/// namespace, before any named output row is constructed. Coordinate ordinals are
/// copied but do not participate in names.
pub fn allocate_scalar_names(
    projection: &ScalarAbiProjection,
) -> Result<NamedScalarProjection, ScalarNameError> {
    let mut diagnostics = Vec::new();
    if projection.canonical_id.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_metadata",
            "projection",
            "$",
            "$.canonical_id",
            "canonical_id must not be empty",
        ));
    }
    if projection.revision <= BigInt::from(0_u8) {
        diagnostics.push(diagnostic(
            "invalid_metadata",
            "projection",
            "$",
            "$.revision",
            "revision must be positive",
        ));
    }

    let mut parameter_candidates = parameter_candidates(&projection.parameters, &mut diagnostics);
    let mut connector_candidates = connector_candidates(&projection.connectors, &mut diagnostics);
    add_duplicate_diagnostics(
        &mut parameter_candidates,
        "parameter",
        "$.parameters",
        &mut diagnostics,
    );
    add_duplicate_diagnostics(
        &mut connector_candidates,
        "connector",
        "$.connectors",
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(ScalarNameError::new(diagnostics));
    }

    parameter_candidates.sort_unstable_by_key(|candidate| candidate.row_index);
    connector_candidates.sort_unstable_by_key(|candidate| candidate.row_index);

    let mut parameters = Vec::new();
    let mut connectors = Vec::new();
    if let Some(diagnostic) = reserve_output_rows(
        &mut parameters,
        projection.parameters.len(),
        "$.parameters",
        "parameter",
    ) {
        diagnostics.push(diagnostic);
    }
    if let Some(diagnostic) = reserve_output_rows(
        &mut connectors,
        projection.connectors.len(),
        "$.connectors",
        "connector",
    ) {
        diagnostics.push(diagnostic);
    }
    if !diagnostics.is_empty() {
        return Err(ScalarNameError::new(diagnostics));
    }

    for (row, candidate) in projection.parameters.iter().zip(parameter_candidates) {
        parameters.push(NamedScalarParameterRow {
            scalar_name: candidate.scalar_name,
            parameter_id: row.parameter_id.clone(),
            coordinates: row.coordinates.clone(),
            abi_type: row.abi_type.clone(),
            source: row.source,
            value: row.value.clone(),
        });
    }
    for (row, candidate) in projection.connectors.iter().zip(connector_candidates) {
        connectors.push(NamedScalarConnectorRow {
            scalar_name: candidate.scalar_name,
            connector_id: row.connector_id.clone(),
            coordinates: row.coordinates.clone(),
            abi_type: row.abi_type.clone(),
            direction: row.direction,
        });
    }

    Ok(NamedScalarProjection {
        canonical_id: projection.canonical_id.clone(),
        revision: projection.revision.clone(),
        parameters,
        connectors,
    })
}

#[cfg(test)]
mod tests;
