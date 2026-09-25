use crate::*;

#[derive(Clone, Copy, Debug)]
pub enum ShapeLinkProductionV1<'a> {
    Producer(&'a StrongProductionSectionV2),
    Reader(&'a ValidatedStrongProductionSectionV2),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum PhysicalProduction<'a> {
    Complete(ShapeLinkProductionV1<'a>),
    Replayed(ReplayedStrongLayoutExportsV2<'a>),
}

impl<'a> PhysicalProduction<'a> {
    pub(super) fn definitions(self) -> &'a StrongObjectSymbolSurfaceV1 {
        match self {
            Self::Complete(ShapeLinkProductionV1::Producer(section)) => {
                section.canonical_definitions()
            }
            Self::Complete(ShapeLinkProductionV1::Reader(section)) => {
                section.canonical_definitions()
            }
            Self::Replayed(view) => view.production.canonical_definitions(),
        }
    }
    pub(super) fn types(self) -> &'a StrongTypeRegistrationPlanSetV2 {
        match self {
            Self::Complete(ShapeLinkProductionV1::Producer(section)) => {
                section.registration_production().types()
            }
            Self::Complete(ShapeLinkProductionV1::Reader(section)) => section.type_registrations(),
            Self::Replayed(view) => view.production.type_registrations(),
        }
    }
    pub(super) fn callables(self) -> &'a StrongCallableRegistrationPlanSetV1 {
        match self {
            Self::Complete(ShapeLinkProductionV1::Producer(section)) => {
                section.registration_production().callables()
            }
            Self::Complete(ShapeLinkProductionV1::Reader(section)) => {
                section.callable_registrations()
            }
            Self::Replayed(view) => view.production.callable_registrations(),
        }
    }
    pub(super) fn storages(self) -> &'a StrongStaticStorageRegistrationPlanSetV1 {
        match self {
            Self::Complete(ShapeLinkProductionV1::Producer(section)) => {
                section.registration_production().static_storages()
            }
            Self::Complete(ShapeLinkProductionV1::Reader(section)) => {
                section.static_storage_registrations()
            }
            Self::Replayed(view) => view.production.static_storage_registrations(),
        }
    }
    pub(super) fn units(self) -> &'a StrongInitializationUnitRegistrationPlanSetV2 {
        match self {
            Self::Complete(ShapeLinkProductionV1::Producer(section)) => {
                section.registration_production().initialization_units()
            }
            Self::Complete(ShapeLinkProductionV1::Reader(section)) => {
                section.initialization_registrations()
            }
            Self::Replayed(view) => view.production.initialization_registrations(),
        }
    }
    pub(super) fn initialization_abi(self) -> Option<&'a CallableAbiRecordV1> {
        match self {
            Self::Complete(ShapeLinkProductionV1::Producer(section)) => {
                section.initialization_cycle_abi()
            }
            Self::Complete(ShapeLinkProductionV1::Reader(section)) => {
                section.initialization_cycle_abi()
            }
            Self::Replayed(view) => view.production.initialization_cycle_abi(),
        }
    }
}
