//! Shared identity and native ABI checks for complete Compile artifacts.

use scoop_hir::DecodedHirFoundation;
use scoop_identity::{IdentityValidationError, PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_lir::DecodedLirFoundation;
use scoop_mir::DecodedMirFoundation;

use crate::ValidatedGraphArtifact;

mod commit;
pub(crate) use commit::commit_identity_graph;
pub use commit::{
    CompileCapabilityProfile, CompileCommitError, CrossConeGenericProfile, SingleConeStrongProfile,
    ValidatedCompileArtifact,
};
mod native_boundary;
pub(crate) use native_boundary::{
    AbiReplayDependency, NativeBoundaryFoundationView, collect_abi_types,
    replay_canonical_scoop_abi, validate_native_boundary_parts,
    validate_shared_native_boundary_parts,
};
pub use native_boundary::{NativeBoundaryCompileError, NativeBoundaryTargetError};

pub(crate) fn validate_foundation_identity_graph(
    graph: &mut ValidatedGraphArtifact<'_>,
    hir: &DecodedHirFoundation,
    mir: &DecodedMirFoundation,
    lir: &DecodedLirFoundation,
) -> Result<ValidatedIdentityGraph, IdentityValidationError> {
    validate_foundation_identity_graph_with_authorities(graph, hir, mir, lir, std::iter::empty())
}

pub(crate) fn validate_foundation_identity_graph_with_authorities<'authority>(
    graph: &mut ValidatedGraphArtifact<'_>,
    hir: &DecodedHirFoundation,
    mir: &DecodedMirFoundation,
    lir: &DecodedLirFoundation,
    external_authorities: impl IntoIterator<Item = &'authority ValidatedIdentityGraph>,
) -> Result<ValidatedIdentityGraph, IdentityValidationError> {
    let producer = graph.identity();
    let manifest = graph.envelope.manifest();
    let mut validation = PendingIdentityValidation::new();
    validation.register_authority(producer)?;
    for dependency in manifest.direct_dependencies() {
        validation.register_authority(dependency.identity())?;
    }

    hir.register_identities(&mut validation)?;
    mir.register_identities(&mut validation)?;
    lir.register_identities(&mut validation)?;
    for authority in external_authorities {
        validation.register_external_graph_authorities(authority)?;
    }

    hir.resolve_identities(&mut validation)?;
    mir.resolve_identities(&mut validation)?;
    lir.resolve_identities(&mut validation)?;

    validation.finish()
}

#[cfg(test)]
mod tests;
