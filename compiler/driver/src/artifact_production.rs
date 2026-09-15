//! Terminal archive assembly and two-view publication for strong production.

use std::fmt;
use std::path::Path;

use scoop_slib::{
    AssembledSingleConeStrongArtifactV1, CanonicalDefinedLinkSymbolOwnerSetV1, ConeRecord,
    DependencyRecord, ProducerRecord, PublishedSingleConeArtifact, SingleConeArtifactPublishError,
    SingleConeStrongArtifactInputV1, SingleConeStrongArtifactWriteError,
    publish_single_cone_artifact,
};
use scoop_wire::DecodeLimits;

use crate::CodeFingerprintedObjectProductionV1;

/// The exact HIR/MIR metadata chain paired with the Cone/dependency request
/// that was already used to compute the terminal Code proof.
pub struct StrongArtifactMetadataInputV1<'ir> {
    producer: ProducerRecord,
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    hir_foundation: &'ir scoop_hir::OdrFreeHirFoundation,
    hir_production: &'ir scoop_hir::CoreBootstrapInterfaceSectionV1,
    mir_foundation: &'ir scoop_mir::OdrFreeMirFoundation,
    mir_production: &'ir scoop_mir::CoreBootstrapBridgeSectionV1,
}

impl<'ir> StrongArtifactMetadataInputV1<'ir> {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        producer: ProducerRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        hir_foundation: &'ir scoop_hir::OdrFreeHirFoundation,
        hir_production: &'ir scoop_hir::CoreBootstrapInterfaceSectionV1,
        mir_foundation: &'ir scoop_mir::OdrFreeMirFoundation,
        mir_production: &'ir scoop_mir::CoreBootstrapBridgeSectionV1,
    ) -> Self {
        Self {
            producer,
            cone,
            direct_dependencies,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
        }
    }
}

/// Final archive bytes together with the exact validation inputs needed by
/// the publication gate. No provisional object state survives this boundary.
#[derive(Debug)]
pub struct AssembledStrongArtifactProductionV1 {
    artifact: AssembledSingleConeStrongArtifactV1,
    external_bridges: scoop_lir::StrongExternalLirBridgeSurfaceV1,
    c_bridge_profile: scoop_lir::CBridgeToolchainProfileV1,
}

impl CodeFingerprintedObjectProductionV1 {
    pub fn assemble_strong_artifact(
        self,
        metadata: StrongArtifactMetadataInputV1<'_>,
    ) -> Result<AssembledStrongArtifactProductionV1, StrongArtifactProductionError> {
        let (target_selection, lir_foundation, c_bridge_profile, members, production_manifest) =
            self.into_archive_parts();
        let external_bridges = production_manifest
            .code_proof()
            .production()
            .strong_production()
            .external_bridges()
            .clone();
        let artifact =
            AssembledSingleConeStrongArtifactV1::write(SingleConeStrongArtifactInputV1::new(
                metadata.producer,
                metadata.cone,
                metadata.direct_dependencies,
                metadata.hir_foundation,
                metadata.hir_production,
                metadata.mir_foundation,
                metadata.mir_production,
                &lir_foundation,
                production_manifest,
                members,
            ))
            .map_err(StrongArtifactProductionError::Assembly)?;
        if artifact.target_selection() != target_selection {
            return Err(StrongArtifactProductionError::TargetSelectionMismatch);
        }
        Ok(AssembledStrongArtifactProductionV1 {
            artifact,
            external_bridges,
            c_bridge_profile,
        })
    }
}

impl AssembledStrongArtifactProductionV1 {
    pub const fn artifact_fingerprint(&self) -> scoop_slib::ArtifactFingerprint {
        self.artifact.artifact_fingerprint()
    }

    /// Closes the writable temporary file, reopens the same bytes independently
    /// through Compile and Link views, then atomically replaces `destination`.
    pub fn publish(
        self,
        destination: &Path,
        limits: DecodeLimits,
        core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    ) -> Result<PublishedSingleConeArtifact, StrongArtifactProductionError> {
        publish_single_cone_artifact(
            self.artifact.as_bytes(),
            destination,
            limits,
            self.artifact.target_selection(),
            &self.external_bridges,
            core_owners,
            &self.c_bridge_profile,
        )
        .map_err(StrongArtifactProductionError::Publication)
    }
}

#[derive(Debug)]
pub enum StrongArtifactProductionError {
    TargetSelectionMismatch,
    Assembly(SingleConeStrongArtifactWriteError),
    Publication(SingleConeArtifactPublishError),
}

impl fmt::Display for StrongArtifactProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetSelectionMismatch => formatter.write_str(
                "assembled artifact target selection differs from the finalized Code state",
            ),
            Self::Assembly(source) => source.fmt(formatter),
            Self::Publication(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for StrongArtifactProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TargetSelectionMismatch => None,
            Self::Assembly(source) => Some(source as &(dyn std::error::Error + 'static)),
            Self::Publication(source) => Some(source as &(dyn std::error::Error + 'static)),
        }
    }
}
