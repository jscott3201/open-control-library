//! In-memory direct declaration checks for owner source claims.
//!
//! The checker compares supplied bytes to claimed legacy Git blob IDs, parses
//! each required document once, and checks direct class and component syntax.
//! SHA-1 is used only for Git interoperability; this boundary does not claim
//! collision resistance, inventory or tree membership, inheritance or type
//! resolution, or persisted evidence. Rumoca runs in-process and provides no
//! hard time or stack isolation here.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

use num_bigint::BigInt;
use rumoca_core::Variability;
use rumoca_ir_ast::StoredDefinition;
use sha1::{Digest, Sha1};

use crate::declaration_requirements::DeclarationRequirementProjection;
use crate::declarations::{direct_class, direct_component, expect_public_component};
use crate::scalar_source_claims::{
    SourceFileLocator, SourceSnapshotRole, is_class_path, is_modelica_identifier, is_revision,
    is_sha1, safe_source_path,
};

/// Bytes supplied for one exact source identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationSourceDocument {
    pub snapshot: SourceSnapshotRole,
    pub revision: String,
    pub file: SourceFileLocator,
    pub bytes: Vec<u8>,
}

/// Caller-selected bounds applied before hashing and after parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclarationSyntaxLimits {
    pub max_documents: usize,
    pub max_requirements: usize,
    pub max_source_bytes: usize,
    pub max_total_source_bytes: usize,
    pub max_direct_members: usize,
}

/// One sortable refusal from declaration syntax checking.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclarationSyntaxDiagnostic {
    pub code: String,
    pub owner_kind: String,
    pub owner_id: String,
    pub location: String,
    pub message: String,
}

impl fmt::Display for DeclarationSyntaxDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} {}: {}: {}",
            self.code, self.owner_kind, self.owner_id, self.location, self.message
        )
    }
}

/// Atomic syntax-check failure with diagnostics sorted by every public field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationSyntaxError {
    pub diagnostics: Vec<DeclarationSyntaxDiagnostic>,
}

impl DeclarationSyntaxError {
    fn new(mut diagnostics: Vec<DeclarationSyntaxDiagnostic>) -> Self {
        diagnostics.sort_unstable();
        Self { diagnostics }
    }
}

impl fmt::Display for DeclarationSyntaxError {
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

impl std::error::Error for DeclarationSyntaxError {}

struct DiagnosticBuffer {
    diagnostics: Vec<DeclarationSyntaxDiagnostic>,
    allocation_failed: bool,
}

impl DiagnosticBuffer {
    fn with_capacity(count: usize, location: &str) -> Result<Self, DeclarationSyntaxError> {
        let mut diagnostics = Vec::new();
        diagnostics
            .try_reserve(count)
            .map_err(|_| resource_error(location, "diagnostic vector allocation failed"))?;
        Ok(Self {
            diagnostics,
            allocation_failed: false,
        })
    }

    fn push(&mut self, diagnostic: DeclarationSyntaxDiagnostic) {
        if self.allocation_failed {
            return;
        }
        if self.diagnostics.try_reserve(1).is_err() {
            self.allocation_failed = true;
            return;
        }
        self.diagnostics.push(diagnostic);
    }

