use std::collections::BTreeMap;

use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_lir::{OdrFreeLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::{
    CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, CrossConeMirTypeBridgeSectionV1,
    MirTypeBridgeSectionSourceAuthorityV1, OdrFreeMirFoundation,
};

use crate::CheckedCrossConeLayoutHirProviderV1;

/// One already checked dependency in the exact artifact graph. HIR and MIR
/// tokens refer to the same terminal provider instance.
#[derive(Clone, Copy)]
pub struct CheckedLayoutMirDependencyV1<'a> {
    pub(super) hir: &'a CheckedCrossConeLayoutHirProviderV1<'a>,
    pub(super) mir: &'a CheckedCrossConeLayoutMirProviderV1<'a>,
}

impl<'a> CheckedLayoutMirDependencyV1<'a> {
    pub const fn provider(self) -> ConeIdentity {
        self.mir.provider
    }

    pub const fn hir(self) -> &'a CheckedCrossConeLayoutHirProviderV1<'a> {
        self.hir
    }

    pub const fn core_production(self) -> &'a CoreBootstrapBridgeSectionV1 {
        self.mir.core
    }

    pub const fn ordinary_bridge(self) -> &'a CrossConeMirBridgeSectionV1 {
        self.mir.ordinary
    }

    pub const fn type_bridge(self) -> &'a CrossConeMirTypeBridgeSectionV1<'a> {
        &self.mir.bridge
    }
}

/// Exact reader context used to derive one independent MIR source authority.
/// The candidate seven-field transport is deliberately absent.
pub struct LayoutMirSourceAuthorityContextV1<'a> {
    pub(super) provider: ConeIdentity,
    pub(super) identities: &'a ValidatedIdentityGraph,
    pub(super) hir: &'a CheckedCrossConeLayoutHirProviderV1<'a>,
    pub(super) foundation: &'a OdrFreeMirFoundation,
    pub(super) core: &'a CoreBootstrapBridgeSectionV1,
    pub(super) ordinary: &'a CrossConeMirBridgeSectionV1,
    pub(super) direct: &'a [CheckedLayoutMirDependencyV1<'a>],
    pub(super) transitive: &'a [CheckedLayoutMirDependencyV1<'a>],
}

impl<'a> LayoutMirSourceAuthorityContextV1<'a> {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn identities(&self) -> &'a ValidatedIdentityGraph {
        self.identities
    }

    pub const fn hir(&self) -> &'a CheckedCrossConeLayoutHirProviderV1<'a> {
        self.hir
    }

    pub const fn mir_foundation(&self) -> &'a OdrFreeMirFoundation {
        self.foundation
    }

    pub const fn core_production(&self) -> &'a CoreBootstrapBridgeSectionV1 {
        self.core
    }

    pub const fn ordinary_bridge(&self) -> &'a CrossConeMirBridgeSectionV1 {
        self.ordinary
    }

    pub const fn direct_dependencies(&self) -> &'a [CheckedLayoutMirDependencyV1<'a>] {
        self.direct
    }

    pub const fn transitive_dependencies(&self) -> &'a [CheckedLayoutMirDependencyV1<'a>] {
        self.transitive
    }
}

/// Builds source authority only from independent reader inputs. The output
/// lifetime is tied to the factory, not to the context, so it cannot retain a
/// candidate graph or checked dependency view while identity resolution runs.
pub trait LayoutMirSourceAuthorityFactoryV1 {
    type Error;
    type Authority<'a>: MirTypeBridgeSectionSourceAuthorityV1<Self::Error>
    where
        Self: 'a;

    fn build<'factory>(
        &'factory mut self,
        context: LayoutMirSourceAuthorityContextV1<'_>,
    ) -> Result<Self::Authority<'factory>, Self::Error>;
}

pub struct LayoutMirProviderSourceAuthorityV1<'a, F> {
    pub(super) provider: ConeIdentity,
    pub(super) factory: &'a mut F,
}

impl<'a, F> LayoutMirProviderSourceAuthorityV1<'a, F> {
    pub const fn new(provider: ConeIdentity, factory: &'a mut F) -> Self {
        Self { provider, factory }
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
}

/// Complete checked HIR and MIR publication for one provider.
pub struct CheckedCrossConeLayoutMirProviderV1<'a> {
    pub(super) position: usize,
    pub(super) provider: ConeIdentity,
    pub(super) hir: &'a CheckedCrossConeLayoutHirProviderV1<'a>,
    pub(super) core: &'a CoreBootstrapBridgeSectionV1,
    pub(super) ordinary: &'a CrossConeMirBridgeSectionV1,
    pub(super) bridge: CrossConeMirTypeBridgeSectionV1<'a>,
    pub(super) lir_foundation: &'a OdrFreeLirFoundation,
}

impl<'a> CheckedCrossConeLayoutMirProviderV1<'a> {
    pub const fn position(&self) -> usize {
        self.position
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn hir(&self) -> &'a CheckedCrossConeLayoutHirProviderV1<'a> {
        self.hir
    }

    pub const fn core_production(&self) -> &'a CoreBootstrapBridgeSectionV1 {
        self.core
    }

    pub const fn ordinary_bridge(&self) -> &'a CrossConeMirBridgeSectionV1 {
        self.ordinary
    }

    pub const fn type_bridge(&self) -> &CrossConeMirTypeBridgeSectionV1<'a> {
        &self.bridge
    }

    pub const fn lir_foundation(&self) -> &'a OdrFreeLirFoundation {
        self.lir_foundation
    }
}

/// Callback-scoped closure whose recursive MIR terminal references all point
/// into one typed arena.
pub struct CheckedCrossConeLayoutMirClosureV1<'a> {
    pub(super) current: ConeIdentity,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) providers: Vec<&'a CheckedCrossConeLayoutMirProviderV1<'a>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
    pub(super) dependency_positions: Vec<Vec<usize>>,
}

pub(crate) struct CheckedCrossConeLayoutMirParts<'a> {
    pub(crate) current: ConeIdentity,
    pub(crate) target: ValidatedLirTargetSelection,
    pub(crate) direct: Vec<ConeIdentity>,
    pub(crate) providers: Vec<&'a CheckedCrossConeLayoutMirProviderV1<'a>>,
    pub(crate) positions: BTreeMap<ConeIdentity, usize>,
    pub(crate) dependency_positions: Vec<Vec<usize>>,
}

impl<'a> CheckedCrossConeLayoutMirClosureV1<'a> {
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
    ) -> impl ExactSizeIterator<Item = &'a CheckedCrossConeLayoutMirProviderV1<'a>> + '_ {
        self.providers.iter().copied()
    }

    pub fn provider(
        &self,
        provider: ConeIdentity,
    ) -> Option<&'a CheckedCrossConeLayoutMirProviderV1<'a>> {
        self.positions
            .get(&provider)
            .map(|position| self.providers[*position])
    }

    pub(crate) fn into_lir_parts(self) -> CheckedCrossConeLayoutMirParts<'a> {
        CheckedCrossConeLayoutMirParts {
            current: self.current,
            target: self.target,
            direct: self.direct,
            providers: self.providers,
            positions: self.positions,
            dependency_positions: self.dependency_positions,
        }
    }
}
