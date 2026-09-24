use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    CanonicalExactCallableAbiExportsV1, CanonicalExactDescriptorExportsV1,
    CanonicalExactDispatchExportsV1, CanonicalExactLayoutExportsV1,
    CanonicalParamFreeShapeSupportExportsV1, EntryProductionSourceV1, ExternalShapeLinkImportV1,
    LayoutAbiDependencyV1, LayoutAbiExportConstituentsV1, LayoutAbiSectionSourceAuthorityV1,
    StrongInitializationDefinitionCatalogV2, StrongTypeReferenceDefinitionsV2,
};
use scoop_wire::{BudgetMeter, DecodeLimits};

use crate::{
    LayoutLirProviderSourceAuthorityV1, LayoutLirReplayAuthorityV1,
    LayoutLirSourceAuthorityContextV1, LayoutLirSourceAuthorityFactoryV1,
    LayoutLirStrongReplayAuthorityV2,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LirSourceError {
    RejectedContext,
    RejectedExports,
    UnexpectedPhysicalImports,
}

impl fmt::Display for LirSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "LIR source rejected: {self:?}")
    }
}

impl std::error::Error for LirSourceError {}

pub(super) struct TestLirSource<'a> {
    exports: LayoutAbiExportConstituentsV1,
    roots: &'a [LayoutAbiDependencyV1],
    reject_exports: bool,
}