    fn finish(
        self,
        location: &str,
    ) -> Result<Vec<DeclarationSyntaxDiagnostic>, DeclarationSyntaxError> {
        if self.allocation_failed {
            Err(resource_error(
                location,
                "diagnostic vector allocation failed",
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
) -> DeclarationSyntaxDiagnostic {
    DeclarationSyntaxDiagnostic {
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

fn resource_diagnostic(location: &str, message: impl Into<String>) -> DeclarationSyntaxDiagnostic {
    diagnostic("resource_limit", "checker", "$", location, message)
}

fn resource_error(location: &str, message: &str) -> DeclarationSyntaxError {
    DeclarationSyntaxError::new(vec![resource_diagnostic(location, message)])
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
) -> Result<(), DeclarationSyntaxError> {
    map.try_reserve(count)
        .map_err(|_| resource_error(location, &format!("{label} allocation failed")))
}

fn reserve_set<T: Eq + Hash>(
    set: &mut HashSet<T>,
    count: usize,
    location: &str,
    label: &str,
) -> Result<(), DeclarationSyntaxError> {
    set.try_reserve(count)
        .map_err(|_| resource_error(location, &format!("{label} allocation failed")))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SourceIdentity<'a> {
    snapshot: SourceSnapshotRole,
    revision: &'a str,
    path: &'a str,
    blob: &'a str,
}

impl SourceIdentity<'_> {
    fn location(self) -> String {
        format!(
            "$.documents[{}:{}:{}:{}]",
            snapshot_name(self.snapshot),
            self.revision,
            self.path,
            self.blob
        )
    }
}

fn snapshot_name(snapshot: SourceSnapshotRole) -> &'static str {
    match snapshot {
        SourceSnapshotRole::Release => "release",
        SourceSnapshotRole::Development => "development",
    }
}

#[derive(Clone, Copy)]
enum OwnerKind {
    Parameter,
    Connector,
}

impl OwnerKind {
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

    fn prefix(self) -> &'static str {
        match self {
            Self::Parameter => "p_",
            Self::Connector => "c_",
        }
    }
}

#[derive(Clone, Copy)]
struct RequirementRef<'a> {
    kind: OwnerKind,
    owner_id: &'a str,
    canonical_class_path: &'a str,
    source_member: &'a str,
    scalar_names: &'a [String],
    identity: SourceIdentity<'a>,
}

impl RequirementRef<'_> {
    fn location(self) -> String {
        format!(
            "$.requirements.{}[{}]",
            self.kind.plural(),
            if self.owner_id.is_empty() {
                "$"
            } else {
                self.owner_id
            }
        )
    }
}

fn requirements_iter(
    requirements: &DeclarationRequirementProjection,
) -> impl Iterator<Item = RequirementRef<'_>> {
    requirements
        .parameters
        .iter()
        .map(|requirement| RequirementRef {
            kind: OwnerKind::Parameter,
            owner_id: &requirement.parameter_id,
            canonical_class_path: &requirement.canonical_class_path,
            source_member: &requirement.source_member,
            scalar_names: &requirement.scalar_names,
            identity: SourceIdentity {
                snapshot: requirement.snapshot,
                revision: &requirement.revision,
                path: &requirement.file.path,
                blob: &requirement.file.git_blob_sha1,
            },
        })
        .chain(
            requirements
                .connectors
                .iter()
                .map(|requirement| RequirementRef {
                    kind: OwnerKind::Connector,
                    owner_id: &requirement.connector_id,
                    canonical_class_path: &requirement.canonical_class_path,
                    source_member: &requirement.source_member,
                    scalar_names: &requirement.scalar_names,
                    identity: SourceIdentity {
                        snapshot: requirement.snapshot,
                        revision: &requirement.revision,
                        path: &requirement.file.path,
                        blob: &requirement.file.git_blob_sha1,
                    },
                }),
        )
}

fn document_identity(document: &DeclarationSourceDocument) -> SourceIdentity<'_> {
    SourceIdentity {
        snapshot: document.snapshot,
        revision: &document.revision,
        path: &document.file.path,
        blob: &document.file.git_blob_sha1,
    }
}

