use crate::*;

#[derive(Clone, Copy, Debug)]
pub enum ShapeLinkProductionV1<'a> {
    Producer(&'a StrongProductionSectionV2),
    Reader(&'a ValidatedStrongProductionSectionV2),
}

impl<'a> ShapeLinkProductionV1<'a> {
    pub(super) fn definitions(self) -> &'a StrongObjectSymbolSurfaceV1 {
        match self {
            Self::Producer(section) => section.canonical_definitions(),
            Self::Reader(section) => section.canonical_definitions(),
        }
    }
    pub(super) fn types(self) -> &'a StrongTypeRegistrationPlanSetV2 {
        match self {
            Self::Producer(section) => section.registration_production().types(),
            Self::Reader(section) => section.type_registrations(),
        }
    }
    pub(super) fn callables(self) -> &'a StrongCallableRegistrationPlanSetV1 {
        match self {
            Self::Producer(section) => section.registration_production().callables(),
            Self::Reader(section) => section.callable_registrations(),
        }
    }
    pub(super) fn storages(self) -> &'a StrongStaticStorageRegistrationPlanSetV1 {
        match self {
            Self::Producer(section) => section.registration_production().static_storages(),
            Self::Reader(section) => section.static_storage_registrations(),
        }
    }
    pub(super) fn units(self) -> &'a StrongInitializationUnitRegistrationPlanSetV2 {
        match self {
            Self::Producer(section) => section.registration_production().initialization_units(),
            Self::Reader(section) => section.initialization_registrations(),
        }
    }
    pub(super) fn initialization_abi(self) -> Option<&'a CallableAbiRecordV1> {
        match self {
            Self::Producer(section) => section.initialization_cycle_abi(),
            Self::Reader(section) => section.initialization_cycle_abi(),
        }
    }
}
