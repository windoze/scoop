//! Complete source and type sections for normal artifact assembly.

use scoop_slib::{ConeRecord, DependencyRecord, ProducerRecord};

mod layout;
pub use layout::{CrossConeLayoutArtifactMetadataInputV1, LayoutArtifactProductionError};

/// Complete shared semantic sections from the current compilation.
pub struct CrossConeStrongArtifactMetadataInputV1<'ir> {
    producer: ProducerRecord,
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    hir_foundation: &'ir scoop_hir::OdrFreeHirFoundation,
    hir_core: &'ir scoop_hir::CoreBootstrapInterfaceSectionV1,
    hir_cross_cone: scoop_hir::CrossConeHirInterfaceSectionV1,
    mir_foundation: &'ir scoop_mir::CanonicalMirFoundation,
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
        mir_foundation: &'ir scoop_mir::CanonicalMirFoundation,
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
