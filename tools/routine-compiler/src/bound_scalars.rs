//! Validated internal join of named scalar ABI rows and source claims.
//!
//! Named rows supply order and ABI data. Source rows supply caller claims for
//! Modelica classes and members. This stage neither checks declarations nor
//! defines a serialized contract.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::hash::Hash;

use num_bigint::BigInt;

use crate::resolution::{ConnectorDirection, ParameterSource, PrimitiveType};
use crate::scalar_abi::{ScalarAbiType, ScalarAbiValue, ScalarCoordinate};
use crate::scalar_names::{
    NamedScalarConnectorRow, NamedScalarParameterRow, NamedScalarProjection, build_scalar_name,
};
use crate::scalar_source_claims::{
    ScalarConnectorSourceClaim, ScalarParameterSourceClaim, ScalarSourceClaimProjection,
    SourceFileLocator, SourceSnapshotRole,
};

const SOURCE_ROOT_PREFIX: &str = "Buildings/Controls/OBC/ASHRAE/G36/";
const CLASS_PATH_PREFIX: &str = "Buildings.Controls.OBC.ASHRAE.G36.";
const MAX_IDENTIFIER_LENGTH: usize = 255;
const MAX_CLASS_PATH_LENGTH: usize = 1024;

/// Detached caller claim attached to one bound scalar row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundSourceClaim {
    pub canonical_class_path: String,
    pub source_member: String,
    pub snapshot: SourceSnapshotRole,
    /// Source Git revision; this is independent of the routine revision.
    pub revision: String,
    pub file: SourceFileLocator,
}

/// One named parameter ABI row with its source claim.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundScalarParameterRow {
    pub scalar_name: String,
    pub parameter_id: String,
    pub coordinates: Vec<ScalarCoordinate>,
    pub abi_type: ScalarAbiType,
    pub source: ParameterSource,
    pub value: ScalarAbiValue,
    pub source_claim: BoundSourceClaim,
}

/// One named connector ABI row with its source claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundScalarConnectorRow {
    pub scalar_name: String,
    pub connector_id: String,
    pub coordinates: Vec<ScalarCoordinate>,
    pub abi_type: ScalarAbiType,
    pub direction: ConnectorDirection,
    pub source_claim: BoundSourceClaim,
}

/// Borrowed result of a bound scalar-name lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoundScalarRef<'a> {
    Parameter(&'a BoundScalarParameterRow),
    Connector(&'a BoundScalarConnectorRow),
}

/// Detached bound rows in named projection order.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundScalarProjection {
    pub canonical_id: String,
    pub revision: BigInt,
    pub parameters: Vec<BoundScalarParameterRow>,
    pub connectors: Vec<BoundScalarConnectorRow>,
}

impl BoundScalarProjection {
    /// Finds a bound row without storing a reverse index.
    pub fn row_for_scalar(&self, scalar_name: &str) -> Option<BoundScalarRef<'_>> {
        self.parameters
            .iter()
            .find(|row| row.scalar_name == scalar_name)
            .map(BoundScalarRef::Parameter)
            .or_else(|| {
                self.connectors
                    .iter()
                    .find(|row| row.scalar_name == scalar_name)
                    .map(BoundScalarRef::Connector)
            })
    }
}

/// One sortable refusal from scalar binding validation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundScalarDiagnostic {
    pub code: String,
    pub owner_kind: String,
    pub owner_id: String,
    pub location: String,
    pub message: String,
}

impl fmt::Display for BoundScalarDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} {}: {}: {}",
            self.code, self.owner_kind, self.owner_id, self.location, self.message
        )
    }
}

/// Atomic binding failure with diagnostics sorted by every public field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundScalarError {
    pub diagnostics: Vec<BoundScalarDiagnostic>,
}

impl BoundScalarError {
    fn new(mut diagnostics: Vec<BoundScalarDiagnostic>) -> Self {
        diagnostics.sort_unstable();
        Self { diagnostics }
    }
}