fn preflight_inputs(
    requirements: &DeclarationRequirementProjection,
    documents: &[DeclarationSourceDocument],
    limits: DeclarationSyntaxLimits,
) -> Result<usize, DeclarationSyntaxError> {
    let capacity = checked_total_count([documents.len(), 3])
        .ok_or_else(|| resource_error("$.limits", "preflight diagnostic count overflows usize"))?;
    let mut diagnostics = DiagnosticBuffer::with_capacity(capacity, "$.limits")?;

    if documents.len() > limits.max_documents {
        diagnostics.push(resource_diagnostic(
            "$.documents",
            format!(
                "document count {} exceeds max_documents {}",
                documents.len(),
                limits.max_documents
            ),
        ));
    }

    let requirement_count =
        checked_total_count([requirements.parameters.len(), requirements.connectors.len()]);
    match requirement_count {
        Some(count) if count > limits.max_requirements => diagnostics.push(resource_diagnostic(
            "$.requirements",
            format!(
                "requirement count {count} exceeds max_requirements {}",
                limits.max_requirements
            ),
        )),
        Some(_) => {}
        None => diagnostics.push(resource_diagnostic(
            "$.requirements",
            "requirement count overflows usize",
        )),
    }

    for document in documents {
        if document.bytes.len() > limits.max_source_bytes {
            diagnostics.push(resource_diagnostic(
                &document_identity(document).location(),
                format!(
                    "source byte count {} exceeds max_source_bytes {}",
                    document.bytes.len(),
                    limits.max_source_bytes
                ),
            ));
        }
    }
    match checked_total_count(documents.iter().map(|document| document.bytes.len())) {
        Some(total) if total > limits.max_total_source_bytes => {
            diagnostics.push(resource_diagnostic(
                "$.documents",
                format!(
                    "total source byte count {total} exceeds max_total_source_bytes {}",
                    limits.max_total_source_bytes
                ),
            ));
        }
        Some(_) => {}
        None => diagnostics.push(resource_diagnostic(
            "$.documents",
            "total source byte count overflows usize",
        )),
    }

    let diagnostics = diagnostics.finish("$.limits")?;
    if diagnostics.is_empty() {
        Ok(requirement_count.expect("absence of overflow diagnostic proves a count"))
    } else {
        Err(DeclarationSyntaxError::new(diagnostics))
    }
}

fn is_canonical_scalar_name(name: &str, prefix: &str) -> bool {
    let Some(suffix) = name.strip_prefix(prefix) else {
        return false;
    };
    let mut segment_count = 0_usize;
    for segment in suffix.split('_') {
        if segment.is_empty()
            || segment.len() % 2 != 0
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return false;
        }
        segment_count += 1;
    }
    segment_count % 2 == 1
}

fn validate_source_path(
    path: &str,
    owner_kind: &str,
    owner_id: &str,
    location: &str,
    diagnostics: &mut DiagnosticBuffer,
) -> bool {
    match safe_source_path(path) {
        Ok(()) if path.ends_with(".mo") => true,
        Ok(()) => {
            diagnostics.push(diagnostic(
                "invalid_source_path",
                owner_kind,
                owner_id,
                location,
                "source file path must end in `.mo`",
            ));
            false
        }
        Err(problem) => {
            diagnostics.push(diagnostic(
                "invalid_source_path",
                owner_kind,
                owner_id,
                location,
                problem,
            ));
            false
        }
    }
}

fn validate_requirement_source(
    requirement: RequirementRef<'_>,
    location: &str,
    diagnostics: &mut DiagnosticBuffer,
) -> bool {
    let owner_kind = requirement.kind.as_str();
    let owner_id = requirement.owner_id;
    if !is_class_path(requirement.canonical_class_path) {
        diagnostics.push(diagnostic(
            "invalid_source_class",
            owner_kind,
            owner_id,
            &format!("{location}.canonical_class_path"),
            "canonical_class_path must be a bounded class below the G36 package",
        ));
    }
    if !is_modelica_identifier(requirement.source_member) {
        diagnostics.push(diagnostic(
            "invalid_source_member",
            owner_kind,
            owner_id,
            &format!("{location}.source_member"),
            "source_member must be a bounded ASCII Modelica identifier",
        ));
    }
    let revision_valid = is_revision(requirement.identity.revision);
    if !revision_valid {
        diagnostics.push(diagnostic(
            "invalid_source_revision",
            owner_kind,
            owner_id,
            &format!("{location}.revision"),
            "source revision must be 40 lowercase hexadecimal characters",
        ));
    }
    let path_valid = validate_source_path(
        requirement.identity.path,
        owner_kind,
        owner_id,
        &format!("{location}.file.path"),
        diagnostics,
    );
    let blob_valid = is_sha1(requirement.identity.blob);
    if !blob_valid {
        diagnostics.push(diagnostic(
            "invalid_source_blob",
            owner_kind,
            owner_id,
            &format!("{location}.file.git_blob_sha1"),
            "git_blob_sha1 must match sha1:<40 lowercase hex>",
        ));
    }
    revision_valid && path_valid && blob_valid
}

struct CountGroup<'a> {
    count: usize,
    diagnostic_owner: &'a str,
}

struct SourceGroup<'a> {
    first_owner: &'a str,
    diagnostic_owner: &'a str,
    multiple_owners: bool,
}