impl LayoutAbiSectionSourceAuthorityV1<LirSourceError> for TestLirSource<'_> {
    fn validate_local_exports(
        &self,
        exports: &LayoutAbiExportConstituentsV1,
        _meter: &mut BudgetMeter,
    ) -> Result<(), LirSourceError> {
        if self.reject_exports || exports != &self.exports {
            Err(LirSourceError::RejectedExports)
        } else {
            Ok(())
        }
    }

    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], LirSourceError> {
        Ok(self.roots)
    }

    fn validate_physical_imports(
        &self,
        imports: &[ExternalShapeLinkImportV1<'_>],
        _meter: &mut BudgetMeter,
    ) -> Result<(), LirSourceError> {
        if imports.is_empty() {
            Ok(())
        } else {
            Err(LirSourceError::UnexpectedPhysicalImports)
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct LirContextObservation {
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) transitive: Vec<ConeIdentity>,
    pub(super) direct_layout_addresses: Vec<usize>,
    pub(super) transitive_layout_addresses: Vec<usize>,
    pub(super) mir_addresses: Vec<usize>,
}

pub(super) struct RecordingLirFactory {
    expected: ConeIdentity,
    output_provider: ConeIdentity,
    reject_context: bool,
    reject_exports: bool,
    roots: Vec<LayoutAbiDependencyV1>,
    pub(super) builds: usize,
    pub(super) observation: LirContextObservation,
}

impl RecordingLirFactory {
    pub(super) fn new(provider: ConeIdentity) -> Self {
        Self {
            expected: provider,
            output_provider: provider,
            reject_context: false,
            reject_exports: false,
            roots: Vec::new(),
            builds: 0,
            observation: LirContextObservation::default(),
        }
    }

    pub(super) fn with_output_provider(expected: ConeIdentity, output: ConeIdentity) -> Self {
        Self {
            output_provider: output,
            ..Self::new(expected)
        }
    }

    pub(super) fn rejecting_source(provider: ConeIdentity) -> Self {
        Self {
            reject_exports: true,
            ..Self::new(provider)
        }
    }

    pub(super) fn with_roots(provider: ConeIdentity, roots: Vec<LayoutAbiDependencyV1>) -> Self {
        Self {
            roots,
            ..Self::new(provider)
        }
    }

    pub(super) fn authority(&mut self) -> LayoutLirProviderSourceAuthorityV1<'_, Self> {
        LayoutLirProviderSourceAuthorityV1::new(self.expected, self)
    }
}

impl LayoutLirSourceAuthorityFactoryV1 for RecordingLirFactory {
    type Error = LirSourceError;
    type Source<'a> = TestLirSource<'a>;

    fn build<'factory>(
        &'factory mut self,
        context: LayoutLirSourceAuthorityContextV1<'_>,
    ) -> Result<LayoutLirReplayAuthorityV1<'factory, Self::Source<'factory>>, Self::Error> {
        if self.reject_context || context.provider() != self.expected {
            return Err(LirSourceError::RejectedContext);
        }
        assert_eq!(context.mir().provider(), context.provider());
        assert_eq!(context.mir().hir().provider(), context.provider());
        assert_eq!(context.ordinary_bridge().artifact(), context.provider());
        let _ = context.identities().identity_count();
        let _ = context.lir_foundation().as_canonical();

        self.builds += 1;
        self.observation.direct = context
            .direct_dependencies()
            .iter()
            .map(|dependency| dependency.provider())
            .collect();
        self.observation.transitive = context
            .transitive_dependencies()
            .iter()
            .map(|dependency| dependency.provider())
            .collect();
        self.observation.direct_layout_addresses = context
            .direct_dependencies()
            .iter()
            .map(|dependency| dependency.layout_abi() as *const _ as usize)
            .collect();
        self.observation.transitive_layout_addresses = context
            .transitive_dependencies()
            .iter()
            .map(|dependency| dependency.layout_abi() as *const _ as usize)
            .collect();
        self.observation.mir_addresses = context
            .transitive_dependencies()
            .iter()
            .map(|dependency| dependency.mir() as *const _ as usize)
            .collect();

        let (_, production) = crate::link_decode::strong_production_fixture_for_test(
            context.coordinate().clone(),
            &self.observation.direct,
        );
        let mut meter = meter();
        let strong = LayoutLirStrongReplayAuthorityV2::new(
            production.external_bridges().clone(),
            production.digest_finalization_plan().clone(),
            EntryProductionSourceV1::Library,
            Vec::new(),
            production.initialization_cycle_abi().cloned().map(Box::new),
            StrongTypeReferenceDefinitionsV2::new(context.provider(), &[], &mut meter).unwrap(),
            StrongInitializationDefinitionCatalogV2::new(context.provider(), &[], &mut meter)
                .unwrap(),
        );
        let exports = empty_exports(&context);
        let source = TestLirSource {
            exports: exports.clone(),
            roots: &self.roots,
            reject_exports: self.reject_exports,
        };
        Ok(LayoutLirReplayAuthorityV1::new(
            self.output_provider,
            strong,
            exports,
            Vec::new(),
            source,
        ))
    }
}

pub(super) fn lir_authorities(
    factories: &mut [RecordingLirFactory],
) -> Vec<LayoutLirProviderSourceAuthorityV1<'_, RecordingLirFactory>> {
    factories
        .iter_mut()
        .map(RecordingLirFactory::authority)
        .collect()
}

fn empty_exports(context: &LayoutLirSourceAuthorityContextV1<'_>) -> LayoutAbiExportConstituentsV1 {
    let mut meter = meter();
    let target = context.target_selection().target();
    let foundation = context.lir_foundation();
    let layouts =
        CanonicalExactLayoutExportsV1::try_new(target, foundation, Vec::new(), &mut meter).unwrap();
    let descriptors =
        CanonicalExactDescriptorExportsV1::try_new(target, foundation, Vec::new(), &mut meter)
            .unwrap();
    let dispatch =
        CanonicalExactDispatchExportsV1::try_new(target, foundation, Vec::new(), &mut meter)
            .unwrap();
    let callables =
        CanonicalExactCallableAbiExportsV1::try_new(target, foundation, Vec::new(), &mut meter)
            .unwrap();
    let shape_support = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
        &mut meter,
    )
    .unwrap();
    LayoutAbiExportConstituentsV1::try_new(layouts, descriptors, dispatch, callables, shape_support)
        .unwrap()
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
