//! Layout-profile assembly from the actual V2 object and semantic products.

use super::*;
use crate::object_production::{BuiltinObjectProductionError as ObjectError, layout as objects};
use scoop_slib as slib;

mod error;
use LayoutArtifactProductionError as Error;
pub use error::LayoutArtifactProductionError;

pub struct CrossConeLayoutArtifactMetadataInputV1<'ir> {
    ordinary: CrossConeStrongArtifactMetadataInputV1<'ir>,
    hir_types: &'ir scoop_hir::CrossConeTypeSemanticsSectionV1,
    mir_types: &'ir scoop_mir::CrossConeMirTypeBridgeSectionV1<'ir>,
    lir_layout: &'ir scoop_lir::CrossConeLayoutAbiSectionV1<'ir>,
}

impl<'ir> CrossConeLayoutArtifactMetadataInputV1<'ir> {
    pub const fn new(
        ordinary: CrossConeStrongArtifactMetadataInputV1<'ir>,
        hir_types: &'ir scoop_hir::CrossConeTypeSemanticsSectionV1,
        mir_types: &'ir scoop_mir::CrossConeMirTypeBridgeSectionV1<'ir>,
        lir_layout: &'ir scoop_lir::CrossConeLayoutAbiSectionV1<'ir>,
    ) -> Self {
        Self {
            ordinary,
            hir_types,
            mir_types,
            lir_layout,
        }
    }

    pub fn assemble(
        self,
        emitted: scoop_codegen::EmittedStrongObjectSetV2,
        generated: &scoop_codegen::EmittedGeneratedCBridgeObjectSetV1,
        dependency_owners: &[slib::CanonicalDefinedLinkSymbolOwnerSetV1],
    ) -> Result<slib::AssembledCrossConeLayoutStrongArtifactV1, Error> {
        let prepared = objects::prepare(emitted, generated)?;
        let strong = prepared.patch_sites.builtins().strong_relocations().clone();
        let defined =
            slib::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong)
                .map_err(ObjectError::DefinedSymbols)?;
        let native = scoop_lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            prepared.target_selection.target(),
            &prepared.foundation,
        )
        .map_err(ObjectError::NativeRequirementSurface)?;
        let ordinary = slib::verify_cross_cone_strong_requirements_v1(
            prepared.target_selection.target(),
            strong,
            prepared.production.external_bridges().clone(),
            dependency_owners,
            self.ordinary.lir_cross_cone,
        )
        .map_err(ObjectError::DependencyRequirements)?;
        let shape =
            slib::verify_external_shape_requirements_v1(&ordinary, self.lir_layout.selected())
                .map_err(|error| Error::Layout(Box::new(error)))?;
        let undefined = objects::complete_requirements(&prepared, &native, &shape)?;
        let current = self.ordinary.cone.identity();
        let finalized = prepared.finalize(
            &undefined,
            &self.ordinary.cone,
            &self.ordinary.direct_dependencies,
            self.ordinary
                .hir_foundation
                .as_canonical()
                .source_count_for_cone(current),
        )?;
        let code = slib::compute_cross_cone_layout_code_fingerprint_v1(
            finalized.projection,
            native,
            defined,
            undefined,
            &shape,
        )
        .map_err(|error| Error::Code(Box::new(error)))?;
        let artifact = slib::AssembledCrossConeLayoutStrongArtifactV1::write(
            slib::CrossConeLayoutStrongArtifactInputV1::new(
                self.ordinary.producer,
                self.ordinary.cone,
                self.ordinary.direct_dependencies,
                self.ordinary.hir_foundation,
                self.ordinary.hir_core,
                self.ordinary.hir_cross_cone,
                self.hir_types,
                self.ordinary.mir_foundation,
                self.ordinary.mir_core,
                self.ordinary.mir_cross_cone,
                self.mir_types,
                &finalized.foundation,
                self.ordinary.lir_cross_cone,
                self.lir_layout,
                code,
                finalized.members,
            ),
        )
        .map_err(|error| Error::Assembly(Box::new(error)))?;
        if artifact.target_selection() != finalized.target_selection {
            return Err(Error::TargetSelectionMismatch);
        }
        Ok(artifact)
    }
}