struct NamespaceIndex<'a> {
    scalar_names: HashMap<&'a str, CountGroup<'a>>,
}

fn increment_group<'a>(
    group: &mut CountGroup<'a>,
    owner_id: &'a str,
    location: &str,
    label: &str,
    diagnostics: &mut DiagnosticBuffer,
) {
    match group.count.checked_add(1) {
        Some(count) => group.count = count,
        None => diagnostics.push(resource_diagnostic(
            location,
            format!("{label} count overflows usize"),
        )),
    }
    if owner_id < group.diagnostic_owner {
        group.diagnostic_owner = owner_id;
    }
}

fn validate_namespace<'a>(
    rows: impl Iterator<Item = RequirementRef<'a>>,
    kind: OwnerKind,
    row_count: usize,
    scalar_count: usize,
    required_identities: &mut HashSet<SourceIdentity<'a>>,
    diagnostics: &mut DiagnosticBuffer,
) -> Result<NamespaceIndex<'a>, DeclarationSyntaxError> {
    let collection_location = format!("$.requirements.{}", kind.plural());
    let mut owner_groups = HashMap::new();
    reserve_map(
        &mut owner_groups,
        row_count,
        &collection_location,
        "owner index",
    )?;
    let mut scalar_groups = HashMap::new();
    reserve_map(
        &mut scalar_groups,
        scalar_count,
        &collection_location,
        "scalar name index",
    )?;
    let mut source_groups = HashMap::new();
    reserve_map(
        &mut source_groups,
        row_count,
        &collection_location,
        "source key index",
    )?;

    for requirement in rows {
        let location = requirement.location();
        let owner_id = requirement.owner_id;
        let owner_kind = kind.as_str();
        if owner_id.is_empty() {
            diagnostics.push(diagnostic(
                "invalid_owner_id",
                owner_kind,
                owner_id,
                &format!("{location}.{}_id", owner_kind),
                format!("{owner_kind}_id must not be empty"),
            ));
        } else {
            match owner_groups.entry(owner_id) {
                Entry::Vacant(entry) => {
                    entry.insert(1_usize);
                }
                Entry::Occupied(mut entry) => match entry.get().checked_add(1) {
                    Some(count) => *entry.get_mut() = count,
                    None => diagnostics.push(resource_diagnostic(
                        &collection_location,
                        "owner count overflows usize",
                    )),
                },
            }
        }

        if requirement.scalar_names.is_empty() {
            diagnostics.push(diagnostic(
                "invalid_scalar_name",
                owner_kind,
                owner_id,
                &format!("{location}.scalar_names"),
                "every requirement must contain at least one scalar name",
            ));
        }
        for scalar_name in requirement.scalar_names {
            if !scalar_name.starts_with(kind.prefix()) {
                diagnostics.push(diagnostic(
                    "scalar_name_namespace",
                    owner_kind,
                    owner_id,
                    &format!("{location}.scalar_names"),
                    format!(
                        "{owner_kind} scalar names must start with `{}`",
                        kind.prefix()
                    ),
                ));
            } else if !is_canonical_scalar_name(scalar_name, kind.prefix()) {
                diagnostics.push(diagnostic(
                    "invalid_scalar_name",
                    owner_kind,
                    owner_id,
                    &format!("{location}.scalar_names"),
                    "scalar name must contain canonical lowercase hexadecimal segments",
                ));
            }
            match scalar_groups.entry(scalar_name.as_str()) {
                Entry::Vacant(entry) => {
                    entry.insert(CountGroup {
                        count: 1,
                        diagnostic_owner: owner_id,
                    });
                }
                Entry::Occupied(mut entry) => increment_group(
                    entry.get_mut(),
                    owner_id,
                    &collection_location,
                    "scalar name",
                    diagnostics,
                ),
            }
        }

        let identity_valid = validate_requirement_source(requirement, &location, diagnostics);
        if identity_valid {
            required_identities.insert(requirement.identity);
        }
        if !owner_id.is_empty()
            && is_class_path(requirement.canonical_class_path)
            && is_modelica_identifier(requirement.source_member)
        {
            let key = (requirement.canonical_class_path, requirement.source_member);
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

    for (owner_id, count) in owner_groups {
        if count > 1 {
            diagnostics.push(diagnostic(
                "duplicate_owner",
                kind.as_str(),
                owner_id,
                &collection_location,
                format!("owner ID occurs {count} times"),
            ));
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

    Ok(NamespaceIndex {
        scalar_names: scalar_groups,
    })
}

fn validate_document(document: &DeclarationSourceDocument, diagnostics: &mut DiagnosticBuffer) {
    let identity = document_identity(document);
    let location = identity.location();
    if !is_revision(identity.revision) {
        diagnostics.push(diagnostic(
            "invalid_source_revision",
            "document",
            "$",
            &format!("{location}.revision"),
            "source revision must be 40 lowercase hexadecimal characters",
        ));
    }
    validate_source_path(
        identity.path,
        "document",
        "$",
        &format!("{location}.file.path"),
        diagnostics,
    );
    if !is_sha1(identity.blob) {
        diagnostics.push(diagnostic(
            "invalid_source_blob",
            "document",
            "$",
            &format!("{location}.file.git_blob_sha1"),
            "git_blob_sha1 must match sha1:<40 lowercase hex>",
        ));
    }
}

struct InputIndexes<'a> {
    documents: HashMap<SourceIdentity<'a>, &'a DeclarationSourceDocument>,
}

fn validate_and_index_inputs<'a>(
    requirements: &'a DeclarationRequirementProjection,
    documents: &'a [DeclarationSourceDocument],
    requirement_count: usize,
) -> Result<InputIndexes<'a>, DeclarationSyntaxError> {
    let parameter_scalar_count = checked_total_count(
        requirements
            .parameters
            .iter()
            .map(|requirement| requirement.scalar_names.len()),
    )
    .ok_or_else(|| {
        resource_error(
            "$.requirements.parameters",
            "parameter scalar name count overflows usize",
        )
    })?;
    let connector_scalar_count = checked_total_count(
        requirements
            .connectors
            .iter()
            .map(|requirement| requirement.scalar_names.len()),
    )
    .ok_or_else(|| {
        resource_error(
            "$.requirements.connectors",
            "connector scalar name count overflows usize",
        )
    })?;
    let diagnostic_capacity = checked_total_count([requirement_count, documents.len(), 2])
        .ok_or_else(|| resource_error("$", "validation diagnostic count overflows usize"))?;
    let mut diagnostics = DiagnosticBuffer::with_capacity(diagnostic_capacity, "$")?;

    if requirements.canonical_id.is_empty() {
        diagnostics.push(diagnostic(
            "invalid_metadata",
            "projection",
            "$",
            "$.requirements.canonical_id",
            "canonical_id must not be empty",
        ));
    }
    if requirements.revision <= BigInt::from(0_u8) {
        diagnostics.push(diagnostic(
            "invalid_metadata",
            "projection",
            "$",
            "$.requirements.revision",
            "revision must be positive",
        ));
    }

    let mut required_identities = HashSet::new();
    reserve_set(
        &mut required_identities,
        requirement_count,
        "$.requirements",
        "required source identity index",
    )?;
    let parameter_index = validate_namespace(
        requirements
            .parameters
            .iter()
            .map(|requirement| RequirementRef {
                kind: OwnerKind::Parameter,
                owner_id: &requirement.parameter_id,
                canonical_class_path: &requirement.canonical_class_path,
                source_member: &requirement.source_member,
                scalar_names: &requirement.scalar_names,
                identity: SourceIdentity {
                    snapshot: requirement.snapshot,
                    revision: &requirement.revision,
                    path: &requirement.file.path,
                    blob: &requirement.file.git_blob_sha1,
                },
            }),
        OwnerKind::Parameter,
        requirements.parameters.len(),
        parameter_scalar_count,
        &mut required_identities,
        &mut diagnostics,
    )?;
    let connector_index = validate_namespace(
        requirements
            .connectors
            .iter()
            .map(|requirement| RequirementRef {
                kind: OwnerKind::Connector,
                owner_id: &requirement.connector_id,
                canonical_class_path: &requirement.canonical_class_path,
                source_member: &requirement.source_member,
                scalar_names: &requirement.scalar_names,
                identity: SourceIdentity {
                    snapshot: requirement.snapshot,
                    revision: &requirement.revision,
                    path: &requirement.file.path,
                    blob: &requirement.file.git_blob_sha1,
                },
            }),
        OwnerKind::Connector,
        requirements.connectors.len(),
        connector_scalar_count,
        &mut required_identities,
        &mut diagnostics,
    )?;
    for scalar_name in parameter_index.scalar_names.keys() {
        if connector_index.scalar_names.contains_key(scalar_name) {
            diagnostics.push(diagnostic(
                "cross_kind_collision",
                "projection",
                "$",
                "$.requirements",
                "one scalar name occurs in both namespaces",
            ));
        }
    }

    let mut document_index = HashMap::new();
    reserve_map(
        &mut document_index,
        documents.len(),
        "$.documents",
        "source document index",
    )?;
    for document in documents {
        validate_document(document, &mut diagnostics);
        let identity = document_identity(document);
        match document_index.entry(identity) {
            Entry::Vacant(entry) => {
                entry.insert(document);
            }
            Entry::Occupied(_) => diagnostics.push(diagnostic(
                "duplicate_source_document",
                "document",
                "$",
                &identity.location(),
                "source identity occurs more than once",
            )),
        }
    }

    for identity in &required_identities {
        if !document_index.contains_key(identity) {
            diagnostics.push(diagnostic(
                "missing_source_document",
                "document",
                "$",
                &identity.location(),
                "required source identity has no document",
            ));
        }
    }
    for identity in document_index.keys() {
        if !required_identities.contains(identity) {
            diagnostics.push(diagnostic(
                "unused_source_document",
                "document",
                "$",
                &identity.location(),
                "source document is not required",
            ));
        }
    }

    let diagnostics = diagnostics.finish("$")?;
    if diagnostics.is_empty() {
        Ok(InputIndexes {
            documents: document_index,
        })
    } else {
        Err(DeclarationSyntaxError::new(diagnostics))
    }
}

