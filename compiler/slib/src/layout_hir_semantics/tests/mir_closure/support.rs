use std::fmt;

use scoop_identity::{
    ConeIdentity, InitializationCallableRole, PersistentExactTypeId,
    PersistentInitializationUnitId, PersistentObjectValueId, PersistentTypeId,
    StrongCallableDefinitionOwner,
};
use scoop_mir::{
    CanonicalMirExternalInitializationUsesV1, MirBridgeCallableSignatureV1,
    MirTypeBridgeDependencyV1, MirTypeBridgeSectionSourceAuthorityV1,
    MirTypeBridgeSourceSemanticAuthorityV1, ParamFreeMirCallableBindingV1,
    ParamFreeMirDispatchSchemaV1, ParamFreeMirObjectValueV1, ParamFreeMirTypeExportV1,
};
use scoop_wire::{BudgetMeter, DecodeLimits};

use super::super::{
    EmptyCommittedUses, EmptyDeclarations, EmptyDefaults, EmptyFoundation, RecordingPublicFactory,
    type_authority::NominalFixture,
};
use crate::{
    LayoutHirProviderSemanticAuthoritiesV1, LayoutMirProviderSourceAuthorityV1,
    LayoutMirSourceAuthorityContextV1, LayoutMirSourceAuthorityFactoryV1,
};

pub(super) struct HirAuthorityBundle {
    provider: ConeIdentity,
    public: RecordingPublicFactory,
    foundation: EmptyFoundation,
    declarations: EmptyDeclarations,
    defaults: EmptyDefaults,
    committed: EmptyCommittedUses,
}

impl HirAuthorityBundle {
    pub(super) fn empty(provider: ConeIdentity) -> Self {
        Self {
            provider,
            public: RecordingPublicFactory::new(provider),
            foundation: EmptyFoundation::new(provider),
            declarations: EmptyDeclarations::new(),
            defaults: EmptyDefaults::new(provider),
            committed: EmptyCommittedUses::default(),
        }
    }

    pub(super) fn nominal(nominal: &NominalFixture) -> Self {
        Self {
            provider: nominal.provider(),
            public: RecordingPublicFactory::new(nominal.provider()),
            foundation: EmptyFoundation::with_nominal(nominal),
            declarations: EmptyDeclarations::with_nominal(nominal),
            defaults: EmptyDefaults::with_nominal(nominal),
            committed: EmptyCommittedUses::default(),
        }
    }

    pub(super) fn dependency(provider: ConeIdentity, nominal: &NominalFixture) -> Self {
        Self {
            provider,
            public: RecordingPublicFactory::new(provider),
            foundation: EmptyFoundation::with_dependency(provider, nominal),
            declarations: EmptyDeclarations::new(),
            defaults: EmptyDefaults::new(provider),
            committed: EmptyCommittedUses::default(),
        }
    }

    pub(super) fn authority(
        &mut self,
    ) -> LayoutHirProviderSemanticAuthoritiesV1<
        '_,
        RecordingPublicFactory,
        EmptyFoundation,
        EmptyDeclarations,
        EmptyDefaults,
        EmptyCommittedUses,
    > {
        LayoutHirProviderSemanticAuthoritiesV1::new(
            self.provider,
            &mut self.public,
            &self.foundation,
            &mut self.declarations,
            &mut self.defaults,
            &self.committed,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MirSourceError {
    UnexpectedCall,
    Rejected,
}

impl fmt::Display for MirSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedCall => formatter.write_str("unexpected MIR source query"),
            Self::Rejected => formatter.write_str("MIR source factory rejected the context"),
        }
    }
}

impl std::error::Error for MirSourceError {}

pub(super) struct EmptyMirSource {
    provider: ConeIdentity,
    types: Vec<ParamFreeMirTypeExportV1>,
    required_types: Vec<PersistentExactTypeId>,
    initialization_uses: CanonicalMirExternalInitializationUsesV1,
    committed_uses: Vec<MirTypeBridgeDependencyV1>,
}

impl EmptyMirSource {
    fn new(provider: ConeIdentity) -> Self {
        Self {
            provider,
            types: Vec::new(),
            required_types: Vec::new(),
            initialization_uses: CanonicalMirExternalInitializationUsesV1::try_new(
                Vec::new(),
                &mut BudgetMeter::new(DecodeLimits::default()),
            )
            .unwrap(),
            committed_uses: Vec::new(),
        }
    }
}

impl MirTypeBridgeSourceSemanticAuthorityV1<MirSourceError> for &EmptyMirSource {
    fn provider(&self) -> ConeIdentity {
        self.provider
    }

    fn required_types(&self) -> Result<&[PersistentExactTypeId], MirSourceError> {
        Ok(&self.required_types)
    }

    fn required_callables(&self) -> Result<&[StrongCallableDefinitionOwner], MirSourceError> {
        Ok(&[])
    }

    fn required_dispatch(&self) -> Result<&[PersistentExactTypeId], MirSourceError> {
        Ok(&[])
    }

    fn required_objects(&self) -> Result<&[PersistentObjectValueId], MirSourceError> {
        Ok(&[])
    }

    fn required_source_roots(&self) -> Result<&[PersistentTypeId], MirSourceError> {
        Ok(&[])
    }

