//! Share decoded Link inputs with the artifact's semantic records.

use scoop_wire::WireError;

use super::*;
use crate::DecodedCrossConeLayoutCompileSections;

/// Link fields retained alongside the same artifact's shared semantic payloads.
#[derive(Debug)]
pub struct DecodedCrossConeLayoutLinkOnlySections {
    pub(super) link_identity_closure: DecodedLinkIdentityClosureSectionV1,

    pub(super) link_support: crate::DecodedLirLinkSupportSectionV1,
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
    /// Shares already decoded sections with Compile's semantic input.
    pub fn into_shared_sections(
        self,
    ) -> Result<DecodedCrossConeLayoutCompileSections<'input>, WireError> {
        let Self {
            graph,
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

            link_support,
        } = self;

        let view = DecodedLayoutView::Link(Box::new(DecodedCrossConeLayoutLinkOnlySections {
            link_identity_closure,

            link_support,
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
    pub const fn link_support_wire(&self) -> &crate::DecodedLirLinkSupportSectionV1 {
        &self.link_support
    }
    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }
}