fn git_blob_sha1(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(b"blob ");
    hasher.update(bytes.len().to_string().as_bytes());
    hasher.update([0_u8]);
    hasher.update(bytes);
    format!("sha1:{:x}", hasher.finalize())
}

fn parse_documents<'a, F, E>(
    indexes: &InputIndexes<'a>,
    parser: &mut F,
    diagnostics: &mut DiagnosticBuffer,
) -> Result<HashMap<SourceIdentity<'a>, StoredDefinition>, DeclarationSyntaxError>
where
    F: FnMut(&str, &str) -> Result<StoredDefinition, E>,
{
    let mut parsed_documents = HashMap::new();
    reserve_map(
        &mut parsed_documents,
        indexes.documents.len(),
        "$.documents",
        "parsed document index",
    )?;
    for (identity, document) in &indexes.documents {
        let location = identity.location();
        let actual_blob = git_blob_sha1(&document.bytes);
        if actual_blob != identity.blob {
            diagnostics.push(diagnostic(
                "source_blob_mismatch",
                "document",
                "$",
                &location,
                format!("supplied bytes hash to `{actual_blob}`, not the claimed blob"),
            ));
            continue;
        }
        let source = match std::str::from_utf8(&document.bytes) {
            Ok(source) => source,
            Err(_) => {
                diagnostics.push(diagnostic(
                    "source_not_utf8",
                    "document",
                    "$",
                    &location,
                    "source bytes are not UTF-8",
                ));
                continue;
            }
        };
        match parser(source, identity.path) {
            Ok(parsed) => {
                parsed_documents.insert(*identity, parsed);
            }
            Err(_) => diagnostics.push(diagnostic(
                "modelica_parse_failed",
                "document",
                "$",
                &location,
                "source did not parse as Modelica",
            )),
        }
    }
    Ok(parsed_documents)
}

