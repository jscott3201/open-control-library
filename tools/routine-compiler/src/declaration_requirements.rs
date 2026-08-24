//! Owner-level declaration requirements derived from scalar source claims.
//!
//! Source classes, members, revisions, paths, and blobs remain caller claims.
//! This stage does no inventory recheck, file access, source parsing,
//! declaration verification, or serialization. Although exposed by the crate,
//! these types are an internal in-memory handoff, not a public interchange or
//! persisted contract.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::hash::Hash;

use num_bigint::BigInt;

use crate::scalar_abi::ScalarCoordinate;
use crate::scalar_names::build_scalar_name;
use crate::scalar_source_claims::{
    ScalarConnectorSourceClaim, ScalarParameterSourceClaim, ScalarSourceClaimProjection,
    SourceFileLocator, SourceSnapshotRole, is_class_path, is_modelica_identifier, is_revision,
    is_sha1, safe_source_path,
};

/// Source claim and scalar names for one parameter declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterDeclarationRequirement {
    pub parameter_id: String,
    pub canonical_class_path: String,
    pub source_member: String,
    pub snapshot: SourceSnapshotRole,
    pub revision: String,
    pub file: SourceFileLocator,
    pub scalar_names: Vec<String>,
}

/// Source claim and scalar names for one connector declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectorDeclarationRequirement {
    pub connector_id: String,
    pub canonical_class_path: String,
    pub source_member: String,
    pub snapshot: SourceSnapshotRole,
    pub revision: String,
    pub file: SourceFileLocator,
    pub scalar_names: Vec<String>,
}

/// Detached owner requirements in first-occurrence order within each namespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationRequirementProjection {
    pub canonical_id: String,
    pub revision: BigInt,
    pub parameters: Vec<ParameterDeclarationRequirement>,
    pub connectors: Vec<ConnectorDeclarationRequirement>,
}

/// One sortable refusal from declaration-requirement projection.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclarationRequirementDiagnostic {
    pub code: String,
    pub owner_kind: String,
    pub owner_id: String,
    pub location: String,
    pub message: String,
}

impl fmt::Display for DeclarationRequirementDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} {}: {}: {}",
            self.code, self.owner_kind, self.owner_id, self.location, self.message
        )
    }
}

/// Atomic projection failure with diagnostics sorted by every public field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationRequirementError {
    pub diagnostics: Vec<DeclarationRequirementDiagnostic>,
}

impl DeclarationRequirementError {
    fn new(mut diagnostics: Vec<DeclarationRequirementDiagnostic>) -> Self {
        diagnostics.sort_unstable();
        Self { diagnostics }
    }
}

impl fmt::Display for DeclarationRequirementError {
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

impl std::error::Error for DeclarationRequirementError {}

struct DiagnosticBuffer {
    diagnostics: Vec<DeclarationRequirementDiagnostic>,
    allocation_failed: bool,
}

impl DiagnosticBuffer {
    fn with_capacity(count: usize) -> Result<Self, DeclarationRequirementError> {
        let mut diagnostics = Vec::new();
        diagnostics
            .try_reserve(count)
            .map_err(|_| output_resource_error("$.source_claim_projection", "diagnostic vector"))?;
        Ok(Self {
            diagnostics,
            allocation_failed: false,
        })
    }

    fn push(&mut self, diagnostic: DeclarationRequirementDiagnostic) {
        if self.allocation_failed {
            return;
        }
        if self.diagnostics.try_reserve(1).is_err() {
            self.allocation_failed = true;
            return;
        }
        self.diagnostics.push(diagnostic);
    }

