use std::collections::BTreeMap;

use scoop_hir::{
    CheckedCrossConeTypeSemanticsSectionV1, CheckedTypeSectionPublicSupportV1,
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    CrossConeHirInterfaceSemanticAuthority, OdrFreeHirFoundation,
};
use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_lir::ValidatedLirTargetSelection;

/// Read-only view of one already checked terminal public provider.
#[derive(Clone, Copy)]
pub struct CheckedLayoutHirPublicDependencyV1<'a> {
    pub(super) provider: ConeIdentity,
    pub(super) core: &'a CoreBootstrapInterfaceSectionV1,
    pub(super) interface: &'a CrossConeHirInterfaceSectionV1,
    pub(super) checked: CheckedTypeSectionPublicSupportV1<'a>,
}

impl<'a> CheckedLayoutHirPublicDependencyV1<'a> {
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }

    pub const fn core(self) -> &'a CoreBootstrapInterfaceSectionV1 {
        self.core
    }

    pub const fn interface(self) -> &'a CrossConeHirInterfaceSectionV1 {
        self.interface
    }

    pub const fn checked(self) -> CheckedTypeSectionPublicSupportV1<'a> {
        self.checked
    }
}

/// Exact artifact and graph inputs supplied when constructing one complete
/// ten-table public authority. Implementations must project semantic facts
/// independently; the candidate interface is exposed only as the value being
/// checked and as already checked terminal dependency views.
pub struct LayoutHirPublicAuthorityContextV1<'a> {
    pub(super) provider: ConeIdentity,
    pub(super) identities: &'a ValidatedIdentityGraph,
    pub(super) foundation: &'a OdrFreeHirFoundation,
    pub(super) core: &'a CoreBootstrapInterfaceSectionV1,
    pub(super) interface: &'a CrossConeHirInterfaceSectionV1,
    pub(super) direct_dependencies: &'a [ConeIdentity],
    pub(super) dependencies: &'a [CheckedLayoutHirPublicDependencyV1<'a>],
}

impl<'a> LayoutHirPublicAuthorityContextV1<'a> {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn identities(&self) -> &'a ValidatedIdentityGraph {
        self.identities
    }

    pub const fn foundation(&self) -> &'a OdrFreeHirFoundation {
        self.foundation
    }

    pub const fn core(&self) -> &'a CoreBootstrapInterfaceSectionV1 {
        self.core
    }

    pub const fn interface(&self) -> &'a CrossConeHirInterfaceSectionV1 {
        self.interface
    }

    pub const fn direct_dependencies(&self) -> &'a [ConeIdentity] {
        self.direct_dependencies
    }

    pub const fn dependencies(&self) -> &'a [CheckedLayoutHirPublicDependencyV1<'a>] {
        self.dependencies
    }
}

/// Constructs the independent source/route authority for the old complete
/// public section. The context is fixed by the validated artifact closure, so
/// a factory cannot substitute another provider graph or dependency set.
pub trait LayoutHirPublicAuthorityFactoryV1 {
    type Error;
    type Authority<'a>: CrossConeHirInterfaceSemanticAuthority<Self::Error>
    where
        Self: 'a;

    fn build<'a>(
        &'a mut self,
        context: LayoutHirPublicAuthorityContextV1<'a>,
    ) -> Result<Self::Authority<'a>, Self::Error>;
}

/// One provider's independent factory for the complete old public surface.
pub struct LayoutHirProviderPublicAuthorityV1<'a, P> {
    pub(super) provider: ConeIdentity,
    pub(super) factory: &'a mut P,
}

impl<'a, P> LayoutHirProviderPublicAuthorityV1<'a, P> {
    pub const fn new(provider: ConeIdentity, factory: &'a mut P) -> Self {
        Self { provider, factory }
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
}

/// Checked old ten-table surface retained in dependency-first order.
pub struct CheckedCrossConeLayoutHirPublicProviderV1<'a> {
    pub(super) position: usize,
    pub(super) provider: ConeIdentity,
    pub(super) core: &'a CoreBootstrapInterfaceSectionV1,
    pub(super) checked: CheckedTypeSectionPublicSupportV1<'a>,
}