fn check_requirements(
    requirements: &DeclarationRequirementProjection,
    parsed_documents: &HashMap<SourceIdentity<'_>, StoredDefinition>,
    limits: DeclarationSyntaxLimits,
    diagnostics: &mut DiagnosticBuffer,
) {
    for requirement in requirements_iter(requirements) {
        let Some(parsed) = parsed_documents.get(&requirement.identity) else {
            continue;
        };
        let location = requirement.location();
        let (expected_within, expected_class) = requirement
            .canonical_class_path
            .rsplit_once('.')
            .expect("validated class paths contain a package and class name");
        let (class, canonical) = match direct_class(
            parsed,
            requirement.identity.path,
            expected_within,
            expected_class,
        ) {
            Ok(found) => found,
            Err(_) => {
                diagnostics.push(diagnostic(
                    "invalid_direct_class",
                    requirement.kind.as_str(),
                    requirement.owner_id,
                    &format!("{location}.canonical_class_path"),
                    format!(
                        "source must contain exactly the direct class `{}`",
                        requirement.canonical_class_path
                    ),
                ));
                continue;
            }
        };
        if class.components.len() > limits.max_direct_members {
            diagnostics.push(diagnostic(
                "resource_limit",
                requirement.kind.as_str(),
                requirement.owner_id,
                &format!("{location}.canonical_class_path"),
                format!(
                    "direct member count {} exceeds max_direct_members {}",
                    class.components.len(),
                    limits.max_direct_members
                ),
            ));
            continue;
        }
        let component = match direct_component(
            class,
            requirement.identity.path,
            &canonical,
            requirement.source_member,
        ) {
            Ok(component) => component,
            Err(_) => {
                diagnostics.push(diagnostic(
                    "missing_direct_member",
                    requirement.kind.as_str(),
                    requirement.owner_id,
                    &format!("{location}.source_member"),
                    format!(
                        "`{}.{}` must be a direct component",
                        requirement.canonical_class_path, requirement.source_member
                    ),
                ));
                continue;
            }
        };
        if expect_public_component(component, requirement.identity.path, &canonical).is_err() {
            diagnostics.push(diagnostic(
                "protected_member",
                requirement.kind.as_str(),
                requirement.owner_id,
                &format!("{location}.source_member"),
                format!(
                    "`{}.{}` must be public",
                    requirement.canonical_class_path, requirement.source_member
                ),
            ));
        }
        if matches!(requirement.kind, OwnerKind::Parameter)
            && !matches!(component.variability, Variability::Parameter(_))
        {
            diagnostics.push(diagnostic(
                "parameter_variability",
                requirement.kind.as_str(),
                requirement.owner_id,
                &format!("{location}.source_member"),
                format!(
                    "`{}.{}` must have parameter variability",
                    requirement.canonical_class_path, requirement.source_member
                ),
            ));
        }
    }
}

