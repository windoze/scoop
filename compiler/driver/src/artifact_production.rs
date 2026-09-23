//! Terminal archive assembly and two-view publication for strong production.

use std::fmt;
use std::path::Path;

use scoop_slib::{
    AssembledCrossConeStrongArtifactV1, ConeRecord, CrossConeArtifactPublishError,
    CrossConeStrongArtifactInputV1, CrossConeStrongArtifactWriteError, DependencyRecord,
    ProducerRecord, PublishedCrossConeArtifact, publish_cross_cone_artifact,
};
use scoop_wire::DecodeLimits;

use crate::CrossConeCodeFingerprintedObjectProductionV1;

mod layout;
pub use layout::{CrossConeLayoutArtifactMetadataInputV1, LayoutArtifactProductionError};

/// The complete semantic sections paired with one cross-Cone Code proof.
pub struct CrossConeStrongArtifactMetadataInputV1<'ir> {
    producer: ProducerRecord,
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    hir_foundation: &'ir scoop_hir::OdrFreeHirFoundation,
    hir_core: &'ir scoop_hir::CoreBootstrapInterfaceSectionV1,
    hir_cross_cone: scoop_hir::CrossConeHirInterfaceSectionV1,
    mir_foundation: &'ir scoop_mir::OdrFreeMirFoundation,
    mir_core: &'ir scoop_mir::CoreBootstrapBridgeSectionV1,
    mir_cross_cone: &'ir scoop_mir::CrossConeMirBridgeSectionV1,
    lir_cross_cone: &'ir scoop_lir::CrossConeLirBridgeSectionV1,
}

impl<'ir> CrossConeStrongArtifactMetadataInputV1<'ir> {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        producer: ProducerRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        hir_foundation: &'ir scoop_hir::OdrFreeHirFoundation,
        hir_core: &'ir scoop_hir::CoreBootstrapInterfaceSectionV1,
        hir_cross_cone: scoop_hir::CrossConeHirInterfaceSectionV1,
        mir_foundation: &'ir scoop_mir::OdrFreeMirFoundation,
        mir_core: &'ir scoop_mir::CoreBootstrapBridgeSectionV1,
        mir_cross_cone: &'ir scoop_mir::CrossConeMirBridgeSectionV1,
        lir_cross_cone: &'ir scoop_lir::CrossConeLirBridgeSectionV1,
    ) -> Self {
        Self {
            producer,
            cone,
            direct_dependencies,
            hir_foundation,
            hir_core,
            hir_cross_cone,
            mir_foundation,
            mir_core,
            mir_cross_cone,
            lir_cross_cone,
        }
    }
}

/// Final cross-Cone archive bytes plus the closure inputs needed at publish.
#[derive(Debug)]
pub struct AssembledCrossConeArtifactProductionV1 {
    artifact: AssembledCrossConeStrongArtifactV1,
    current: scoop_identity::ConeIdentity,
    direct: Vec<scoop_identity::ConeIdentity>,
    c_bridge_profile: scoop_lir::CBridgeToolchainProfileV1,
}

impl CrossConeCodeFingerprintedObjectProductionV1 {
    pub fn assemble_cross_cone_strong_artifact(
        self,
        metadata: CrossConeStrongArtifactMetadataInputV1<'_>,
    ) -> Result<AssembledCrossConeArtifactProductionV1, CrossConeArtifactProductionError> {
        let (target_selection, lir_foundation, c_bridge_profile, members, code) =
            self.into_archive_parts();
        let current = metadata.cone.identity();
        let direct = metadata
            .direct_dependencies
            .iter()
            .map(DependencyRecord::identity)
            .collect();
        let artifact =
            AssembledCrossConeStrongArtifactV1::write(CrossConeStrongArtifactInputV1::new(
                metadata.producer,
                metadata.cone,
                metadata.direct_dependencies,
                metadata.hir_foundation,
                metadata.hir_core,
                metadata.hir_cross_cone,
                metadata.mir_foundation,
                metadata.mir_core,
                metadata.mir_cross_cone,
                &lir_foundation,
                metadata.lir_cross_cone,
                code,
                members,
            ))
            .map_err(CrossConeArtifactProductionError::Assembly)?;
        if artifact.target_selection() != target_selection {
            return Err(CrossConeArtifactProductionError::TargetSelectionMismatch);
        }
        Ok(AssembledCrossConeArtifactProductionV1 {
            artifact,
            current,
            direct,
            c_bridge_profile,
        })
    }
}

impl AssembledCrossConeArtifactProductionV1 {
    pub const fn artifact_fingerprint(&self) -> scoop_slib::ArtifactFingerprint {
        self.artifact.artifact_fingerprint()
    }

    pub fn publish(
        self,
        destination: &Path,
        dependency_first: Vec<&[u8]>,
        limits: DecodeLimits,
    ) -> Result<PublishedCrossConeArtifact, CrossConeArtifactProductionError> {
        publish_cross_cone_artifact(
            self.artifact.as_bytes(),
            destination,
            limits,
            self.current,
            self.direct,
            dependency_first,
            self.artifact.target_selection(),
            &self.c_bridge_profile,
        )
        .map_err(CrossConeArtifactProductionError::Publication)
    }
}

#[derive(Debug)]
pub enum CrossConeArtifactProductionError {
    TargetSelectionMismatch,
    Assembly(CrossConeStrongArtifactWriteError),
    Publication(CrossConeArtifactPublishError),
}

impl fmt::Display for CrossConeArtifactProductionError {
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

impl std::error::Error for CrossConeArtifactProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TargetSelectionMismatch => None,
            Self::Assembly(source) => Some(source),
            Self::Publication(source) => Some(source),
        }
    }
}
