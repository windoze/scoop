//! The existing production plan is the input to registration content calculation.

pub type InitializationRegistrationDataV1<D = scoop_identity::PersistentInitializationUnitId> =
    scoop_lir::StrongInitializationUnitRegistrationPlanSet<D>;
