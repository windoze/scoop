use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity, SourceDeclarationKey, ValidatedIdentityGraph};
use scoop_lir::{
    CoreLirBridgeBranchV1, CrossConeLayoutAbiSectionV1, CrossConeLirBridgeSectionV1,
    EntryProductionSourceV1, ExternalShapeLinkImportV1, LayoutAbiExportConstituentsV1,
    LayoutAbiSectionSourceAuthorityV1, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongExternalLirBridgeSurfaceV1, StrongInitializationDefinitionCatalogV2,
    StrongTypeReferenceDefinitionsV2, ValidatedLirTargetSelection,
    ValidatedStrongProductionSectionV2,
};

use crate::CheckedCrossConeLayoutMirProviderV1;

/// Independently reconstructed inputs needed to replay one complete Strong V2
/// section. Every field is required; a candidate section cannot fill gaps.
pub struct LayoutLirStrongReplayAuthorityV2 {
    external_bridges: StrongExternalLirBridgeSurfaceV1,
    digests: StrongDigestFinalizationPlanV1,
    entry: EntryProductionSourceV1,
    core_shape_sources: Vec<SourceDeclarationKey>,
    core_bridge: CoreLirBridgeBranchV1,
    type_definitions: StrongTypeReferenceDefinitionsV2,
    initialization_definitions: StrongInitializationDefinitionCatalogV2,
}

impl LayoutLirStrongReplayAuthorityV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        external_bridges: StrongExternalLirBridgeSurfaceV1,
        digests: StrongDigestFinalizationPlanV1,
        entry: EntryProductionSourceV1,
        core_shape_sources: Vec<SourceDeclarationKey>,
        core_bridge: CoreLirBridgeBranchV1,
        type_definitions: StrongTypeReferenceDefinitionsV2,
        initialization_definitions: StrongInitializationDefinitionCatalogV2,
    ) -> Self {
        Self {
            external_bridges,
            digests,
            entry,
            core_shape_sources,
            core_bridge,
            type_definitions,
            initialization_definitions,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        StrongExternalLirBridgeSurfaceV1,
        StrongDigestFinalizationPlanV1,
        EntryProductionSourceV1,
        Vec<SourceDeclarationKey>,
        CoreLirBridgeBranchV1,
        StrongTypeReferenceDefinitionsV2,
        StrongInitializationDefinitionCatalogV2,
    ) {
        (
            self.external_bridges,
            self.digests,
            self.entry,
            self.core_shape_sources,
            self.core_bridge,
            self.type_definitions,
            self.initialization_definitions,
        )
    }
}

/// Complete independent source bundle for one LIR candidate trio.
pub struct LayoutLirReplayAuthorityV1<'a, S> {
    provider: ConeIdentity,
    strong: LayoutLirStrongReplayAuthorityV2,
    exports: LayoutAbiExportConstituentsV1,
    physical_imports: Vec<ExternalShapeLinkImportV1<'a>>,
    source: S,
}

impl<'a, S> LayoutLirReplayAuthorityV1<'a, S> {
    pub fn new(
        provider: ConeIdentity,
        strong: LayoutLirStrongReplayAuthorityV2,
        exports: LayoutAbiExportConstituentsV1,
        physical_imports: Vec<ExternalShapeLinkImportV1<'a>>,
        source: S,
    ) -> Self {
        Self {
            provider,
            strong,
            exports,
            physical_imports,
            source,
        }
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ConeIdentity,
        LayoutLirStrongReplayAuthorityV2,
        LayoutAbiExportConstituentsV1,
        Vec<ExternalShapeLinkImportV1<'a>>,
        S,
    ) {
        (
            self.provider,
            self.strong,
            self.exports,
            self.physical_imports,
            self.source,
        )
    }
}

/// One checked dependency from the exact artifact graph.
#[derive(Clone, Copy)]
pub struct CheckedLayoutLirDependencyV1<'a> {
    pub(super) provider: &'a CheckedCrossConeLayoutLirProviderV1<'a>,
}

impl<'a> CheckedLayoutLirDependencyV1<'a> {
    pub const fn provider(self) -> ConeIdentity {
        self.provider.provider
    }

    pub const fn mir(self) -> &'a CheckedCrossConeLayoutMirProviderV1<'a> {
        self.provider.mir
    }

    pub const fn ordinary_bridge(self) -> &'a CrossConeLirBridgeSectionV1 {
        &self.provider.ordinary
    }

    pub const fn strong_production(self) -> &'a ValidatedStrongProductionSectionV2 {
        &self.provider.strong
    }

    pub const fn layout_abi(self) -> &'a CrossConeLayoutAbiSectionV1<'a> {
        &self.provider.layout
    }
}

