//! Semantic data retained after the input archive has been consumed.

use super::*;
use crate::graph::ArtifactMetadata;

pub(crate) struct LayoutSemanticSections {
    pub(crate) metadata: std::rc::Rc<ArtifactMetadata>,
    pub(crate) view: crate::link_decode::DecodedLayoutView,
    pub(crate) identities: std::rc::Rc<ValidatedIdentityGraph>,
    pub(crate) foundations: CanonicalFoundationSet,
    pub(crate) hir_core: CoreBootstrapInterfaceSectionV1,
    pub(crate) hir_interface: CrossConeHirInterfaceSectionV1,
    pub(crate) hir_types: CrossConeTypeSemanticsSectionV1,
    pub(crate) hir_aliases: scoop_hir::CanonicalTypeAliasExpansionsV1,
    pub(crate) mir_core: CoreBootstrapBridgeSectionV1,
    pub(crate) mir_ordinary: CrossConeMirBridgeSectionV1,
}

impl PreparedCrossConeLayoutMirSections<'_> {
    pub(crate) fn into_semantics(self) -> LayoutSemanticSections {
        LayoutSemanticSections {
            metadata: std::rc::Rc::new(self.graph.into()),
            view: self.view,
            identities: std::rc::Rc::new(self.identities),
            foundations: self.foundations,
            hir_core: self.hir_core,
            hir_interface: self.hir_interface,
            hir_types: self.hir_types,
            hir_aliases: self.hir_aliases,
            mir_core: self.mir_core,
            mir_ordinary: self.mir_ordinary,
        }
    }
}