    fn into_diagnostics(
        self,
    ) -> Result<Vec<DeclarationRequirementDiagnostic>, DeclarationRequirementError> {
        if self.allocation_failed {
            Err(output_resource_error(
                "$.source_claim_projection",
                "diagnostic vector",
            ))
        } else {
            Ok(self.diagnostics)
        }
    }
}

fn diagnostic(
    code: &str,
    owner_kind: &str,
    owner_id: &str,
    location: &str,
    message: impl Into<String>,
) -> DeclarationRequirementDiagnostic {
    DeclarationRequirementDiagnostic {
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

fn resource_diagnostic(location: &str, message: &str) -> DeclarationRequirementDiagnostic {
    diagnostic("resource_limit", "projection", "$", location, message)
}

fn resource_error(location: &str, message: &str) -> DeclarationRequirementError {
    DeclarationRequirementError::new(vec![resource_diagnostic(location, message)])
}

fn output_resource_error(location: &str, label: &str) -> DeclarationRequirementError {
    resource_error(location, &format!("{label} allocation failed"))
}

fn checked_total_count(lengths: impl IntoIterator<Item = usize>) -> Option<usize> {
    lengths
        .into_iter()
        .try_fold(0_usize, |total, length| total.checked_add(length))
}

fn checked_input_count(projection: &ScalarSourceClaimProjection) -> Option<usize> {
    let row_count =
        checked_total_count([projection.parameters.len(), projection.connectors.len()])?;
    let coordinate_count = projection
        .parameters
        .iter()
        .map(|row| row.coordinates.len())
        .chain(
            projection
                .connectors
                .iter()
                .map(|row| row.coordinates.len()),
        )
        .try_fold(0_usize, |total, length| total.checked_add(length))?;
    checked_total_count([2, row_count, coordinate_count])
}

fn reserve_map<K: Eq + Hash, V>(
    map: &mut HashMap<K, V>,
    count: usize,
    location: &str,
    label: &str,
) -> Result<(), DeclarationRequirementError> {
    map.try_reserve(count)
        .map_err(|_| output_resource_error(location, label))
}

fn reserve_vec<T>(
    values: &mut Vec<T>,
    count: usize,
    location: &str,
    label: &str,
) -> Result<(), DeclarationRequirementError> {
    values
        .try_reserve_exact(count)
        .map_err(|_| output_resource_error(location, label))
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

trait SourceClaimRow {
    fn scalar_name(&self) -> &str;
    fn owner_id(&self) -> &str;
    fn coordinates(&self) -> &[ScalarCoordinate];
    fn canonical_class_path(&self) -> &str;
    fn source_member(&self) -> &str;
    fn snapshot(&self) -> SourceSnapshotRole;
    fn source_revision(&self) -> &str;
    fn file(&self) -> &SourceFileLocator;
}

impl SourceClaimRow for ScalarParameterSourceClaim {
    fn scalar_name(&self) -> &str {
        &self.scalar_name
    }

    fn owner_id(&self) -> &str {
        &self.parameter_id
    }

    fn coordinates(&self) -> &[ScalarCoordinate] {
        &self.coordinates
    }

    fn canonical_class_path(&self) -> &str {
        &self.canonical_class_path
    }

    fn source_member(&self) -> &str {
        &self.source_member
    }

    fn snapshot(&self) -> SourceSnapshotRole {
        self.snapshot
    }

    fn source_revision(&self) -> &str {
        &self.revision
    }

    fn file(&self) -> &SourceFileLocator {
        &self.file
    }
}

impl SourceClaimRow for ScalarConnectorSourceClaim {
    fn scalar_name(&self) -> &str {
        &self.scalar_name
    }

    fn owner_id(&self) -> &str {
        &self.connector_id
    }

    fn coordinates(&self) -> &[ScalarCoordinate] {
        &self.coordinates
    }

    fn canonical_class_path(&self) -> &str {
        &self.canonical_class_path
    }

    fn source_member(&self) -> &str {
        &self.source_member
    }

    fn snapshot(&self) -> SourceSnapshotRole {
        self.snapshot
    }

    fn source_revision(&self) -> &str {
        &self.revision
    }

    fn file(&self) -> &SourceFileLocator {
        &self.file
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SourceIdentity<'a> {
    canonical_class_path: &'a str,
    source_member: &'a str,
    snapshot: SourceSnapshotRole,
    revision: &'a str,
    path: &'a str,
    blob: &'a str,
}

fn source_identity(row: &impl SourceClaimRow) -> SourceIdentity<'_> {
    SourceIdentity {
        canonical_class_path: row.canonical_class_path(),
        source_member: row.source_member(),
        snapshot: row.snapshot(),
        revision: row.source_revision(),
        path: &row.file().path,
        blob: &row.file().git_blob_sha1,
    }
}

struct ScalarGroup<'a> {
    count: usize,
    diagnostic_owner: &'a str,
}

struct SourceGroup<'a> {
    first_owner: &'a str,
    diagnostic_owner: &'a str,
    multiple_owners: bool,
}

struct OwnerGroup<'a, R> {
    first: &'a R,
    scalar_names: Vec<&'a str>,
    source_coherent: bool,
}

struct NamespaceValidation<'a, R> {
    scalar_groups: HashMap<&'a str, ScalarGroup<'a>>,
    owner_groups: Vec<OwnerGroup<'a, R>>,
}