impl fmt::Display for BoundScalarError {
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

impl std::error::Error for BoundScalarError {}

fn diagnostic(
    code: &str,
    owner_kind: &str,
    owner_id: &str,
    location: &str,
    message: impl Into<String>,
) -> BoundScalarDiagnostic {
    BoundScalarDiagnostic {
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

fn resource_diagnostic(location: &str, message: &str) -> BoundScalarDiagnostic {
    diagnostic("resource_limit", "projection", "$", location, message)
}

fn checked_total_count(lengths: impl IntoIterator<Item = usize>) -> Option<usize> {
    lengths
        .into_iter()
        .try_fold(0_usize, |total, length| total.checked_add(length))
}

fn reserve_map<K: Eq + Hash, V>(
    map: &mut HashMap<K, V>,
    count: usize,
    location: &str,
    label: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) -> bool {
    if map.try_reserve(count).is_ok() {
        true
    } else {
        diagnostics.push(resource_diagnostic(
            location,
            &format!("{label} allocation failed"),
        ));
        false
    }
}

#[derive(Clone, Copy)]
enum ScalarKind {
    Parameter,
    Connector,
}

impl ScalarKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::Connector => "connector",
        }
    }

    fn plural(self) -> &'static str {
        match self {
            Self::Parameter => "parameters",
            Self::Connector => "connectors",
        }
    }

    fn owner_field(self) -> &'static str {
        match self {
            Self::Parameter => "parameter_id",
            Self::Connector => "connector_id",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Parameter => "p_",
            Self::Connector => "c_",
        }
    }
}

#[derive(Clone, Copy)]
struct IndexedRow<'a> {
    owner_id: &'a str,
    coordinates: &'a [ScalarCoordinate],
}

#[derive(Clone, Copy)]
struct RowGroup<'a> {
    count: usize,
    first: IndexedRow<'a>,
    diagnostic_owner: &'a str,
}

type RowIndex<'a> = HashMap<&'a str, RowGroup<'a>>;

struct ProjectionIndexes<'a> {
    parameters: RowIndex<'a>,
    connectors: RowIndex<'a>,
    retain_parameters: bool,
    retain_connectors: bool,
}

fn index_row<'a>(
    index: &mut RowIndex<'a>,
    retain: bool,
    scalar_name: &'a str,
    owner_id: &'a str,
    coordinates: &'a [ScalarCoordinate],
    location: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    if !retain || scalar_name.is_empty() {
        return;
    }
    let row = IndexedRow {
        owner_id,
        coordinates,
    };
    match index.entry(scalar_name) {
        Entry::Vacant(entry) => {
            entry.insert(RowGroup {
                count: 1,
                first: row,
                diagnostic_owner: owner_id,
            });
        }
        Entry::Occupied(mut entry) => {
            let group = entry.get_mut();
            match group.count.checked_add(1) {
                Some(count) => group.count = count,
                None => diagnostics.push(resource_diagnostic(
                    location,
                    "scalar name count overflows usize",
                )),
            }
            let current_owner = if group.diagnostic_owner.is_empty() {
                "$"
            } else {
                group.diagnostic_owner
            };
            let next_owner = if owner_id.is_empty() { "$" } else { owner_id };
            if next_owner < current_owner {
                group.diagnostic_owner = owner_id;
            }
        }
    }
}

fn validate_metadata(
    canonical_id: &str,
    revision: &BigInt,
    label: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    if canonical_id.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_metadata",
            "projection",
            label,
            &format!("$.{label}.canonical_id"),
            "canonical_id must not be empty",
        ));
    }
    if revision <= &BigInt::from(0_u8) {
        diagnostics.push(diagnostic(
            "invalid_metadata",
            "projection",
            label,
            &format!("$.{label}.revision"),
            "revision must be positive",
        ));
    }
}

fn validate_scalar_identity(
    scalar_name: &str,
    owner_id: &str,
    coordinates: &[ScalarCoordinate],
    kind: ScalarKind,
    location: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    let owner_kind = kind.as_str();
    let mut components_valid = true;
    if owner_id.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_owner_id",
            owner_kind,
            owner_id,
            &format!("{location}.{}", kind.owner_field()),
            format!("{} must not be empty", kind.owner_field()),
        ));
        components_valid = false;
    }
    if scalar_name.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_scalar_name",
            owner_kind,
            owner_id,
            &format!("{location}.scalar_name"),
            "scalar_name must not be empty",
        ));
    } else if !scalar_name.starts_with(kind.prefix()) {
        diagnostics.push(diagnostic(
            "scalar_name_namespace",
            owner_kind,
            owner_id,
            &format!("{location}.scalar_name"),
            format!(
                "{owner_kind} scalar names must start with `{}`",
                kind.prefix()
            ),
        ));
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
            components_valid = false;
        }
        if coordinate.member_id.is_empty() {
            diagnostics.push(diagnostic(
                "invalid_member_id",
                owner_kind,
                owner_id,
                &format!("{coordinate_location}.member_id"),
                "member_id must not be empty",
            ));
            components_valid = false;
        }
    }
    if components_valid {
        match build_scalar_name(kind.prefix(), owner_id, coordinates) {
            Ok(expected) if expected != scalar_name => diagnostics.push(diagnostic(
                "scalar_name_mismatch",
                owner_kind,
                owner_id,
                &format!("{location}.scalar_name"),
                "scalar name does not match its owner and stable coordinates",
            )),
            Ok(_) => {}
            Err(failure) => diagnostics.push(resource_diagnostic(location, failure.message())),
        }
    }
}

