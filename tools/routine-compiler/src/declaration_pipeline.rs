//! Typed composition of declaration requirements, scalar binding, source acquisition,
//! and syntax checks.
//!
//! Requirements and binding fan out from caller-supplied source claims. The
//! in-memory entry point borrows caller-owned bytes; the root entry point reads
//! them through caller-supplied directory capabilities. Successful output is
//! the detached bound projection rather than persisted declaration evidence.

use std::fmt;

use crate::bound_scalars::{BoundScalarError, BoundScalarProjection, bind_scalar_source_claims};
use crate::declaration_requirements::{
    DeclarationRequirementError, DeclarationRequirementProjection, project_declaration_requirements,
};
use crate::declaration_source::{
    DeclarationSourceError, DeclarationSourceLimits, DeclarationSourceRoots,
    read_declaration_sources,
};
use crate::declaration_syntax::{
    DeclarationSourceDocument, DeclarationSyntaxError, DeclarationSyntaxLimits,
    check_owner_declaration_syntax,
};
use crate::scalar_names::NamedScalarProjection;
use crate::scalar_source_claims::ScalarSourceClaimProjection;

/// Inclusive bounds shared by root acquisition and declaration syntax checking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclarationRootPipelineLimits {
    /// Maximum number of unique source documents.
    pub max_documents: usize,
    /// Maximum number of owner declaration requirements.
    pub max_requirements: usize,
    /// Maximum byte count for one source document.
    pub max_source_bytes: usize,
    /// Maximum byte count across all source documents.
    pub max_total_source_bytes: usize,
    /// Maximum number of direct members in one parsed owner class.
    pub max_direct_members: usize,
}

impl From<DeclarationRootPipelineLimits> for DeclarationSourceLimits {
    fn from(limits: DeclarationRootPipelineLimits) -> Self {
        Self {
            max_documents: limits.max_documents,
            max_source_bytes: limits.max_source_bytes,
            max_total_source_bytes: limits.max_total_source_bytes,
        }
    }
}

impl From<DeclarationRootPipelineLimits> for DeclarationSyntaxLimits {
    fn from(limits: DeclarationRootPipelineLimits) -> Self {
        Self {
            max_documents: limits.max_documents,
            max_requirements: limits.max_requirements,
            max_source_bytes: limits.max_source_bytes,
            max_total_source_bytes: limits.max_total_source_bytes,
            max_direct_members: limits.max_direct_members,
        }
    }
}

/// Failure from the first declaration pipeline stage that rejects its input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclarationPipelineError {
    /// Declaration requirements could not be projected from source claims.
    Requirements(DeclarationRequirementError),
    /// Named scalar rows and source claims could not be bound.
    Binding(BoundScalarError),
    /// Supplied source bytes failed declaration syntax checks.
    Syntax(DeclarationSyntaxError),
}

impl fmt::Display for DeclarationPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requirements(error) => {
                write!(formatter, "declaration requirements failed: {error}")
            }
            Self::Binding(error) => write!(formatter, "scalar binding failed: {error}"),
            Self::Syntax(error) => write!(formatter, "declaration syntax failed: {error}"),
        }
    }
}

impl std::error::Error for DeclarationPipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Requirements(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Syntax(error) => Some(error),
        }
    }
}

/// Failure from source acquisition or the typed declaration pipeline around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclarationRootPipelineError {
    /// Requirements, binding, or syntax checking failed.
    Pipeline(DeclarationPipelineError),
    /// Capability-relative source acquisition failed.
    Source(DeclarationSourceError),
}

impl fmt::Display for DeclarationRootPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipeline(error) => write!(formatter, "declaration pipeline failed: {error}"),
            Self::Source(error) => {
                write!(formatter, "declaration source acquisition failed: {error}")
            }
        }
    }
}

impl std::error::Error for DeclarationRootPipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pipeline(error) => Some(error),
            Self::Source(error) => Some(error),
        }
    }
}

struct PreparedDeclarationPipeline {
    requirements: DeclarationRequirementProjection,
    bound_projection: BoundScalarProjection,
}

fn prepare_declaration_pipeline(
    named_projection: &NamedScalarProjection,
    source_claim_projection: &ScalarSourceClaimProjection,
) -> Result<PreparedDeclarationPipeline, DeclarationPipelineError> {
    let requirements = project_declaration_requirements(source_claim_projection)
        .map_err(DeclarationPipelineError::Requirements)?;
    let bound_projection = bind_scalar_source_claims(named_projection, source_claim_projection)
        .map_err(DeclarationPipelineError::Binding)?;
    Ok(PreparedDeclarationPipeline {
        requirements,
        bound_projection,
    })
}

fn complete_declaration_pipeline(
    prepared: PreparedDeclarationPipeline,
    documents: &[DeclarationSourceDocument],
    limits: DeclarationSyntaxLimits,
) -> Result<BoundScalarProjection, DeclarationPipelineError> {
    check_owner_declaration_syntax(&prepared.requirements, documents, limits)
        .map_err(DeclarationPipelineError::Syntax)?;
    Ok(prepared.bound_projection)
}

/// Projects requirements, binds scalar claims, and checks supplied source bytes.
///
/// Requirements are derived directly from `source_claim_projection`; the bound
/// projection is a parallel output and is returned only after syntax succeeds.
/// Inputs are borrowed, and this function performs no source acquisition or I/O.
///
/// # Errors
///
/// Returns the first failed stage as a variant containing its unchanged error.
pub fn check_declaration_pipeline(
    named_projection: &NamedScalarProjection,
    source_claim_projection: &ScalarSourceClaimProjection,
    documents: &[DeclarationSourceDocument],
    limits: DeclarationSyntaxLimits,
) -> Result<BoundScalarProjection, DeclarationPipelineError> {
    let prepared = prepare_declaration_pipeline(named_projection, source_claim_projection)?;
    complete_declaration_pipeline(prepared, documents, limits)
}

/// Projects requirements, binds scalar claims, acquires source bytes, and checks syntax.
///
/// Both directory capabilities remain borrowed. Requirements and binding finish
/// before either root is read; acquisition finishes before syntax checking. The
/// bound projection is returned only after every stage succeeds.
///
/// # Errors
///
/// Returns the first failed stage with its unchanged pipeline or source error.
pub fn check_declaration_pipeline_from_roots(
    named_projection: &NamedScalarProjection,
    source_claim_projection: &ScalarSourceClaimProjection,
    roots: DeclarationSourceRoots<'_>,
    limits: DeclarationRootPipelineLimits,
) -> Result<BoundScalarProjection, DeclarationRootPipelineError> {
    let prepared = prepare_declaration_pipeline(named_projection, source_claim_projection)
        .map_err(DeclarationRootPipelineError::Pipeline)?;
    let documents = read_declaration_sources(
        &prepared.requirements,
        roots,
        DeclarationSourceLimits::from(limits),
    )
    .map_err(DeclarationRootPipelineError::Source)?;
    complete_declaration_pipeline(prepared, &documents, DeclarationSyntaxLimits::from(limits))
        .map_err(DeclarationRootPipelineError::Pipeline)
}

#[cfg(test)]
mod tests;