fn check_owner_declaration_syntax_with_parser<F, E>(
    requirements: &DeclarationRequirementProjection,
    documents: &[DeclarationSourceDocument],
    limits: DeclarationSyntaxLimits,
    mut parser: F,
) -> Result<(), DeclarationSyntaxError>
where
    F: FnMut(&str, &str) -> Result<StoredDefinition, E>,
{
    let requirement_count = preflight_inputs(requirements, documents, limits)?;
    let indexes = validate_and_index_inputs(requirements, documents, requirement_count)?;
    let capacity = checked_total_count([requirement_count, documents.len()])
        .ok_or_else(|| resource_error("$", "syntax diagnostic count overflows usize"))?;
    let mut diagnostics = DiagnosticBuffer::with_capacity(capacity, "$")?;
    let parsed_documents = parse_documents(&indexes, &mut parser, &mut diagnostics)?;
    check_requirements(requirements, &parsed_documents, limits, &mut diagnostics);
    let diagnostics = diagnostics.finish("$")?;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(DeclarationSyntaxError::new(diagnostics))
    }
}

/// Checks blob-matched source bytes for direct owner declaration syntax.
///
/// Limits are caller policy. Source-byte limits apply before hashing or parsing;
/// `max_direct_members` applies to parsed direct components before owner member
/// checks. The parser runs in-process without hard time or stack isolation.
pub fn check_owner_declaration_syntax(
    requirements: &DeclarationRequirementProjection,
    documents: &[DeclarationSourceDocument],
    limits: DeclarationSyntaxLimits,
) -> Result<(), DeclarationSyntaxError> {
    check_owner_declaration_syntax_with_parser(
        requirements,
        documents,
        limits,
        rumoca_phase_parse::parse_to_ast,
    )
}

#[cfg(test)]
mod tests;