fn validate_optional_alias_metadata(
    value: &Option<String>,
    field: &str,
    kind: ScalarKind,
    owner_id: &str,
    location: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    if let Some(value) = value
        && (value.is_empty() || value.trim() != value)
    {
        diagnostics.push(diagnostic(
            "invalid_abi_payload",
            kind.as_str(),
            owner_id,
            &format!("{location}.abi_type.{field}"),
            format!("alias {field} must be nonempty and trimmed"),
        ));
    }
}

fn validate_abi_type(
    abi_type: &ScalarAbiType,
    kind: ScalarKind,
    owner_id: &str,
    location: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    match abi_type {
        ScalarAbiType::Primitive(_) => {}
        ScalarAbiType::Alias {
            type_id,
            quantity,
            unit,
            display_unit,
            ..
        } => {
            if type_id.is_empty() {
                diagnostics.push(diagnostic(
                    "invalid_abi_payload",
                    kind.as_str(),
                    owner_id,
                    &format!("{location}.abi_type.type_id"),
                    "alias type ID must not be empty",
                ));
            }
            validate_optional_alias_metadata(
                quantity,
                "quantity",
                kind,
                owner_id,
                location,
                diagnostics,
            );
            validate_optional_alias_metadata(unit, "unit", kind, owner_id, location, diagnostics);
            validate_optional_alias_metadata(
                display_unit,
                "display_unit",
                kind,
                owner_id,
                location,
                diagnostics,
            );
        }
        ScalarAbiType::Enum {
            canonical_class_path,
        } if canonical_class_path.is_empty() => diagnostics.push(diagnostic(
            "invalid_abi_payload",
            kind.as_str(),
            owner_id,
            &format!("{location}.abi_type.canonical_class_path"),
            "enum canonical class path must not be empty",
        )),
        ScalarAbiType::Enum { .. } => {}
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
    row: &NamedScalarParameterRow,
    location: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
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

fn validate_named_projection<'a>(
    projection: &'a NamedScalarProjection,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) -> ProjectionIndexes<'a> {
    validate_metadata(
        &projection.canonical_id,
        &projection.revision,
        "named_projection",
        diagnostics,
    );
    let mut parameters = HashMap::new();
    let retain_parameters = reserve_map(
        &mut parameters,
        projection.parameters.len(),
        "$.named_projection.parameters",
        "parameter scalar index",
        diagnostics,
    );
    let mut connectors = HashMap::new();
    let retain_connectors = reserve_map(
        &mut connectors,
        projection.connectors.len(),
        "$.named_projection.connectors",
        "connector scalar index",
        diagnostics,
    );

    for (index, row) in projection.parameters.iter().enumerate() {
        let location = format!("$.named_projection.parameters[{index}]");
        validate_scalar_identity(
            &row.scalar_name,
            &row.parameter_id,
            &row.coordinates,
            ScalarKind::Parameter,
            &location,
            diagnostics,
        );
        validate_abi_type(
            &row.abi_type,
            ScalarKind::Parameter,
            &row.parameter_id,
            &location,
            diagnostics,
        );
        validate_parameter_value(row, &location, diagnostics);
        index_row(
            &mut parameters,
            retain_parameters,
            &row.scalar_name,
            &row.parameter_id,
            &row.coordinates,
            "$.named_projection.parameters",
            diagnostics,
        );
    }
    for (index, row) in projection.connectors.iter().enumerate() {
        let location = format!("$.named_projection.connectors[{index}]");
        validate_scalar_identity(
            &row.scalar_name,
            &row.connector_id,
            &row.coordinates,
            ScalarKind::Connector,
            &location,
            diagnostics,
        );
        validate_abi_type(
            &row.abi_type,
            ScalarKind::Connector,
            &row.connector_id,
            &location,
            diagnostics,
        );
        index_row(
            &mut connectors,
            retain_connectors,
            &row.scalar_name,
            &row.connector_id,
            &row.coordinates,
            "$.named_projection.connectors",
            diagnostics,
        );
    }

    ProjectionIndexes {
        parameters,
        connectors,
        retain_parameters,
        retain_connectors,
    }
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_revision(value: &str) -> bool {
    is_lower_hex(value, 40)
}

fn is_sha1(value: &str) -> bool {
    value
        .strip_prefix("sha1:")
        .is_some_and(|digest| is_lower_hex(digest, 40))
}

fn is_modelica_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_LENGTH || !value.is_ascii() {
        return false;
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn is_class_path(value: &str) -> bool {
    if value.len() > MAX_CLASS_PATH_LENGTH {
        return false;
    }
    let Some(suffix) = value.strip_prefix(CLASS_PATH_PREFIX) else {
        return false;
    };
    !suffix.is_empty() && suffix.split('.').all(is_modelica_identifier)
}

fn safe_source_path(path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("path must not be empty");
    }
    if path.starts_with('/') {
        return Err("absolute paths are forbidden");
    }
    if path.contains('\\') {
        return Err("backslashes are forbidden");
    }
    if path
        .chars()
        .any(|character| character < ' ' || character == '\u{7f}')
    {
        return Err("control characters are forbidden");
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err("empty path segments are forbidden");
        }
        if segment == "." {
            return Err("dot path segments are forbidden");
        }
        if segment == ".." {
            return Err("parent traversal is forbidden");
        }
    }
    if !path.starts_with(SOURCE_ROOT_PREFIX) {
        return Err("path must be below the governed G36 source root");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_source_payload(
    canonical_class_path: &str,
    source_member: &str,
    revision: &str,
    file: &SourceFileLocator,
    kind: ScalarKind,
    owner_id: &str,
    location: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    if !is_class_path(canonical_class_path) {
        diagnostics.push(diagnostic(
            "invalid_source_class",
            kind.as_str(),
            owner_id,
            &format!("{location}.canonical_class_path"),
            "canonical_class_path must be a bounded class below the G36 package",
        ));
    }
    if !is_modelica_identifier(source_member) {
        diagnostics.push(diagnostic(
            "invalid_source_member",
            kind.as_str(),
            owner_id,
            &format!("{location}.source_member"),
            "source_member must be a bounded ASCII Modelica identifier",
        ));
    }
    if !is_revision(revision) {
        diagnostics.push(diagnostic(
            "invalid_source_revision",
            kind.as_str(),
            owner_id,
            &format!("{location}.revision"),
            "source revision must be 40 lowercase hexadecimal characters",
        ));
    }
    match safe_source_path(&file.path) {
        Ok(()) if !file.path.ends_with(".mo") => diagnostics.push(diagnostic(
            "invalid_source_path",
            kind.as_str(),
            owner_id,
            &format!("{location}.file.path"),
            "source file path must end in `.mo`",
        )),
        Ok(()) => {}
        Err(problem) => diagnostics.push(diagnostic(
            "invalid_source_path",
            kind.as_str(),
            owner_id,
            &format!("{location}.file.path"),
            problem,
        )),
    }
    if !is_sha1(&file.git_blob_sha1) {
        diagnostics.push(diagnostic(
            "invalid_source_blob",
            kind.as_str(),
            owner_id,
            &format!("{location}.file.git_blob_sha1"),
            "git_blob_sha1 must match sha1:<40 lowercase hex>",
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_source_row<'a>(
    scalar_name: &'a str,
    owner_id: &'a str,
    coordinates: &'a [ScalarCoordinate],
    canonical_class_path: &str,
    source_member: &str,
    revision: &str,
    file: &SourceFileLocator,
    kind: ScalarKind,
    location: &str,
    index: &mut RowIndex<'a>,
    retain: bool,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    validate_scalar_identity(
        scalar_name,
        owner_id,
        coordinates,
        kind,
        location,
        diagnostics,
    );
    validate_source_payload(
        canonical_class_path,
        source_member,
        revision,
        file,
        kind,
        owner_id,
        location,
        diagnostics,
    );
    index_row(
        index,
        retain,
        scalar_name,
        owner_id,
        coordinates,
        &format!("$.source_claim_projection.{}", kind.plural()),
        diagnostics,
    );
}

fn validate_source_projection<'a>(
    projection: &'a ScalarSourceClaimProjection,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) -> ProjectionIndexes<'a> {
    validate_metadata(
        &projection.canonical_id,
        &projection.revision,
        "source_claim_projection",
        diagnostics,
    );
    let mut parameters = HashMap::new();
    let retain_parameters = reserve_map(
        &mut parameters,
        projection.parameters.len(),
        "$.source_claim_projection.parameters",
        "parameter claim index",
        diagnostics,
    );
    let mut connectors = HashMap::new();
    let retain_connectors = reserve_map(
        &mut connectors,
        projection.connectors.len(),
        "$.source_claim_projection.connectors",
        "connector claim index",
        diagnostics,
    );

    for (index, row) in projection.parameters.iter().enumerate() {
        let location = format!("$.source_claim_projection.parameters[{index}]");
        validate_source_row(
            &row.scalar_name,
            &row.parameter_id,
            &row.coordinates,
            &row.canonical_class_path,
            &row.source_member,
            &row.revision,
            &row.file,
            ScalarKind::Parameter,
            &location,
            &mut parameters,
            retain_parameters,
            diagnostics,
        );
    }
    for (index, row) in projection.connectors.iter().enumerate() {
        let location = format!("$.source_claim_projection.connectors[{index}]");
        validate_source_row(
            &row.scalar_name,
            &row.connector_id,
            &row.coordinates,
            &row.canonical_class_path,
            &row.source_member,
            &row.revision,
            &row.file,
            ScalarKind::Connector,
            &location,
            &mut connectors,
            retain_connectors,
            diagnostics,
        );
    }

    ProjectionIndexes {
        parameters,
        connectors,
        retain_parameters,
        retain_connectors,
    }
}

fn validate_duplicates(
    label: &str,
    kind: ScalarKind,
    index: &RowIndex<'_>,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    for group in index.values() {
        if group.count > 1 {
            diagnostics.push(diagnostic(
                "duplicate_scalar_name",
                kind.as_str(),
                group.diagnostic_owner,
                &format!("$.{label}.{}", kind.plural()),
                format!("scalar name occurs {} times", group.count),
            ));
        }
    }
}

fn validate_cross_kind_collisions(
    label: &str,
    indexes: &ProjectionIndexes<'_>,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    if !indexes.retain_parameters || !indexes.retain_connectors {
        return;
    }
    for scalar_name in indexes.parameters.keys() {
        if indexes.connectors.contains_key(scalar_name) {
            diagnostics.push(diagnostic(
                "cross_kind_collision",
                "projection",
                "$",
                &format!("$.{label}"),
                "one scalar name occurs in both namespaces",
            ));
        }
    }
}

fn compare_coordinates(
    named: &[ScalarCoordinate],
    claim: &[ScalarCoordinate],
    kind: ScalarKind,
    owner_id: &str,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    if named.len() != claim.len() {
        diagnostics.push(diagnostic(
            "coordinate_count_mismatch",
            kind.as_str(),
            owner_id,
            "$.join.coordinates",
            "named and source-claim coordinate counts differ",
        ));
    }
    for (index, (named, claim)) in named.iter().zip(claim).enumerate() {
        let location = format!("$.join.coordinates[{index}]");
        if named.dimension_id != claim.dimension_id {
            diagnostics.push(diagnostic(
                "dimension_mismatch",
                kind.as_str(),
                owner_id,
                &format!("{location}.dimension_id"),
                "named and source-claim dimension IDs differ",
            ));
        }
        if named.member_id != claim.member_id {
            diagnostics.push(diagnostic(
                "member_mismatch",
                kind.as_str(),
                owner_id,
                &format!("{location}.member_id"),
                "named and source-claim member IDs differ",
            ));
        }
        if named.ordinal != claim.ordinal {
            diagnostics.push(diagnostic(
                "ordinal_mismatch",
                kind.as_str(),
                owner_id,
                &format!("{location}.ordinal"),
                "named and source-claim ordinals differ",
            ));
        }
    }
}

fn validate_namespace_join(
    kind: ScalarKind,
    named: &RowIndex<'_>,
    claims: &RowIndex<'_>,
    opposite_named: &RowIndex<'_>,
    opposite_claims: &RowIndex<'_>,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    for (scalar_name, named_group) in named {
        match claims.get(scalar_name) {
            None => {
                diagnostics.push(diagnostic(
                    "missing_source_claim",
                    kind.as_str(),
                    named_group.first.owner_id,
                    &format!("$.source_claim_projection.{}", kind.plural()),
                    "no source claim matches the named scalar",
                ));
                if opposite_claims.contains_key(scalar_name) {
                    diagnostics.push(diagnostic(
                        "namespace_confusion",
                        kind.as_str(),
                        named_group.first.owner_id,
                        "$.join",
                        "matching source claim occurs in the opposite namespace",
                    ));
                }
            }
            Some(claim_group) if named_group.count == 1 && claim_group.count == 1 => {
                if named_group.first.owner_id != claim_group.first.owner_id {
                    diagnostics.push(diagnostic(
                        "owner_mismatch",
                        kind.as_str(),
                        named_group.first.owner_id,
                        &format!("$.join.{}", kind.owner_field()),
                        "named and source-claim owner IDs differ",
                    ));
                }
                compare_coordinates(
                    named_group.first.coordinates,
                    claim_group.first.coordinates,
                    kind,
                    named_group.first.owner_id,
                    diagnostics,
                );
            }
            Some(_) => {}
        }
    }
    for (scalar_name, claim_group) in claims {
        if !named.contains_key(scalar_name) {
            diagnostics.push(diagnostic(
                "extra_source_claim",
                kind.as_str(),
                claim_group.first.owner_id,
                &format!("$.source_claim_projection.{}", kind.plural()),
                "source claim scalar has no named row",
            ));
            if opposite_named.contains_key(scalar_name) {
                diagnostics.push(diagnostic(
                    "namespace_confusion",
                    kind.as_str(),
                    claim_group.first.owner_id,
                    "$.join",
                    "source claim matches a named row in the opposite namespace",
                ));
            }
        }
    }
}

fn validate_join(
    named: &ProjectionIndexes<'_>,
    claims: &ProjectionIndexes<'_>,
    diagnostics: &mut Vec<BoundScalarDiagnostic>,
) {
    if named.retain_parameters {
        validate_duplicates(
            "named_projection",
            ScalarKind::Parameter,
            &named.parameters,
            diagnostics,
        );
    }
    if named.retain_connectors {
        validate_duplicates(
            "named_projection",
            ScalarKind::Connector,
            &named.connectors,
            diagnostics,
        );
    }
    if claims.retain_parameters {
        validate_duplicates(
            "source_claim_projection",
            ScalarKind::Parameter,
            &claims.parameters,
            diagnostics,
        );
    }
    if claims.retain_connectors {
        validate_duplicates(
            "source_claim_projection",
            ScalarKind::Connector,
            &claims.connectors,
            diagnostics,
        );
    }
    validate_cross_kind_collisions("named_projection", named, diagnostics);
    validate_cross_kind_collisions("source_claim_projection", claims, diagnostics);

    if named.retain_parameters
        && named.retain_connectors
        && claims.retain_parameters
        && claims.retain_connectors
    {
        validate_namespace_join(
            ScalarKind::Parameter,
            &named.parameters,
            &claims.parameters,
            &named.connectors,
            &claims.connectors,
            diagnostics,
        );
        validate_namespace_join(
            ScalarKind::Connector,
            &named.connectors,
            &claims.connectors,
            &named.parameters,
            &claims.parameters,
            diagnostics,
        );
    }
}

struct PreparedParameter<'a> {
    named: &'a NamedScalarParameterRow,
    claim: &'a ScalarParameterSourceClaim,
}

struct PreparedConnector<'a> {
    named: &'a NamedScalarConnectorRow,
    claim: &'a ScalarConnectorSourceClaim,
}

fn output_resource_error(location: &str, label: &str) -> BoundScalarError {
    BoundScalarError::new(vec![resource_diagnostic(
        location,
        &format!("{label} allocation failed"),
    )])
}

fn prepare_rows<'a>(
    named: &'a NamedScalarProjection,
    claims: &'a ScalarSourceClaimProjection,
) -> Result<(Vec<PreparedParameter<'a>>, Vec<PreparedConnector<'a>>), BoundScalarError> {
    let mut parameter_claims = HashMap::new();
    parameter_claims
        .try_reserve(claims.parameters.len())
        .map_err(|_| output_resource_error("$.parameters", "parameter source index"))?;
    let mut connector_claims = HashMap::new();
    connector_claims
        .try_reserve(claims.connectors.len())
        .map_err(|_| output_resource_error("$.connectors", "connector source index"))?;
    let mut diagnostics = Vec::new();
    for claim in &claims.parameters {
        if parameter_claims
            .insert(claim.scalar_name.as_str(), claim)
            .is_some()
        {
            diagnostics.push(diagnostic(
                "invalid_join_state",
                "parameter",
                &claim.parameter_id,
                "$.join",
                "validated parameter source claim is duplicated",
            ));
        }
    }
    for claim in &claims.connectors {
        if connector_claims
            .insert(claim.scalar_name.as_str(), claim)
            .is_some()
        {
            diagnostics.push(diagnostic(
                "invalid_join_state",
                "connector",
                &claim.connector_id,
                "$.join",
                "validated connector source claim is duplicated",
            ));
        }
    }

    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(named.parameters.len())
        .map_err(|_| output_resource_error("$.parameters", "parameter join vector"))?;
    let mut connectors = Vec::new();
    connectors
        .try_reserve_exact(named.connectors.len())
        .map_err(|_| output_resource_error("$.connectors", "connector join vector"))?;

    for row in &named.parameters {
        if let Some(claim) = parameter_claims.get(row.scalar_name.as_str()).copied() {
            parameters.push(PreparedParameter { named: row, claim });
        } else {
            diagnostics.push(diagnostic(
                "invalid_join_state",
                "parameter",
                &row.parameter_id,
                "$.join",
                "validated parameter source claim is unavailable",
            ));
        }
    }
    for row in &named.connectors {
        if let Some(claim) = connector_claims.get(row.scalar_name.as_str()).copied() {
            connectors.push(PreparedConnector { named: row, claim });
        } else {
            diagnostics.push(diagnostic(
                "invalid_join_state",
                "connector",
                &row.connector_id,
                "$.join",
                "validated connector source claim is unavailable",
            ));
        }
    }
    if diagnostics.is_empty() {
        Ok((parameters, connectors))
    } else {
        Err(BoundScalarError::new(diagnostics))
    }
}

fn clone_text(value: &str) -> Result<String, ()> {
    let mut output = String::new();
    output.try_reserve_exact(value.len()).map_err(|_| ())?;
    output.push_str(value);
    Ok(output)
}

fn clone_optional_text(value: &Option<String>) -> Result<Option<String>, ()> {
    value.as_deref().map(clone_text).transpose()
}

fn clone_coordinates(coordinates: &[ScalarCoordinate]) -> Result<Vec<ScalarCoordinate>, ()> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(coordinates.len())
        .map_err(|_| ())?;
    for coordinate in coordinates {
        output.push(ScalarCoordinate {
            dimension_id: clone_text(&coordinate.dimension_id)?,
            member_id: clone_text(&coordinate.member_id)?,
            ordinal: coordinate.ordinal,
        });
    }
    Ok(output)
}