fn validate_metadata(projection: &ScalarSourceClaimProjection, diagnostics: &mut DiagnosticBuffer) {
    if projection.canonical_id.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_metadata",
            "projection",
            "$",
            "$.source_claim_projection.canonical_id",
            "canonical_id must not be empty",
        ));
    }
    if projection.revision <= BigInt::from(0_u8) {
        diagnostics.push(diagnostic(
            "invalid_metadata",
            "projection",
            "$",
            "$.source_claim_projection.revision",
            "revision must be positive",
        ));
    }
}

fn validate_scalar_identity(
    row: &impl SourceClaimRow,
    kind: ScalarKind,
    location: &str,
    diagnostics: &mut DiagnosticBuffer,
) {
    let owner_id = row.owner_id();
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
    if row.scalar_name().is_empty() {
        diagnostics.push(diagnostic(
            "invalid_scalar_name",
            owner_kind,
            owner_id,
            &format!("{location}.scalar_name"),
            "scalar_name must not be empty",
        ));
    } else if !row.scalar_name().starts_with(kind.prefix()) {
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
    for (index, coordinate) in row.coordinates().iter().enumerate() {
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
        match build_scalar_name(kind.prefix(), owner_id, row.coordinates()) {
            Ok(expected) if expected != row.scalar_name() => diagnostics.push(diagnostic(
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

fn validate_source_payload(
    row: &impl SourceClaimRow,
    kind: ScalarKind,
    location: &str,
    diagnostics: &mut DiagnosticBuffer,
) {
    let owner_id = row.owner_id();
    let owner_kind = kind.as_str();
    if !is_class_path(row.canonical_class_path()) {
        diagnostics.push(diagnostic(
            "invalid_source_class",
            owner_kind,
            owner_id,
            &format!("{location}.canonical_class_path"),
            "canonical_class_path must be a bounded class below the G36 package",
        ));
    }
    if !is_modelica_identifier(row.source_member()) {
        diagnostics.push(diagnostic(
            "invalid_source_member",
            owner_kind,
            owner_id,
            &format!("{location}.source_member"),
            "source_member must be a bounded ASCII Modelica identifier",
        ));
    }
    if !is_revision(row.source_revision()) {
        diagnostics.push(diagnostic(
            "invalid_source_revision",
            owner_kind,
            owner_id,
            &format!("{location}.revision"),
            "source revision must be 40 lowercase hexadecimal characters",
        ));
    }
    match safe_source_path(&row.file().path) {
        Ok(()) if !row.file().path.ends_with(".mo") => diagnostics.push(diagnostic(
            "invalid_source_path",
            owner_kind,
            owner_id,
            &format!("{location}.file.path"),
            "source file path must end in `.mo`",
        )),
        Ok(()) => {}
        Err(problem) => diagnostics.push(diagnostic(
            "invalid_source_path",
            owner_kind,
            owner_id,
            &format!("{location}.file.path"),
            problem,
        )),
    }
    if !is_sha1(&row.file().git_blob_sha1) {
        diagnostics.push(diagnostic(
            "invalid_source_blob",
            owner_kind,
            owner_id,
            &format!("{location}.file.git_blob_sha1"),
            "git_blob_sha1 must match sha1:<40 lowercase hex>",
        ));
    }
}

fn index_scalar<'a>(
    scalar_groups: &mut HashMap<&'a str, ScalarGroup<'a>>,
    scalar_name: &'a str,
    owner_id: &'a str,
    location: &str,
    diagnostics: &mut DiagnosticBuffer,
) {
    if scalar_name.is_empty() {
        return;
    }
    match scalar_groups.entry(scalar_name) {
        Entry::Vacant(entry) => {
            entry.insert(ScalarGroup {
                count: 1,
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
            let current = if group.diagnostic_owner.is_empty() {
                "$"
            } else {
                group.diagnostic_owner
            };
            let candidate = if owner_id.is_empty() { "$" } else { owner_id };
            if candidate < current {
                group.diagnostic_owner = owner_id;
            }
        }
    }
}

fn validate_namespace<'a, R: SourceClaimRow>(
    rows: &'a [R],
    kind: ScalarKind,
    diagnostics: &mut DiagnosticBuffer,
) -> Result<NamespaceValidation<'a, R>, DeclarationRequirementError> {
    let collection_location = format!("$.source_claim_projection.{}", kind.plural());
    let mut scalar_groups = HashMap::new();
    reserve_map(
        &mut scalar_groups,
        rows.len(),
        &collection_location,
        "scalar name index",
    )?;
    let mut owner_indexes = HashMap::new();
    reserve_map(
        &mut owner_indexes,
        rows.len(),
        &collection_location,
        "owner index",
    )?;
    let mut source_groups = HashMap::new();
    reserve_map(
        &mut source_groups,
        rows.len(),
        &collection_location,
        "source key index",
    )?;
    let mut owner_groups = Vec::new();
    reserve_vec(
        &mut owner_groups,
        rows.len(),
        &collection_location,
        "owner group vector",
    )?;

    for (index, row) in rows.iter().enumerate() {
        let location = format!("{collection_location}[{index}]");
        validate_scalar_identity(row, kind, &location, diagnostics);
        validate_source_payload(row, kind, &location, diagnostics);
        index_scalar(
            &mut scalar_groups,
            row.scalar_name(),
            row.owner_id(),
            &collection_location,
            diagnostics,
        );

        let owner_id = row.owner_id();
        if !owner_id.is_empty() {
            if let Some(group_index) = owner_indexes.get(owner_id).copied() {
                let group: &mut OwnerGroup<'a, R> = &mut owner_groups[group_index];
                if source_identity(group.first) != source_identity(row) {
                    group.source_coherent = false;
                }
                group.scalar_names.try_reserve(1).map_err(|_| {
                    output_resource_error(&collection_location, "owner scalar name")
                })?;
                group.scalar_names.push(row.scalar_name());
            } else {
                let mut scalar_names = Vec::new();
                scalar_names.try_reserve_exact(1).map_err(|_| {
                    output_resource_error(&collection_location, "owner scalar name")
                })?;
                scalar_names.push(row.scalar_name());
                let group_index = owner_groups.len();
                owner_groups.push(OwnerGroup {
                    first: row,
                    scalar_names,
                    source_coherent: true,
                });
                owner_indexes.insert(owner_id, group_index);
            }
        }

        if !owner_id.is_empty()
            && is_class_path(row.canonical_class_path())
            && is_modelica_identifier(row.source_member())
        {
            let key = (row.canonical_class_path(), row.source_member());
            source_groups
                .entry(key)
                .and_modify(|group: &mut SourceGroup<'a>| {
                    if owner_id != group.first_owner {
                        group.multiple_owners = true;
                    }
                    if owner_id < group.diagnostic_owner {
                        group.diagnostic_owner = owner_id;
                    }
                })
                .or_insert(SourceGroup {
                    first_owner: owner_id,
                    diagnostic_owner: owner_id,
                    multiple_owners: false,
                });
        }
    }

    for group in scalar_groups.values() {
        if group.count > 1 {
            diagnostics.push(diagnostic(
                "duplicate_scalar_name",
                kind.as_str(),
                group.diagnostic_owner,
                &collection_location,
                format!("scalar name occurs {} times", group.count),
            ));
        }
    }
    for group in &owner_groups {
        if !group.source_coherent {
            diagnostics.push(diagnostic(
                "inconsistent_owner_source",
                kind.as_str(),
                group.first.owner_id(),
                &collection_location,
                "owner rows must carry one coherent source identity",
            ));
        }
    }
    for group in source_groups.values() {
        if group.multiple_owners {
            diagnostics.push(diagnostic(
                "duplicate_source_key",
                kind.as_str(),
                group.diagnostic_owner,
                &collection_location,
                "distinct owners claim the same class and source member",
            ));
        }
    }

    Ok(NamespaceValidation {
        scalar_groups,
        owner_groups,
    })
}

fn validate_cross_kind_collisions(
    parameters: &NamespaceValidation<'_, ScalarParameterSourceClaim>,
    connectors: &NamespaceValidation<'_, ScalarConnectorSourceClaim>,
    diagnostics: &mut DiagnosticBuffer,
) {
    for scalar_name in parameters.scalar_groups.keys() {
        if connectors.scalar_groups.contains_key(scalar_name) {
            diagnostics.push(diagnostic(
                "cross_kind_collision",
                "projection",
                "$",
                "$.source_claim_projection",
                "one scalar name occurs in both namespaces",
            ));
        }
    }
}

fn clone_text(value: &str) -> Result<String, ()> {
    let mut output = String::new();
    output.try_reserve_exact(value.len()).map_err(|_| ())?;
    output.push_str(value);
    Ok(output)
}

fn clone_locator(locator: &SourceFileLocator) -> Result<SourceFileLocator, ()> {
    Ok(SourceFileLocator {
        path: clone_text(&locator.path)?,
        git_blob_sha1: clone_text(&locator.git_blob_sha1)?,
    })
}

fn clone_scalar_names(values: &[&str]) -> Result<Vec<String>, ()> {
    let mut output = Vec::new();
    output.try_reserve_exact(values.len()).map_err(|_| ())?;
    for value in values {
        output.push(clone_text(value)?);
    }
    Ok(output)
}

fn materialize_parameters(
    groups: &[OwnerGroup<'_, ScalarParameterSourceClaim>],
) -> Result<Vec<ParameterDeclarationRequirement>, DeclarationRequirementError> {
    let mut requirements = Vec::new();
    reserve_vec(
        &mut requirements,
        groups.len(),
        "$.parameters",
        "parameter requirement vector",
    )?;
    for group in groups {
        let row = group.first;
        requirements.push(ParameterDeclarationRequirement {
            parameter_id: clone_text(row.owner_id())
                .map_err(|_| output_resource_error("$.parameters", "parameter ID"))?,
            canonical_class_path: clone_text(row.canonical_class_path())
                .map_err(|_| output_resource_error("$.parameters", "source class"))?,
            source_member: clone_text(row.source_member())
                .map_err(|_| output_resource_error("$.parameters", "source member"))?,
            snapshot: row.snapshot(),
            revision: clone_text(row.source_revision())
                .map_err(|_| output_resource_error("$.parameters", "source revision"))?,
            file: clone_locator(row.file())
                .map_err(|_| output_resource_error("$.parameters", "file locator"))?,
            scalar_names: clone_scalar_names(&group.scalar_names)
                .map_err(|_| output_resource_error("$.parameters", "scalar name vector"))?,
        });
    }
    Ok(requirements)
}

fn materialize_connectors(
    groups: &[OwnerGroup<'_, ScalarConnectorSourceClaim>],
) -> Result<Vec<ConnectorDeclarationRequirement>, DeclarationRequirementError> {
    let mut requirements = Vec::new();
    reserve_vec(
        &mut requirements,
        groups.len(),
        "$.connectors",
        "connector requirement vector",
    )?;
    for group in groups {
        let row = group.first;
        requirements.push(ConnectorDeclarationRequirement {
            connector_id: clone_text(row.owner_id())
                .map_err(|_| output_resource_error("$.connectors", "connector ID"))?,
            canonical_class_path: clone_text(row.canonical_class_path())
                .map_err(|_| output_resource_error("$.connectors", "source class"))?,
            source_member: clone_text(row.source_member())
                .map_err(|_| output_resource_error("$.connectors", "source member"))?,
            snapshot: row.snapshot(),
            revision: clone_text(row.source_revision())
                .map_err(|_| output_resource_error("$.connectors", "source revision"))?,
            file: clone_locator(row.file())
                .map_err(|_| output_resource_error("$.connectors", "file locator"))?,
            scalar_names: clone_scalar_names(&group.scalar_names)
                .map_err(|_| output_resource_error("$.connectors", "scalar name vector"))?,
        });
    }
    Ok(requirements)
}

/// Validates scalar source claims and collapses them into owner requirements.
///
/// Validation covers the complete forgeable input before any public requirement
/// object is built. Successful output preserves first-owner and scalar-row order
/// and remains detached from the input.
pub fn project_declaration_requirements(
    source_claim_projection: &ScalarSourceClaimProjection,
) -> Result<DeclarationRequirementProjection, DeclarationRequirementError> {
    let input_count = checked_input_count(source_claim_projection).ok_or_else(|| {
        resource_error(
            "$.source_claim_projection",
            "input component count overflows usize",
        )
    })?;
    let mut diagnostics = DiagnosticBuffer::with_capacity(input_count)?;

    validate_metadata(source_claim_projection, &mut diagnostics);
    let parameters = validate_namespace(
        &source_claim_projection.parameters,
        ScalarKind::Parameter,
        &mut diagnostics,
    )?;
    let connectors = validate_namespace(
        &source_claim_projection.connectors,
        ScalarKind::Connector,
        &mut diagnostics,
    )?;
    validate_cross_kind_collisions(&parameters, &connectors, &mut diagnostics);
    let diagnostics = diagnostics.into_diagnostics()?;
    if !diagnostics.is_empty() {
        return Err(DeclarationRequirementError::new(diagnostics));
    }

    let canonical_id = clone_text(&source_claim_projection.canonical_id)
        .map_err(|_| output_resource_error("$", "canonical ID"))?;
    let revision = source_claim_projection.revision.clone();
    let parameter_requirements = materialize_parameters(&parameters.owner_groups)?;
    let connector_requirements = materialize_connectors(&connectors.owner_groups)?;
    Ok(DeclarationRequirementProjection {
        canonical_id,
        revision,
        parameters: parameter_requirements,
        connectors: connector_requirements,
    })
}

#[cfg(test)]
mod tests;
