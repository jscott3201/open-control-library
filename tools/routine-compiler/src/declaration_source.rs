//! Capability-relative acquisition of declaration source bytes.
//!
//! Callers select and validate both roots. This adapter validates lexical
//! source paths, holds opened handles across metadata preflight and reading,
//! and enforces byte limits. A same-size concurrent content change can still
//! reach the downstream content checker.

use std::fmt;
use std::io::{self, Read};

use cap_std::fs::{Dir, File};

use crate::declaration_requirements::DeclarationRequirementProjection;
use crate::declaration_syntax::DeclarationSourceDocument;
use crate::scalar_source_claims::{SourceFileLocator, SourceSnapshotRole, safe_source_path};

const READ_BUFFER_BYTES: usize = 8 * 1024;

/// Borrowed release and development directory capabilities.
///
/// The constructor does not establish root provenance or snapshot identity.
/// Each role is always read from its corresponding directory.
#[derive(Clone, Copy)]
pub struct DeclarationSourceRoots<'a> {
    release: &'a Dir,
    development: &'a Dir,
}

impl<'a> DeclarationSourceRoots<'a> {
    /// Associates already-open directories with their snapshot roles.
    pub const fn new(release: &'a Dir, development: &'a Dir) -> Self {
        Self {
            release,
            development,
        }
    }

    fn for_snapshot(self, snapshot: SourceSnapshotRole) -> &'a Dir {
        match snapshot {
            SourceSnapshotRole::Release => self.release,
            SourceSnapshotRole::Development => self.development,
        }
    }
}

/// Inclusive acquisition bounds for unique source identities and source bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclarationSourceLimits {
    pub max_documents: usize,
    pub max_source_bytes: usize,
    pub max_total_source_bytes: usize,
}

/// One sortable acquisition refusal with a stable identity location.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclarationSourceDiagnostic {
    pub code: String,
    pub location: String,
    pub message: String,
}

impl fmt::Display for DeclarationSourceDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}: {}",
            self.code, self.location, self.message
        )
    }
}

/// Atomic acquisition failure with diagnostics sorted by every public field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationSourceError {
    pub diagnostics: Vec<DeclarationSourceDiagnostic>,
}

impl DeclarationSourceError {
    fn new(mut diagnostics: Vec<DeclarationSourceDiagnostic>) -> Self {
        diagnostics.sort_unstable();
        Self { diagnostics }
    }
}

impl fmt::Display for DeclarationSourceError {
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

impl std::error::Error for DeclarationSourceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SourceIdentity<'a> {
    snapshot: SourceSnapshotRole,
    revision: &'a str,
    path: &'a str,
    blob: &'a str,
}

impl SourceIdentity<'_> {
    fn location(self) -> String {
        format!(
            "$.sources.{}[revision={:?},path={:?},blob={:?}]",
            snapshot_name(self.snapshot),
            self.revision,
            self.path,
            self.blob
        )
    }
}

struct OpenedSource<'a> {
    identity: SourceIdentity<'a>,
    file: File,
    metadata_len: usize,
}

struct DiagnosticBuffer {
    diagnostics: Vec<DeclarationSourceDiagnostic>,
    allocation_failed: bool,
}

impl DiagnosticBuffer {
    fn with_capacity(count: usize, location: &str) -> Result<Self, DeclarationSourceError> {
        let mut diagnostics = Vec::new();
        diagnostics
            .try_reserve_exact(count)
            .map_err(|_| resource_error(location, "diagnostic vector allocation failed"))?;
        Ok(Self {
            diagnostics,
            allocation_failed: false,
        })
    }