fn clone_abi_type(abi_type: &ScalarAbiType) -> Result<ScalarAbiType, ()> {
    match abi_type {
        ScalarAbiType::Primitive(primitive) => Ok(ScalarAbiType::Primitive(*primitive)),
        ScalarAbiType::Alias {
            type_id,
            primitive,
            quantity,
            unit,
            display_unit,
        } => Ok(ScalarAbiType::Alias {
            type_id: clone_text(type_id)?,
            primitive: *primitive,
            quantity: clone_optional_text(quantity)?,
            unit: clone_optional_text(unit)?,
            display_unit: clone_optional_text(display_unit)?,
        }),
        ScalarAbiType::Enum {
            canonical_class_path,
        } => Ok(ScalarAbiType::Enum {
            canonical_class_path: clone_text(canonical_class_path)?,
        }),
    }
}

fn clone_abi_value(value: &ScalarAbiValue) -> ScalarAbiValue {
    match value {
        ScalarAbiValue::Boolean(value) => ScalarAbiValue::Boolean(*value),
        ScalarAbiValue::Integer(value) => ScalarAbiValue::Integer(value.clone()),
        ScalarAbiValue::Real(value) => ScalarAbiValue::Real(*value),
        ScalarAbiValue::Enum { ordinal } => ScalarAbiValue::Enum { ordinal: *ordinal },
    }
}

