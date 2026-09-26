//! Semantic data retained after the input archive has been consumed.

use super::*;
use crate::graph::ArtifactMetadata;

pub(crate) struct LayoutSemanticSections {
    pub(crate) metadata: ArtifactMetadata,
    pub(crate) view: crate::link_decode::DecodedLayoutView,
    pub(crate) identities: ValidatedIdentityGraph,
    pub(crate) foundations: OdrFreeStrongFoundationSet,
    pub(crate) hir_core: CoreBootstrapInterfaceSectionV1,
    pub(crate) hir_interface: CrossConeHirInterfaceSectionV1,
    pub(crate) hir_types: CrossConeTypeSemanticsSectionV1,
    pub(crate) mir_core: CoreBootstrapBridgeSectionV1,
    pub(crate) mir_ordinary: CrossConeMirBridgeSectionV1,
}

impl PreparedCrossConeLayoutMirSections<'_> {
    pub(crate) fn into_semantics(self) -> LayoutSemanticSections {
        LayoutSemanticSections {
            metadata: self.graph.into(),
            view: self.view,
            identities: self.identities,
            foundations: self.foundations,
            hir_core: self.hir_core,
            hir_interface: self.hir_interface,
            hir_types: self.hir_types,
            mir_core: self.mir_core,
            mir_ordinary: self.mir_ordinary,
        }
    }
}