    fn push(&mut self, diagnostic: DeclarationSourceDiagnostic) {
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
    ) -> Result<Vec<DeclarationSourceDiagnostic>, DeclarationSourceError> {
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

fn snapshot_name(snapshot: SourceSnapshotRole) -> &'static str {
    match snapshot {
        SourceSnapshotRole::Release => "release",
        SourceSnapshotRole::Development => "development",
    }
}

fn diagnostic(
    code: &str,
    location: impl Into<String>,
    message: impl Into<String>,
) -> DeclarationSourceDiagnostic {
    DeclarationSourceDiagnostic {
        code: code.to_owned(),
        location: location.into(),
        message: message.into(),
    }
}

fn single_error(diagnostic: DeclarationSourceDiagnostic) -> DeclarationSourceError {
    DeclarationSourceError::new(vec![diagnostic])
}

fn resource_error(location: &str, message: impl Into<String>) -> DeclarationSourceError {
    single_error(diagnostic("resource_limit", location, message))
}

fn requirement_identities(
    requirements: &DeclarationRequirementProjection,
) -> impl Iterator<Item = SourceIdentity<'_>> {
    requirements
        .parameters
        .iter()
        .map(|requirement| SourceIdentity {
            snapshot: requirement.snapshot,
            revision: &requirement.revision,
            path: &requirement.file.path,
            blob: &requirement.file.git_blob_sha1,
        })
        .chain(
            requirements
                .connectors
                .iter()
                .map(|requirement| SourceIdentity {
                    snapshot: requirement.snapshot,
                    revision: &requirement.revision,
                    path: &requirement.file.path,
                    blob: &requirement.file.git_blob_sha1,
                }),
        )
}

fn unique_identities<'a>(
    requirements: &'a DeclarationRequirementProjection,
    max_documents: usize,
) -> Result<Vec<SourceIdentity<'a>>, DeclarationSourceError> {
    let mut identities = Vec::new();
    for identity in requirement_identities(requirements) {
        if identities.contains(&identity) {
            continue;
        }
        if identities.len() == max_documents {
            return Err(resource_error(
                "$.requirements",
                format!("unique document count exceeds max_documents {max_documents}"),
            ));
        }
        identities.try_reserve(1).map_err(|_| {
            resource_error("$.requirements", "source identity vector allocation failed")
        })?;
        identities.push(identity);
    }
    identities.sort_unstable();
    Ok(identities)
}

fn validate_paths(identities: &[SourceIdentity<'_>]) -> Result<(), DeclarationSourceError> {
    let mut diagnostics = DiagnosticBuffer::with_capacity(identities.len(), "$.requirements")?;
    for identity in identities {
        let problem = match safe_source_path(identity.path) {
            Ok(()) if identity.path.ends_with(".mo") => continue,
            Ok(()) => "source file path must end in `.mo`",
            Err(problem) => problem,
        };
        diagnostics.push(diagnostic(
            "invalid_source_path",
            identity.location(),
            problem,
        ));
    }

    let diagnostics = diagnostics.finish("$.requirements")?;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(DeclarationSourceError::new(diagnostics))
    }
}

#[derive(Clone, Copy)]
enum IoOperation {
    Open,
    Metadata,
    Read,
}

fn io_diagnostic(
    identity: SourceIdentity<'_>,
    operation: IoOperation,
    kind: io::ErrorKind,
) -> DeclarationSourceDiagnostic {
    let (code, message) = match kind {
        io::ErrorKind::NotFound => ("source_not_found", "source entry was not found"),
        io::ErrorKind::PermissionDenied => {
            ("source_access_denied", "source entry access was denied")
        }
        io::ErrorKind::InvalidInput => (
            "source_path_unavailable",
            "source entry could not be resolved within the supplied root",
        ),
        io::ErrorKind::Unsupported => (
            "source_operation_unsupported",
            "source entry operation is unsupported",
        ),
        _ => match operation {
            IoOperation::Open => ("source_open_failed", "source entry could not be opened"),
            IoOperation::Metadata => (
                "source_metadata_failed",
                "opened source metadata could not be read",
            ),
            IoOperation::Read => (
                "source_read_failed",
                "opened source bytes could not be read",
            ),
        },
    };
    diagnostic(code, identity.location(), message)
}

fn checked_diagnostic_capacity(document_count: usize) -> Option<usize> {
    document_count.checked_mul(2)?.checked_add(1)
}

fn reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    location: &str,
    label: &str,
) -> Result<(), DeclarationSourceError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| resource_error(location, format!("{label} allocation failed")))
}