fn clone_locator(locator: &SourceFileLocator) -> Result<SourceFileLocator, ()> {
    Ok(SourceFileLocator {
        path: clone_text(&locator.path)?,
        git_blob_sha1: clone_text(&locator.git_blob_sha1)?,
    })
}

fn clone_source_claim(
    canonical_class_path: &str,
    source_member: &str,
    snapshot: SourceSnapshotRole,
    revision: &str,
    file: &SourceFileLocator,
) -> Result<BoundSourceClaim, ()> {
    Ok(BoundSourceClaim {
        canonical_class_path: clone_text(canonical_class_path)?,
        source_member: clone_text(source_member)?,
        snapshot,
        revision: clone_text(revision)?,
        file: clone_locator(file)?,
    })
}

/// Validates both projections and joins rows by scalar name within each namespace.
///
/// Named rows determine output order and ABI payload. Source rows contribute only
/// their caller source claims. The function performs no I/O and does not mutate
/// either input.
pub fn bind_scalar_source_claims(
    named_projection: &NamedScalarProjection,
    source_claim_projection: &ScalarSourceClaimProjection,
) -> Result<BoundScalarProjection, BoundScalarError> {
    let input_count = checked_total_count([
        named_projection.parameters.len(),
        named_projection.connectors.len(),
        source_claim_projection.parameters.len(),
        source_claim_projection.connectors.len(),
    ])
    .ok_or_else(|| {
        BoundScalarError::new(vec![resource_diagnostic(
            "$",
            "input row count overflows usize",
        )])
    })?;
    let mut diagnostics = Vec::new();
    diagnostics.try_reserve(input_count).map_err(|_| {
        BoundScalarError::new(vec![resource_diagnostic(
            "$",
            "diagnostic allocation failed",
        )])
    })?;

    if !named_projection.canonical_id.is_empty()
        && !source_claim_projection.canonical_id.is_empty()
        && named_projection.canonical_id != source_claim_projection.canonical_id
    {
        diagnostics.push(diagnostic(
            "canonical_id_mismatch",
            "projection",
            "$",
            "$.join.canonical_id",
            "projection canonical IDs differ",
        ));
    }
    if named_projection.revision > BigInt::from(0_u8)
        && source_claim_projection.revision > BigInt::from(0_u8)
        && named_projection.revision != source_claim_projection.revision
    {
        diagnostics.push(diagnostic(
            "revision_mismatch",
            "projection",
            "$",
            "$.join.revision",
            "projection revisions differ",
        ));
    }

    let named = validate_named_projection(named_projection, &mut diagnostics);
    let claims = validate_source_projection(source_claim_projection, &mut diagnostics);
    validate_join(&named, &claims, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(BoundScalarError::new(diagnostics));
    }

    let (prepared_parameters, prepared_connectors) =
        prepare_rows(named_projection, source_claim_projection)?;
    let canonical_id = clone_text(&named_projection.canonical_id)
        .map_err(|_| output_resource_error("$", "canonical ID"))?;
    let revision = named_projection.revision.clone();
    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(prepared_parameters.len())
        .map_err(|_| output_resource_error("$.parameters", "parameter output row vector"))?;
    let mut connectors = Vec::new();
    connectors
        .try_reserve_exact(prepared_connectors.len())
        .map_err(|_| output_resource_error("$.connectors", "connector output row vector"))?;

    for prepared in prepared_parameters {
        let row = prepared.named;
        let claim = prepared.claim;
        parameters.push(BoundScalarParameterRow {
            scalar_name: clone_text(&row.scalar_name)
                .map_err(|_| output_resource_error("$.parameters", "scalar name"))?,
            parameter_id: clone_text(&row.parameter_id)
                .map_err(|_| output_resource_error("$.parameters", "parameter ID"))?,
            coordinates: clone_coordinates(&row.coordinates)
                .map_err(|_| output_resource_error("$.parameters", "coordinate"))?,
            abi_type: clone_abi_type(&row.abi_type)
                .map_err(|_| output_resource_error("$.parameters", "ABI type"))?,
            source: row.source,
            value: clone_abi_value(&row.value),
            source_claim: clone_source_claim(
                &claim.canonical_class_path,
                &claim.source_member,
                claim.snapshot,
                &claim.revision,
                &claim.file,
            )
            .map_err(|_| output_resource_error("$.parameters", "source claim"))?,
        });
    }
    for prepared in prepared_connectors {
        let row = prepared.named;
        let claim = prepared.claim;
        connectors.push(BoundScalarConnectorRow {
            scalar_name: clone_text(&row.scalar_name)
                .map_err(|_| output_resource_error("$.connectors", "scalar name"))?,
            connector_id: clone_text(&row.connector_id)
                .map_err(|_| output_resource_error("$.connectors", "connector ID"))?,
            coordinates: clone_coordinates(&row.coordinates)
                .map_err(|_| output_resource_error("$.connectors", "coordinate"))?,
            abi_type: clone_abi_type(&row.abi_type)
                .map_err(|_| output_resource_error("$.connectors", "ABI type"))?,
            direction: row.direction,
            source_claim: clone_source_claim(
                &claim.canonical_class_path,
                &claim.source_member,
                claim.snapshot,
                &claim.revision,
                &claim.file,
            )
            .map_err(|_| output_resource_error("$.connectors", "source claim"))?,
        });
    }

    Ok(BoundScalarProjection {
        canonical_id,
        revision,
        parameters,
        connectors,
    })
}

#[cfg(test)]
mod tests;
