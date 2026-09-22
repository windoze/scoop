//! Sealed three-IR input for the cross-Cone strong artifact profile.

use std::fmt;
use std::path::Path;

use scoop_slib::{
    CanonicalDefinedLinkSymbolOwnerSetV1, ConeRecord, DependencyRecord, ProducerRecord,
};

use crate::{
    AssembledCrossConeArtifactProductionV1, BuiltinObjectProductionError,
    CrossConeArtifactProductionError, CrossConeStrongArtifactMetadataInputV1,
    PlannedBuiltinObjectProductionV1,
};

/// One exact HIR/MIR/LIR chain with both dedicated-core and general
/// cross-Cone metadata attached before object production begins.
pub struct CrossConeStrongIrProductionV1 {
    hir_foundation: scoop_hir::OdrFreeHirFoundation,
    hir_core: scoop_hir::CoreBootstrapInterfaceSectionV1,
    hir_cross_cone: scoop_hir::CrossConeHirInterfaceSectionV1,
    mir_foundation: scoop_mir::OdrFreeMirFoundation,
    mir_core: scoop_mir::CoreBootstrapBridgeSectionV1,
    mir_cross_cone: scoop_mir::CrossConeMirBridgeSectionV1,
    lir: scoop_lir::SingleConeStrongLirOutput,
    lir_cross_cone: scoop_lir::CrossConeLirBridgeSectionV1,
}

impl CrossConeStrongIrProductionV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        hir_foundation: scoop_hir::OdrFreeHirFoundation,
        hir_core: scoop_hir::CoreBootstrapInterfaceSectionV1,
        hir_cross_cone: scoop_hir::CrossConeHirInterfaceSectionV1,
        mir_foundation: scoop_mir::OdrFreeMirFoundation,
        mir_core: scoop_mir::CoreBootstrapBridgeSectionV1,
        mir_cross_cone: scoop_mir::CrossConeMirBridgeSectionV1,
        lir: scoop_lir::SingleConeStrongLirOutput,
        lir_cross_cone: scoop_lir::CrossConeLirBridgeSectionV1,
    ) -> Self {
        Self {
            hir_foundation,
            hir_core,
            hir_cross_cone,
            mir_foundation,
            mir_core,
            mir_cross_cone,
            lir,
            lir_cross_cone,
        }
    }

    pub const fn hir_foundation(&self) -> &scoop_hir::OdrFreeHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_cross_cone(&self) -> &scoop_hir::CrossConeHirInterfaceSectionV1 {
        &self.hir_cross_cone
    }

    pub const fn mir_cross_cone(&self) -> &scoop_mir::CrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone
    }

    pub const fn lir_cross_cone(&self) -> &scoop_lir::CrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone
    }

    /// Emits, verifies, fingerprints, and archives the complete cross-Cone
    /// artifact. Dependency linkage is taken only from the typed LIR bridge.
    #[allow(clippy::too_many_arguments)]
    pub fn produce_artifact(
        self,
        producer: ProducerRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        temporary_parent: &Path,
        target: &scoop_toolchain::ResolvedTargetProfile,
        dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    ) -> Result<AssembledCrossConeArtifactProductionV1, CrossConeStrongIrArtifactProductionError>
    {
        let Self {
            hir_foundation,
            hir_core,
            hir_cross_cone,
            mir_foundation,
            mir_core,
            mir_cross_cone,
            lir,
            lir_cross_cone,
        } = self;
        let entry_source = scoop_lir_lower::lower_entry_production_source(mir_core.entry_bridge());
        let backend =
            scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection())
                .map_err(CrossConeStrongIrArtifactProductionError::Codegen)?;
        let scoop_objects = scoop_codegen::emit_object_set(
            &lir,
            cone.coordinate(),
            entry_source,
            temporary_parent,
            backend,
        )
        .map_err(CrossConeStrongIrArtifactProductionError::Codegen)?;
        let c_bridge_objects = scoop_codegen::emit_c_bridge_object_set(
            &lir,
            temporary_parent,
            target.c_bridge_toolchain(),
        )
        .map_err(CrossConeStrongIrArtifactProductionError::Codegen)?;
        let code =
            PlannedBuiltinObjectProductionV1::from_codegen(&scoop_objects, &c_bridge_objects)
                .and_then(PlannedBuiltinObjectProductionV1::verify_c_bridge_envelopes)
                .and_then(|production| production.verify_strong_relocations())
                .and_then(|production| production.verify_digest_patch_sites())
                .and_then(|production| production.verify_stackmaps())
                .and_then(|production| production.verify_registration_objects())
                .and_then(|production| production.fingerprint_registration_object_leaves())
                .and_then(|production| {
                    production.verify_cross_cone_link_symbol_requirements(
                        dependency_owners,
                        &lir_cross_cone,
                    )
                })
                .and_then(|production| production.fingerprint_registration_dependencies())
                .and_then(|production| production.finalize_strong_objects())
                .and_then(|production| {
                    production.fingerprint_code(
                        &cone,
                        &direct_dependencies,
                        hir_foundation
                            .as_canonical()
                            .source_count_for_cone(cone.identity()),
                    )
                })
                .map_err(CrossConeStrongIrArtifactProductionError::Objects)?;
        code.assemble_cross_cone_strong_artifact(CrossConeStrongArtifactMetadataInputV1::new(
            producer,
            cone,
            direct_dependencies,
            &hir_foundation,
            &hir_core,
            hir_cross_cone,
            &mir_foundation,
            &mir_core,
            &mir_cross_cone,
            &lir_cross_cone,
        ))
        .map_err(CrossConeStrongIrArtifactProductionError::Artifact)
    }
}

#[derive(Debug)]
pub enum CrossConeStrongIrArtifactProductionError {
    Codegen(scoop_codegen::CodegenError),
    Objects(BuiltinObjectProductionError),
    Artifact(CrossConeArtifactProductionError),
}

impl fmt::Display for CrossConeStrongIrArtifactProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codegen(source) => source.fmt(formatter),
            Self::Objects(source) => source.fmt(formatter),
            Self::Artifact(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeStrongIrArtifactProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Codegen(source) => source,
            Self::Objects(source) => source,
            Self::Artifact(source) => source,
        })
    }
}
