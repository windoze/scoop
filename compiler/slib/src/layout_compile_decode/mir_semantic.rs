//! Legacy MIR-front replay before the M23-6 type-bridge closure.

use std::fmt;

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1, OdrFreeHirFoundation,
};
use scoop_identity::ValidatedIdentityGraph;
use scoop_lir::OdrFreeLirFoundation;
use scoop_mir::{
    CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, CrossConeMirBridgeValidationError,
    DecodedCrossConeMirTypeBridgeSectionV1, MirProductionValidationError, OdrFreeMirFoundation,
};

mod materializations;
mod owned;
pub(crate) use owned::LayoutSemanticSections;

use super::HirProductionValidatedCrossConeLayoutSections;
use crate::{
    ValidatedGraphArtifact,
    layout_compile_decode::DecodedCrossConeLayoutLirCandidates,
    strong_compile_decode::{
        OdrFreeStrongFoundationSet, StrongProfileRelationError, validate_strong_profile_relations,
    },
};

pub(crate) struct PreparedCrossConeLayoutMirSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    view: crate::link_decode::DecodedLayoutView,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core: CoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    hir_types: CrossConeTypeSemanticsSectionV1,
    hir_aliases: scoop_hir::CanonicalTypeAliasExpansionsV1,
    mir_core: CoreBootstrapBridgeSectionV1,
    mir_ordinary: CrossConeMirBridgeSectionV1,
}

pub(crate) struct PreparedLayoutMirSemanticParts<'a> {
    pub(crate) identities: &'a mut ValidatedIdentityGraph,
    pub(crate) hir_foundation: &'a OdrFreeHirFoundation,
    pub(crate) hir_core: &'a CoreBootstrapInterfaceSectionV1,
    pub(crate) hir_interface: &'a CrossConeHirInterfaceSectionV1,
    pub(crate) hir_types: &'a CrossConeTypeSemanticsSectionV1,
    pub(crate) mir_foundation: &'a OdrFreeMirFoundation,
    pub(crate) mir_core: &'a CoreBootstrapBridgeSectionV1,
    pub(crate) mir_ordinary: &'a CrossConeMirBridgeSectionV1,
    pub(crate) lir_foundation: &'a OdrFreeLirFoundation,

    pub(crate) manifest: &'a crate::BootstrapManifest,
    pub(crate) link_sections: Option<&'a crate::DecodedCrossConeLayoutLinkOnlySections>,
}

impl<'input> HirProductionValidatedCrossConeLayoutSections<'input> {
    pub(crate) fn prepare_mir_semantics(
        self,
        hir_aliases: scoop_hir::CanonicalTypeAliasExpansionsV1,
    ) -> Result<
        (
            PreparedCrossConeLayoutMirSections<'input>,
            DecodedCrossConeMirTypeBridgeSectionV1,
            DecodedCrossConeLayoutLirCandidates,
        ),
        CrossConeLayoutMirFrontValidationError,
    > {
        let Self {
            graph,
            view,
            mut identities,
            foundations,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
        } = self;
        let provider = graph.identity();
        let mir_core = mir_core_production
            .validate_against(provider, &mut identities, &foundations.mir)
            .map_err(CrossConeLayoutMirFrontValidationError::CoreProduction)?;
        validate_strong_profile_relations(
            provider,
            graph.kind(),
            &hir_core_production,
            &mir_core,
            &foundations.hir,
        )
        .map_err(CrossConeLayoutMirFrontValidationError::CrossLayer)?;
        let mir_ordinary = mir_cross_cone_bridge
            .validate(provider, &mut identities, &foundations.mir)
            .map_err(CrossConeLayoutMirFrontValidationError::OrdinaryBridge)?;
        Ok((
            PreparedCrossConeLayoutMirSections {
                graph,
                view,
                identities,
                foundations,
                hir_core: hir_core_production,
                hir_interface,
                hir_types: hir_type_semantics,
                hir_aliases,
                mir_core,
                mir_ordinary,
            },
            mir_type_bridge,
            DecodedCrossConeLayoutLirCandidates {
                strong: lir_strong_production,
                ordinary: lir_cross_cone_bridge,
                layout: lir_layout_abi,
            },
        ))
    }
}

impl PreparedCrossConeLayoutMirSections<'_> {
    pub(crate) fn direct_callables(&self) -> &scoop_mir::CrossConeMirBridgeSectionV1 {
        &self.mir_ordinary
    }

    pub(crate) fn link_sections(&self) -> Option<&crate::DecodedCrossConeLayoutLinkOnlySections> {
        self.view.link()
    }

    pub(crate) fn shared_metadata(&self) -> scoop_hir::SharedTypeMetadataV1<'_> {
        scoop_hir::SharedTypeMetadataV1 {
            provider: self.graph.identity(),
            identities: &self.identities,
            foundation: &self.foundations.hir,
            public: &self.hir_interface,
        }
    }

    pub(crate) fn provider(&self) -> scoop_identity::ConeIdentity {
        self.graph.identity()
    }

    pub(crate) fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.graph.coordinate()
    }

    pub(crate) fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub(crate) fn semantic_parts(&mut self) -> PreparedLayoutMirSemanticParts<'_> {
        let manifest = self.graph.envelope.manifest();
        PreparedLayoutMirSemanticParts {
            identities: &mut self.identities,
            hir_foundation: &self.foundations.hir,
            hir_core: &self.hir_core,
            hir_interface: &self.hir_interface,
            hir_types: &self.hir_types,
            mir_foundation: &self.foundations.mir,
            mir_core: &self.mir_core,
            mir_ordinary: &self.mir_ordinary,
            lir_foundation: &self.foundations.lir,

            manifest,
            link_sections: self.view.link(),
        }
    }
}

#[derive(Debug)]
pub enum CrossConeLayoutMirFrontValidationError {
    CoreProduction(MirProductionValidationError),
    CrossLayer(StrongProfileRelationError),
    OrdinaryBridge(CrossConeMirBridgeValidationError),
}

impl fmt::Display for CrossConeLayoutMirFrontValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreProduction(source) => {
                write!(formatter, "invalid legacy MIR production: {source}")
            }
            Self::CrossLayer(source) => {
                write!(formatter, "invalid HIR/MIR production relation: {source}")
            }
            Self::OrdinaryBridge(source) => {
                write!(formatter, "invalid ordinary MIR bridge: {source}")
            }
        }
    }
}

impl std::error::Error for CrossConeLayoutMirFrontValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CoreProduction(source) => Some(source),
            Self::CrossLayer(source) => Some(source),
            Self::OrdinaryBridge(source) => Some(source),
        }
    }
}
