//! Sealed three-IR input and the only complete built-in object pipeline.

use std::fmt;
use std::path::Path;

use scoop_slib::{
    CanonicalDefinedLinkSymbolOwnerSetV1, ConeRecord, DependencyRecord, ProducerRecord,
};

use crate::{
    AssembledStrongArtifactProductionV1, BuiltinObjectProductionError,
    PlannedBuiltinObjectProductionV1, StrongArtifactMetadataInputV1, StrongArtifactProductionError,
};

mod cross_cone;
pub use cross_cone::{CrossConeStrongIrArtifactProductionError, CrossConeStrongIrProductionV1};

/// One exact HIR/MIR/LIR chain proven admissible for strong object production.
pub struct SingleConeStrongIrProductionV1 {
    hir_foundation: scoop_hir::OdrFreeHirFoundation,
    hir_production: scoop_hir::CoreBootstrapInterfaceSectionV1,
    mir_foundation: scoop_mir::OdrFreeMirFoundation,
    mir_production: scoop_mir::CoreBootstrapBridgeSectionV1,
    lir: scoop_lir::SingleConeStrongLirOutput,
}

impl SingleConeStrongIrProductionV1 {
    pub(crate) const fn new(
        hir_foundation: scoop_hir::OdrFreeHirFoundation,
        hir_production: scoop_hir::CoreBootstrapInterfaceSectionV1,
        mir_foundation: scoop_mir::OdrFreeMirFoundation,
        mir_production: scoop_mir::CoreBootstrapBridgeSectionV1,
        lir: scoop_lir::SingleConeStrongLirOutput,
    ) -> Self {
        Self {
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir,
        }
    }

    pub const fn lir_output(&self) -> &scoop_lir::SingleConeStrongLirOutput {
        &self.lir
    }

    pub const fn hir_foundation(&self) -> &scoop_hir::OdrFreeHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_production(&self) -> &scoop_hir::CoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation(&self) -> &scoop_mir::OdrFreeMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production(&self) -> &scoop_mir::CoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    /// Runs the complete producer/verifier/finalizer chain and assembles the
    /// resulting final members into one canonical archive.
    #[allow(clippy::too_many_arguments)]
    pub fn produce_artifact(
        self,
        producer: ProducerRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        temporary_parent: &Path,
        target: &scoop_toolchain::ResolvedTargetProfile,
        core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    ) -> Result<AssembledStrongArtifactProductionV1, StrongIrArtifactProductionError> {
        let entry_source =
            scoop_lir_lower::lower_entry_production_source(self.mir_production.entry_bridge());
        let backend =
            scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection())
                .map_err(StrongIrArtifactProductionError::Codegen)?;
        let scoop_objects = scoop_codegen::emit_object_set(
            &self.lir,
            cone.coordinate(),
            entry_source,
            temporary_parent,
            backend,
        )
        .map_err(StrongIrArtifactProductionError::Codegen)?;
        let c_bridge_objects = scoop_codegen::emit_c_bridge_object_set(
            &self.lir,
            temporary_parent,
            target.c_bridge_toolchain(),
        )
        .map_err(StrongIrArtifactProductionError::Codegen)?;
        let code =
            PlannedBuiltinObjectProductionV1::from_codegen(&scoop_objects, &c_bridge_objects)
                .and_then(PlannedBuiltinObjectProductionV1::verify_c_bridge_envelopes)
                .and_then(|production| production.verify_strong_relocations())
                .and_then(|production| production.verify_digest_patch_sites())
                .and_then(|production| production.verify_stackmaps())
                .and_then(|production| production.verify_registration_objects())
                .and_then(|production| production.fingerprint_registration_object_leaves())
                .and_then(|production| production.verify_link_symbol_requirements(core_owners))
                .and_then(|production| production.fingerprint_registration_dependencies())
                .and_then(|production| production.finalize_strong_objects())
                .and_then(|production| {
                    production.fingerprint_code(
                        &cone,
                        &direct_dependencies,
                        self.hir_foundation.as_canonical().counts().sources,
                    )
                })
                .map_err(StrongIrArtifactProductionError::Objects)?;
        code.assemble_strong_artifact(StrongArtifactMetadataInputV1::new(
            producer,
            cone,
            direct_dependencies,
            &self.hir_foundation,
            &self.hir_production,
            &self.mir_foundation,
            &self.mir_production,
        ))
        .map_err(StrongIrArtifactProductionError::Artifact)
    }
}

#[derive(Debug)]
pub enum StrongIrArtifactProductionError {
    Codegen(scoop_codegen::CodegenError),
    Objects(BuiltinObjectProductionError),
    Artifact(StrongArtifactProductionError),
}

impl fmt::Display for StrongIrArtifactProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codegen(source) => source.fmt(formatter),
            Self::Objects(source) => source.fmt(formatter),
            Self::Artifact(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for StrongIrArtifactProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Codegen(source) => source,
            Self::Objects(source) => source,
            Self::Artifact(source) => source,
        })
    }
}