impl<'a> CheckedCrossConeLayoutHirPublicProviderV1<'a> {
    pub const fn position(&self) -> usize {
        self.position
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn checked(&self) -> CheckedTypeSectionPublicSupportV1<'a> {
        self.checked
    }
}

/// Scoped complete-public closure. Its checked tokens borrow the provider
/// artifacts and therefore are intentionally available only to a callback.
pub struct CheckedCrossConeLayoutHirPublicClosureV1<'a> {
    pub(super) current: ConeIdentity,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) providers: Vec<&'a CheckedCrossConeLayoutHirPublicProviderV1<'a>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
}

impl<'a> CheckedCrossConeLayoutHirPublicClosureV1<'a> {
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
    ) -> impl ExactSizeIterator<Item = &'a CheckedCrossConeLayoutHirPublicProviderV1<'a>> + '_ {
        self.providers.iter().copied()
    }

    pub fn provider(
        &self,
        provider: ConeIdentity,
    ) -> Option<&'a CheckedCrossConeLayoutHirPublicProviderV1<'a>> {
        self.positions
            .get(&provider)
            .map(|position| self.providers[*position])
    }
}

/// One provider's independent semantic inputs. None of these values may be
/// projected from the candidate eight-field transport.
pub struct LayoutHirProviderSemanticAuthoritiesV1<'a, P, F, S, D, C> {
    pub(super) provider: ConeIdentity,
    pub(super) public: &'a mut P,
    pub(super) foundation: &'a F,
    pub(super) declarations: &'a mut S,
    pub(super) defaults: &'a mut D,
    pub(super) committed: &'a C,
}

impl<'a, P, F, S, D, C> LayoutHirProviderSemanticAuthoritiesV1<'a, P, F, S, D, C> {
    pub fn new(
        provider: ConeIdentity,
        public: &'a mut P,
        foundation: &'a F,
        declarations: &'a mut S,
        defaults: &'a mut D,
        committed: &'a C,
    ) -> Self {
        Self {
            provider,
            public,
            foundation,
            declarations,
            defaults,
            committed,
        }
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
}

/// Complete checked HIR publication for one provider in the explicit graph.
pub struct CheckedCrossConeLayoutHirProviderV1<'a> {
    pub(super) position: usize,
    pub(super) provider: ConeIdentity,
    pub(super) core: &'a CoreBootstrapInterfaceSectionV1,
    pub(super) public: CheckedTypeSectionPublicSupportV1<'a>,
    pub(super) types: CheckedCrossConeTypeSemanticsSectionV1<'a>,
}

impl<'a> CheckedCrossConeLayoutHirProviderV1<'a> {
    pub const fn position(&self) -> usize {
        self.position
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn public(&self) -> CheckedTypeSectionPublicSupportV1<'a> {
        self.public
    }

    pub const fn type_semantics(&self) -> &CheckedCrossConeTypeSemanticsSectionV1<'a> {
        &self.types
    }
}

/// Borrowed closure valid only while the typed arena and source authorities
/// remain alive. The callback API prevents these recursive proofs escaping.
pub struct CheckedCrossConeLayoutHirClosureV1<'a> {
    pub(super) current: ConeIdentity,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) providers: Vec<&'a CheckedCrossConeLayoutHirProviderV1<'a>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
}

impl<'a> CheckedCrossConeLayoutHirClosureV1<'a> {
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
    ) -> impl ExactSizeIterator<Item = &'a CheckedCrossConeLayoutHirProviderV1<'a>> + '_ {
        self.providers.iter().copied()
    }

    pub fn provider(
        &self,
        provider: ConeIdentity,
    ) -> Option<&'a CheckedCrossConeLayoutHirProviderV1<'a>> {
        self.positions
            .get(&provider)
            .map(|position| self.providers[*position])
    }
}