/// Candidate-free context for reconstructing one LIR source authority.
pub struct LayoutLirSourceAuthorityContextV1<'a> {
    pub(super) provider: ConeIdentity,
    pub(super) coordinate: &'a ConeCoordinate,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) identities: &'a ValidatedIdentityGraph,
    pub(super) mir: &'a CheckedCrossConeLayoutMirProviderV1<'a>,
    pub(super) foundation: &'a OdrFreeLirFoundation,
    pub(super) ordinary: &'a CrossConeLirBridgeSectionV1,
    pub(super) direct: &'a [CheckedLayoutLirDependencyV1<'a>],
    pub(super) transitive: &'a [CheckedLayoutLirDependencyV1<'a>],
}

impl<'a> LayoutLirSourceAuthorityContextV1<'a> {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn coordinate(&self) -> &'a ConeCoordinate {
        self.coordinate
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub const fn identities(&self) -> &'a ValidatedIdentityGraph {
        self.identities
    }

    pub const fn mir(&self) -> &'a CheckedCrossConeLayoutMirProviderV1<'a> {
        self.mir
    }

    pub const fn lir_foundation(&self) -> &'a OdrFreeLirFoundation {
        self.foundation
    }

    pub const fn ordinary_bridge(&self) -> &'a CrossConeLirBridgeSectionV1 {
        self.ordinary
    }

    pub const fn direct_dependencies(&self) -> &'a [CheckedLayoutLirDependencyV1<'a>] {
        self.direct
    }

    pub const fn transitive_dependencies(&self) -> &'a [CheckedLayoutLirDependencyV1<'a>] {
        self.transitive
    }
}

/// Builds Strong/layout authority from independent reader state. The output
/// lifetime is tied to the factory, so it cannot retain the candidate or the
/// temporary dependency context.
pub trait LayoutLirSourceAuthorityFactoryV1 {
    type Error;
    type Source<'a>: LayoutAbiSectionSourceAuthorityV1<Self::Error>
    where
        Self: 'a;

    fn build<'factory>(
        &'factory mut self,
        context: LayoutLirSourceAuthorityContextV1<'_>,
    ) -> Result<LayoutLirReplayAuthorityV1<'factory, Self::Source<'factory>>, Self::Error>;
}

pub struct LayoutLirProviderSourceAuthorityV1<'a, F> {
    pub(super) provider: ConeIdentity,
    pub(super) factory: &'a mut F,
}

impl<'a, F> LayoutLirProviderSourceAuthorityV1<'a, F> {
    pub const fn new(provider: ConeIdentity, factory: &'a mut F) -> Self {
        Self { provider, factory }
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
}

/// Complete checked HIR, MIR, LIR and layout/ABI publication for one provider.
pub struct CheckedCrossConeLayoutLirProviderV1<'a> {
    pub(super) position: usize,
    pub(super) provider: ConeIdentity,
    pub(super) mir: &'a CheckedCrossConeLayoutMirProviderV1<'a>,
    pub(super) ordinary: CrossConeLirBridgeSectionV1,
    pub(super) strong: ValidatedStrongProductionSectionV2,
    pub(super) layout: CrossConeLayoutAbiSectionV1<'a>,
}

impl<'a> CheckedCrossConeLayoutLirProviderV1<'a> {
    pub const fn position(&self) -> usize {
        self.position
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn mir(&self) -> &'a CheckedCrossConeLayoutMirProviderV1<'a> {
        self.mir
    }

    pub const fn ordinary_bridge(&self) -> &CrossConeLirBridgeSectionV1 {
        &self.ordinary
    }

    pub const fn strong_production(&self) -> &ValidatedStrongProductionSectionV2 {
        &self.strong
    }

    pub const fn layout_abi(&self) -> &CrossConeLayoutAbiSectionV1<'a> {
        &self.layout
    }
}

/// Callback-scoped terminal LIR closure allocated from one typed arena.
pub struct CheckedCrossConeLayoutLirClosureV1<'a> {
    pub(super) current: ConeIdentity,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) providers: Vec<&'a CheckedCrossConeLayoutLirProviderV1<'a>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
}

impl<'a> CheckedCrossConeLayoutLirClosureV1<'a> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &'a CheckedCrossConeLayoutLirProviderV1<'a>> + '_ {
        self.providers.iter().copied()
    }

    pub fn provider(
        &self,
        provider: ConeIdentity,
    ) -> Option<&'a CheckedCrossConeLayoutLirProviderV1<'a>> {
        self.positions
            .get(&provider)
            .map(|position| self.providers[*position])
    }
}