    fn type_source(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirTypeExportV1, MirSourceError> {
        self.types
            .iter()
            .find(|record| record.exact() == exact)
            .ok_or(MirSourceError::UnexpectedCall)
    }

    fn callable_source(
        &self,
        _target: StrongCallableDefinitionOwner,
    ) -> Result<&ParamFreeMirCallableBindingV1, MirSourceError> {
        Err(MirSourceError::UnexpectedCall)
    }

    fn dispatch_source(
        &self,
        _owner: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirDispatchSchemaV1, MirSourceError> {
        Err(MirSourceError::UnexpectedCall)
    }

    fn object_source(
        &self,
        _value: PersistentObjectValueId,
    ) -> Result<&ParamFreeMirObjectValueV1, MirSourceError> {
        Err(MirSourceError::UnexpectedCall)
    }

    fn committed_initialization_uses(
        &self,
    ) -> Result<&CanonicalMirExternalInitializationUsesV1, MirSourceError> {
        Ok(&self.initialization_uses)
    }
}

impl MirTypeBridgeSectionSourceAuthorityV1<MirSourceError> for &EmptyMirSource {
    fn committed_external_uses(&self) -> Result<&[MirTypeBridgeDependencyV1], MirSourceError> {
        Ok(&self.committed_uses)
    }

    fn local_initialization_units(
        &self,
    ) -> Result<&[PersistentInitializationUnitId], MirSourceError> {
        Ok(&[])
    }

    fn initialization_signature(
        &self,
        _unit: PersistentInitializationUnitId,
        _role: InitializationCallableRole,
    ) -> Result<&MirBridgeCallableSignatureV1, MirSourceError> {
        Err(MirSourceError::UnexpectedCall)
    }
}

#[derive(Debug, Default)]
pub(super) struct MirContextObservation {
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) transitive: Vec<ConeIdentity>,
    pub(super) direct_bridge_addresses: Vec<usize>,
    pub(super) transitive_bridge_addresses: Vec<usize>,
    pub(super) hir_type_addresses: Vec<usize>,
}

pub(super) struct RecordingMirFactory {
    expected: ConeIdentity,
    source: EmptyMirSource,
    reject: bool,
    pub(super) builds: usize,
    pub(super) observation: MirContextObservation,
}

impl RecordingMirFactory {
    pub(super) fn new(provider: ConeIdentity) -> Self {
        Self {
            expected: provider,
            source: EmptyMirSource::new(provider),
            reject: false,
            builds: 0,
            observation: MirContextObservation::default(),
        }
    }

    pub(super) fn with_source_provider(expected: ConeIdentity, source: ConeIdentity) -> Self {
        Self {
            expected,
            source: EmptyMirSource::new(source),
            reject: false,
            builds: 0,
            observation: MirContextObservation::default(),
        }
    }

    pub(super) fn with_committed_use(
        provider: ConeIdentity,
        dependency: MirTypeBridgeDependencyV1,
    ) -> Self {
        let mut factory = Self::new(provider);
        factory.source.committed_uses.push(dependency);
        factory
    }

    pub(super) fn with_type(provider: ConeIdentity, record: ParamFreeMirTypeExportV1) -> Self {
        let mut factory = Self::new(provider);
        factory.source.required_types.push(record.exact());
        factory.source.types.push(record);
        factory
    }

    pub(super) fn authority(&mut self) -> LayoutMirProviderSourceAuthorityV1<'_, Self> {
        LayoutMirProviderSourceAuthorityV1::new(self.expected, self)
    }
}

impl LayoutMirSourceAuthorityFactoryV1 for RecordingMirFactory {
    type Error = MirSourceError;
    type Authority<'a> = &'a EmptyMirSource;

    fn build<'factory>(
        &'factory mut self,
        context: LayoutMirSourceAuthorityContextV1<'_>,
    ) -> Result<Self::Authority<'factory>, Self::Error> {
        if self.reject || context.provider() != self.expected {
            return Err(MirSourceError::Rejected);
        }
        assert_eq!(context.hir().provider(), context.provider());
        assert_eq!(context.hir().public().provider(), context.provider());
        assert_eq!(
            context.hir().type_semantics().provider(),
            context.provider()
        );
        let _ = context.core_production().entry_bridge();
        assert_eq!(context.ordinary_bridge().artifact(), context.provider());
        let _ = context.identities().identity_count();
        let _ = context.mir_foundation().as_canonical();
        for dependency in context
            .direct_dependencies()
            .iter()
            .chain(context.transitive_dependencies())
        {
            assert_eq!(dependency.hir().provider(), dependency.provider());
            let _ = dependency.core_production().entry_bridge();
            assert_eq!(
                dependency.ordinary_bridge().artifact(),
                dependency.provider()
            );
        }
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
        self.observation.direct_bridge_addresses = context
            .direct_dependencies()
            .iter()
            .map(|dependency| dependency.type_bridge() as *const _ as usize)
            .collect();
        self.observation.transitive_bridge_addresses = context
            .transitive_dependencies()
            .iter()
            .map(|dependency| dependency.type_bridge() as *const _ as usize)
            .collect();
        self.observation.hir_type_addresses = context
            .transitive_dependencies()
            .iter()
            .map(|dependency| dependency.hir().type_semantics() as *const _ as usize)
            .collect();
        Ok(&self.source)
    }
}

pub(super) fn hir_authorities(
    bundles: &mut [HirAuthorityBundle],
) -> Vec<
    LayoutHirProviderSemanticAuthoritiesV1<
        '_,
        RecordingPublicFactory,
        EmptyFoundation,
        EmptyDeclarations,
        EmptyDefaults,
        EmptyCommittedUses,
    >,
> {
    bundles
        .iter_mut()
        .map(HirAuthorityBundle::authority)
        .collect()
}

pub(super) fn mir_authorities(
    factories: &mut [RecordingMirFactory],
) -> Vec<LayoutMirProviderSourceAuthorityV1<'_, RecordingMirFactory>> {
    factories
        .iter_mut()
        .map(RecordingMirFactory::authority)
        .collect()
}
