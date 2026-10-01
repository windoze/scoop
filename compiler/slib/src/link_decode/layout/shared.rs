//! Share decoded Link inputs with semantic validation without reopening bytes.

use scoop_wire::WireError;

use super::*;
use crate::DecodedCrossConeLayoutCompileSections;

/// Complete Link-only wire retained alongside the same artifact's shared
/// semantic payloads. These fields still require object and fingerprint checks.
#[derive(Debug)]
pub struct DecodedCrossConeLayoutLinkOnlySections {
    pub(super) production_manifest: DecodedSingleConeProductionManifestV1,
    pub(super) link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    pub(super) cross_cone_link_closure: DecodedCrossConeLinkClosureSectionV1,
    pub(super) layout_link_closure: DecodedCrossConeLayoutLinkClosureSectionV1,
}

#[derive(Debug)]
pub(crate) enum DecodedLayoutView {
    Compile,
    Link(Box<DecodedCrossConeLayoutLinkOnlySections>),
}

impl DecodedLayoutView {
    pub(crate) fn link(&self) -> Option<&DecodedCrossConeLayoutLinkOnlySections> {
        match self {
            Self::Compile => None,
            Self::Link(sections) => Some(sections),
        }
    }
}

impl<'input> DecodedCrossConeLayoutLinkSections<'input> {
    /// Reuses the complete shared semantic pipeline. Link-only fields remain
    /// owned by this artifact and never become a machine or object permit.
    pub fn into_shared_sections(
        self,
    ) -> Result<DecodedCrossConeLayoutCompileSections<'input>, WireError> {
        let Self {
            graph,
            production_manifest,
            hir_foundation,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
            link_identity_closure,
            cross_cone_link_closure,
            layout_link_closure,
        } = self;

        let view = DecodedLayoutView::Link(Box::new(DecodedCrossConeLayoutLinkOnlySections {
            production_manifest,
            link_identity_closure,
            cross_cone_link_closure,
            layout_link_closure,
        }));
        Ok(DecodedCrossConeLayoutCompileSections {
            graph,
            view,
            hir_foundation,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        })
    }
}

impl DecodedCrossConeLayoutLinkOnlySections {
    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn cross_cone_link_closure_wire(&self) -> &DecodedCrossConeLinkClosureSectionV1 {
        &self.cross_cone_link_closure
    }

    pub const fn layout_link_closure_wire(&self) -> &DecodedCrossConeLayoutLinkClosureSectionV1 {
        &self.layout_link_closure
    }
}