fn preflight_sources<'a>(
    identities: &[SourceIdentity<'a>],
    roots: DeclarationSourceRoots<'_>,
    limits: DeclarationSourceLimits,
) -> Result<Vec<OpenedSource<'a>>, DeclarationSourceError> {
    let diagnostic_capacity = checked_diagnostic_capacity(identities.len())
        .ok_or_else(|| resource_error("$.sources", "preflight diagnostic count overflows usize"))?;
    let mut diagnostics = DiagnosticBuffer::with_capacity(diagnostic_capacity, "$.sources")?;
    let mut opened = Vec::new();
    reserve_exact(
        &mut opened,
        identities.len(),
        "$.sources",
        "opened source handle vector",
    )?;
    let mut metadata_total = Some(0_usize);

    for identity in identities.iter().copied() {
        let file = match roots.for_snapshot(identity.snapshot).open(identity.path) {
            Ok(file) => file,
            Err(error) => {
                diagnostics.push(io_diagnostic(identity, IoOperation::Open, error.kind()));
                continue;
            }
        };
        let metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                diagnostics.push(io_diagnostic(identity, IoOperation::Metadata, error.kind()));
                continue;
            }
        };
        if !metadata.is_file() {
            diagnostics.push(diagnostic(
                "source_not_regular",
                identity.location(),
                "opened source entry is not a regular file",
            ));
            continue;
        }
        let metadata_len = match usize::try_from(metadata.len()) {
            Ok(length) => length,
            Err(_) => {
                diagnostics.push(diagnostic(
                    "resource_limit",
                    identity.location(),
                    "opened source metadata size does not fit usize",
                ));
                continue;
            }
        };
        if metadata_len > limits.max_source_bytes {
            diagnostics.push(diagnostic(
                "resource_limit",
                identity.location(),
                format!(
                    "source metadata byte count {metadata_len} exceeds max_source_bytes {}",
                    limits.max_source_bytes
                ),
            ));
        }
        metadata_total = match metadata_total {
            Some(total) => match total.checked_add(metadata_len) {
                Some(next) => Some(next),
                None => {
                    diagnostics.push(diagnostic(
                        "resource_limit",
                        "$.sources",
                        "total source metadata byte count overflows usize",
                    ));
                    None
                }
            },
            None => None,
        };
        opened.push(OpenedSource {
            identity,
            file,
            metadata_len,
        });
    }

    if let Some(total) = metadata_total
        && total > limits.max_total_source_bytes
    {
        diagnostics.push(diagnostic(
            "resource_limit",
            "$.sources",
            format!(
                "total source metadata byte count {total} exceeds max_total_source_bytes {}",
                limits.max_total_source_bytes
            ),
        ));
    }

    let diagnostics = diagnostics.finish("$.sources")?;
    if diagnostics.is_empty() {
        Ok(opened)
    } else {
        Err(DeclarationSourceError::new(diagnostics))
    }
}

