//! Capability-rooted composition of the typed routine compiler stages.
//!
//! The entry point accepts validated in-memory inputs and returns only the bound
//! scalar projection. Source bytes are read only after resolution, ABI projection,
//! name allocation, and source-claim projection have succeeded.

use std::fmt;

use crate::bound_scalars::BoundScalarProjection;
use crate::declaration_pipeline::{
    DeclarationRootPipelineError, DeclarationRootPipelineLimits,
    check_declaration_pipeline_from_roots,
};
use crate::declaration_source::DeclarationSourceRoots;
use crate::resolution::{
    ResolutionError, ResolutionLimits, ValidatedResolutionInput, resolve_validated,
};
use crate::scalar_abi::{EnumAbiMapping, ScalarAbiError, project_scalar_abi};
use crate::scalar_names::{ScalarNameError, allocate_scalar_names};
use crate::scalar_source_claims::{
    SourceClaimError, SourceClassClaim, SourceInventory, SourceMemberBinding, SourcePin,
    project_scalar_source_claims,
};

/// Failure from the first compiler stage that rejects its input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompilerPipelineError {
    /// Validated-input resolution failed.
    Resolution(ResolutionError),
    /// Scalar ABI projection or enum mapping failed.
    ScalarAbi(ScalarAbiError),
    /// Scalar-name allocation failed.
    ScalarNames(ScalarNameError),
    /// Inventory-anchored source-claim projection failed.
    SourceClaims(SourceClaimError),
    /// Declaration preparation, acquisition, or syntax checking failed.
    Declaration(DeclarationRootPipelineError),
}

impl fmt::Display for CompilerPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolution(error) => write!(formatter, "resolution failed: {error}"),
            Self::ScalarAbi(error) => write!(formatter, "scalar ABI projection failed: {error}"),
            Self::ScalarNames(error) => {
                write!(formatter, "scalar name allocation failed: {error}")
            }
            Self::SourceClaims(error) => {
                write!(formatter, "source claim projection failed: {error}")
            }
            Self::Declaration(error) => write!(formatter, "declaration pipeline failed: {error}"),
        }
    }
}

impl std::error::Error for CompilerPipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolution(error) => Some(error),
            Self::ScalarAbi(error) => Some(error),
            Self::ScalarNames(error) => Some(error),
            Self::SourceClaims(error) => Some(error),
            Self::Declaration(error) => Some(error),
        }
    }
}

/// Runs the typed compiler stages and reads declarations through borrowed roots.
///
/// Resolution and declaration limits are forwarded to their owning stages. The
/// function stops on the first error and returns no intermediate projection.
/// Inputs and directory capabilities remain borrowed.
///
/// # Errors
///
/// Returns the failed stage with its unchanged error.
#[allow(clippy::too_many_arguments)]
pub fn compile_validated_from_roots(
    resolution_input: &ValidatedResolutionInput,
    resolution_limits: ResolutionLimits,
    enum_mappings: &[EnumAbiMapping],
    source_inventory: &SourceInventory,
    source_pins: &[SourcePin],
    class_claims: &[SourceClassClaim],
    member_bindings: &[SourceMemberBinding],
    roots: DeclarationSourceRoots<'_>,
    declaration_limits: DeclarationRootPipelineLimits,
) -> Result<BoundScalarProjection, CompilerPipelineError> {
    let resolved = resolve_validated(resolution_input, resolution_limits)
        .map_err(CompilerPipelineError::Resolution)?;
    let scalar_abi =
        project_scalar_abi(&resolved, enum_mappings).map_err(CompilerPipelineError::ScalarAbi)?;
    let named = allocate_scalar_names(&scalar_abi).map_err(CompilerPipelineError::ScalarNames)?;
    let source_claims = project_scalar_source_claims(
        &named,
        source_inventory,
        source_pins,
        class_claims,
        member_bindings,
    )
    .map_err(CompilerPipelineError::SourceClaims)?;
    check_declaration_pipeline_from_roots(&named, &source_claims, roots, declaration_limits)
        .map_err(CompilerPipelineError::Declaration)
}

#[cfg(test)]
mod tests;
