//! Typed composition of declaration requirements, scalar binding, and syntax checks.
//!
//! Requirements and binding fan out from caller-supplied source claims. Source
//! bytes remain caller-owned, and successful output is the detached bound
//! projection rather than persisted declaration evidence.

use std::fmt;

use crate::bound_scalars::{BoundScalarError, BoundScalarProjection, bind_scalar_source_claims};
use crate::declaration_requirements::{
    DeclarationRequirementError, project_declaration_requirements,
};
use crate::declaration_syntax::{
    DeclarationSourceDocument, DeclarationSyntaxError, DeclarationSyntaxLimits,
    check_owner_declaration_syntax,
};
use crate::scalar_names::NamedScalarProjection;
use crate::scalar_source_claims::ScalarSourceClaimProjection;

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
    let requirements = project_declaration_requirements(source_claim_projection)
        .map_err(DeclarationPipelineError::Requirements)?;
    let bound_projection = bind_scalar_source_claims(named_projection, source_claim_projection)
        .map_err(DeclarationPipelineError::Binding)?;
    check_owner_declaration_syntax(&requirements, documents, limits)
        .map_err(DeclarationPipelineError::Syntax)?;
    Ok(bound_projection)
}

#[cfg(test)]
mod tests;
