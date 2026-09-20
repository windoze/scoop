//! Read-only strong-production inputs shared by the legacy and layout paths.

use scoop_lir::{
    ConeImagePlanV1, EntryProductionPlanV1, StrongCallableRegistrationPlanSetV1,
    StrongDescriptorReference, StrongDigestFinalizationPlanV1, StrongExternalLirBridgeSurfaceV1,
    StrongImmortalObjectRegistrationPlanSetV1, StrongInitializationUnitRegistrationPlanSet,
    StrongObjectSymbolSurfaceV1, StrongProductionSection, StrongSafepointRegistrationPlanSetV1,
    StrongStaticStorageRegistrationPlanSetV1, StrongTypeRegistrationPlanSet,
    ValidatedStrongProductionSectionV2,
};

/// Non-serializable projection used while emitting LLVM and object bytes.
///
/// The layout path keeps its validated production token intact. Codegen only
/// borrows the exact tables it needs and therefore cannot publish a raw V2
/// section before the downstream code-fingerprint proof consumes that token.
pub(crate) struct StrongProductionEmissionView<'a, D, C, I> {
    external_bridges: &'a StrongExternalLirBridgeSurfaceV1,
    canonical_definitions: &'a StrongObjectSymbolSurfaceV1,
    digest_finalization_plan: &'a StrongDigestFinalizationPlanV1,
    safepoints: &'a StrongSafepointRegistrationPlanSetV1,
    callables: &'a StrongCallableRegistrationPlanSetV1,
    types: &'a StrongTypeRegistrationPlanSet<D, C>,
    immortal_objects: &'a StrongImmortalObjectRegistrationPlanSetV1,
    static_storages: &'a StrongStaticStorageRegistrationPlanSetV1,
    initialization_units: &'a StrongInitializationUnitRegistrationPlanSet<I>,
    image_plan: &'a ConeImagePlanV1,
    entry_plan: &'a EntryProductionPlanV1,
}

pub(crate) trait StrongProductionEmissionSource {
    type Descriptor: StrongDescriptorReference;
    type DispatchCallable: Clone;
    type InitializationDependency: Clone;

    fn emission_view(
        &self,
    ) -> StrongProductionEmissionView<
        '_,
        Self::Descriptor,
        Self::DispatchCallable,
        Self::InitializationDependency,
    >;
}

impl<D, C, I> StrongProductionEmissionSource for StrongProductionSection<D, C, I>
where
    D: StrongDescriptorReference,
    C: Clone,
    I: Clone,
{
    type Descriptor = D;
    type DispatchCallable = C;
    type InitializationDependency = I;

    fn emission_view(&self) -> StrongProductionEmissionView<'_, D, C, I> {
        let registrations = self.registration_production();
        StrongProductionEmissionView {
            external_bridges: self.external_bridges(),
            canonical_definitions: self.canonical_definitions(),
            digest_finalization_plan: self.digest_finalization_plan(),
            safepoints: registrations.safepoints(),
            callables: registrations.callables(),
            types: registrations.types(),
            immortal_objects: registrations.immortal_objects(),
            static_storages: registrations.static_storages(),
            initialization_units: registrations.initialization_units(),
            image_plan: self.image_plan(),
            entry_plan: self.entry_plan(),
        }
    }
}

impl StrongProductionEmissionSource for ValidatedStrongProductionSectionV2 {
    type Descriptor = scoop_lir::StrongTypeDescriptorRefV2;
    type DispatchCallable = scoop_lir::StrongTypeDispatchCallableRefV2;
    type InitializationDependency = scoop_lir::StrongInitializationDependencyRefV2;

    fn emission_view(
        &self,
    ) -> StrongProductionEmissionView<
        '_,
        Self::Descriptor,
        Self::DispatchCallable,
        Self::InitializationDependency,
    > {
        StrongProductionEmissionView {
            external_bridges: self.external_bridges(),
            canonical_definitions: self.canonical_definitions(),
            digest_finalization_plan: self.digest_finalization_plan(),
            safepoints: self.safepoint_registrations(),
            callables: self.callable_registrations(),
            types: self.type_registrations(),
            immortal_objects: self.immortal_registrations(),
            static_storages: self.static_storage_registrations(),
            initialization_units: self.initialization_registrations(),
            image_plan: self.image_plan(),
            entry_plan: self.entry_plan(),
        }
    }
}

impl<'a, D, C, I> StrongProductionEmissionView<'a, D, C, I> {
    pub(crate) const fn external_bridges(&self) -> &StrongExternalLirBridgeSurfaceV1 {
        self.external_bridges
    }

    pub(crate) const fn canonical_definitions(&self) -> &StrongObjectSymbolSurfaceV1 {
        self.canonical_definitions
    }

    pub(crate) const fn digest_finalization_plan(&self) -> &StrongDigestFinalizationPlanV1 {
        self.digest_finalization_plan
    }

    pub(crate) const fn safepoints(&self) -> &StrongSafepointRegistrationPlanSetV1 {
        self.safepoints
    }

    pub(crate) const fn callables(&self) -> &StrongCallableRegistrationPlanSetV1 {
        self.callables
    }

    pub(crate) const fn types(&self) -> &StrongTypeRegistrationPlanSet<D, C> {
        self.types
    }

    pub(crate) const fn immortal_objects(&self) -> &StrongImmortalObjectRegistrationPlanSetV1 {
        self.immortal_objects
    }

    pub(crate) const fn static_storages(&self) -> &StrongStaticStorageRegistrationPlanSetV1 {
        self.static_storages
    }

    pub(crate) const fn initialization_units(
        &self,
    ) -> &StrongInitializationUnitRegistrationPlanSet<I> {
        self.initialization_units
    }

    pub(crate) const fn image_plan(&self) -> &ConeImagePlanV1 {
        self.image_plan
    }

    pub(crate) const fn entry_plan(&self) -> &EntryProductionPlanV1 {
        self.entry_plan
    }
}