fn bounded_read<R: Read>(
    reader: &mut R,
    identity: SourceIdentity<'_>,
    metadata_len: usize,
    total_before: usize,
    limits: DeclarationSourceLimits,
) -> Result<Vec<u8>, DeclarationSourceError> {
    if metadata_len > limits.max_source_bytes {
        return Err(resource_error(
            &identity.location(),
            format!(
                "source metadata byte count {metadata_len} exceeds max_source_bytes {}",
                limits.max_source_bytes
            ),
        ));
    }
    let metadata_total = total_before.checked_add(metadata_len).ok_or_else(|| {
        resource_error(
            &identity.location(),
            "total source metadata byte count overflows usize",
        )
    })?;
    if metadata_total > limits.max_total_source_bytes {
        return Err(resource_error(
            &identity.location(),
            format!(
                "total source metadata byte count {metadata_total} exceeds max_total_source_bytes {}",
                limits.max_total_source_bytes
            ),
        ));
    }

    let mut bytes = Vec::new();
    reserve_exact(
        &mut bytes,
        metadata_len,
        &identity.location(),
        "source byte vector",
    )?;
    let mut buffer = [0_u8; READ_BUFFER_BYTES];

    loop {
        let per_source_remaining = limits
            .max_source_bytes
            .checked_sub(bytes.len())
            .ok_or_else(|| {
                resource_error(
                    &identity.location(),
                    "source byte count exceeds max_source_bytes",
                )
            })?;
        let current_total = total_before.checked_add(bytes.len()).ok_or_else(|| {
            resource_error(
                &identity.location(),
                "total source byte count overflows usize",
            )
        })?;
        let total_remaining = limits
            .max_total_source_bytes
            .checked_sub(current_total)
            .ok_or_else(|| {
                resource_error(
                    &identity.location(),
                    "total source byte count exceeds max_total_source_bytes",
                )
            })?;
        let allowed = per_source_remaining.min(total_remaining);
        let probe_bytes = allowed.saturating_add(1).min(buffer.len());
        let count = match reader.read(&mut buffer[..probe_bytes]) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(single_error(io_diagnostic(
                    identity,
                    IoOperation::Read,
                    error.kind(),
                )));
            }
        };
        if count == 0 {
            break;
        }
        if count > allowed {
            let message = if per_source_remaining <= total_remaining {
                format!(
                    "source byte count exceeds max_source_bytes {}",
                    limits.max_source_bytes
                )
            } else {
                format!(
                    "total source byte count exceeds max_total_source_bytes {}",
                    limits.max_total_source_bytes
                )
            };
            return Err(resource_error(&identity.location(), message));
        }
        reserve_exact(
            &mut bytes,
            count,
            &identity.location(),
            "source byte vector",
        )?;
        bytes.extend_from_slice(&buffer[..count]);
    }

    if bytes.len() != metadata_len {
        return Err(single_error(diagnostic(
            "source_changed",
            identity.location(),
            format!(
                "opened source size changed from metadata {metadata_len} bytes to {} bytes",
                bytes.len()
            ),
        )));
    }
    Ok(bytes)
}

fn copy_string(value: &str, location: &str, label: &str) -> Result<String, DeclarationSourceError> {
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| resource_error(location, format!("{label} allocation failed")))?;
    copy.push_str(value);
    Ok(copy)
}

/// Reads every unique required source identity in full-identity order.
///
/// Release identities precede development identities; each role is then
/// ordered by revision, path, and claimed blob. Exact tuple duplicates collapse,
/// while a difference in any tuple field remains a separate document. Identity
/// strings and bytes are copied without content validation.
///
/// Path validation and metadata checks complete before source byte vectors are
/// allocated. Open handles are retained through reading, so descriptor use is
/// bounded by `limits.max_documents`. Any failure discards all documents.
pub fn read_declaration_sources(
    requirements: &DeclarationRequirementProjection,
    roots: DeclarationSourceRoots<'_>,
    limits: DeclarationSourceLimits,
) -> Result<Vec<DeclarationSourceDocument>, DeclarationSourceError> {
    let identities = unique_identities(requirements, limits.max_documents)?;
    validate_paths(&identities)?;
    let opened = preflight_sources(&identities, roots, limits)?;
    let mut documents = Vec::new();
    reserve_exact(
        &mut documents,
        opened.len(),
        "$.sources",
        "source document vector",
    )?;
    let mut total_bytes = 0_usize;

    for mut source in opened {
        let bytes = bounded_read(
            &mut source.file,
            source.identity,
            source.metadata_len,
            total_bytes,
            limits,
        )?;
        total_bytes = total_bytes.checked_add(bytes.len()).ok_or_else(|| {
            resource_error("$.sources", "total source byte count overflows usize")
        })?;
        let location = source.identity.location();
        documents.push(DeclarationSourceDocument {
            snapshot: source.identity.snapshot,
            revision: copy_string(source.identity.revision, &location, "source revision")?,
            file: SourceFileLocator {
                path: copy_string(source.identity.path, &location, "source path")?,
                git_blob_sha1: copy_string(source.identity.blob, &location, "source blob")?,
            },
            bytes,
        });
    }

    Ok(documents)
}

#[cfg(test)]
mod tests;
