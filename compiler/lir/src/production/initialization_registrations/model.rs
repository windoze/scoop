use super::*;

pub type StrongInitializationUnitSemanticPlanV1 =
    StrongInitializationUnitSemanticPlan<PersistentInitializationUnitId>;
pub type StrongInitializationUnitSemanticPlanV2 =
    StrongInitializationUnitSemanticPlan<crate::StrongInitializationDependencyRefV2>;
pub type StrongInitializationUnitSemanticPlanSetV1 =
    StrongInitializationUnitSemanticPlanSet<PersistentInitializationUnitId>;
pub type StrongInitializationUnitSemanticPlanSetV2 =
    StrongInitializationUnitSemanticPlanSet<crate::StrongInitializationDependencyRefV2>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongInitializationSchedulePlanV1 {
    EagerStartup { gateway: PersistentCallableBodyId },
    LazyAccess,
}

impl StrongInitializationSchedulePlanV1 {
    pub const fn tag(self) -> u32 {
        match self {
            Self::EagerStartup { .. } => 1,
            Self::LazyAccess => 2,
        }
    }

    pub const fn gateway(self) -> Option<PersistentCallableBodyId> {
        match self {
            Self::EagerStartup { gateway } => Some(gateway),
            Self::LazyAccess => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitSemanticPlan<D> {
    pub(super) unit: PersistentInitializationUnitId,
    pub(super) diagnostic_path: String,
    pub(super) schedule: StrongInitializationSchedulePlanV1,
    pub(super) storage: PersistentStaticStorageId,
    pub(super) failure_root: PersistentStaticStorageId,
    pub(super) initializer: PersistentCallableBodyId,
    pub(super) ensure: PersistentCallableBodyId,
    pub(super) dependencies: Vec<D>,
}

impl<D> StrongInitializationUnitSemanticPlan<D> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_artifact(
        unit: PersistentInitializationUnitId,
        diagnostic_path: String,
        schedule: StrongInitializationSchedulePlanV1,
        storage: PersistentStaticStorageId,
        failure_root: PersistentStaticStorageId,
        initializer: PersistentCallableBodyId,
        ensure: PersistentCallableBodyId,
        dependencies: Vec<D>,
    ) -> Self {
        Self {
            unit,
            diagnostic_path,
            schedule,
            storage,
            failure_root,
            initializer,
            ensure,
            dependencies,
        }
    }

    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.unit
    }

    pub fn diagnostic_path(&self) -> &str {
        &self.diagnostic_path
    }

    pub const fn schedule(&self) -> StrongInitializationSchedulePlanV1 {
        self.schedule
    }

    pub const fn storage(&self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn failure_root(&self) -> PersistentStaticStorageId {
        self.failure_root
    }

    pub const fn initializer(&self) -> PersistentCallableBodyId {
        self.initializer
    }

    pub const fn ensure(&self) -> PersistentCallableBodyId {
        self.ensure
    }

    pub fn dependencies(&self) -> &[D] {
        &self.dependencies
    }
}

/// Proof that every final-LIR initialization unit has one complete semantic
/// plan and that no startup gateway exists outside the eager-unit set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitSemanticPlanSet<D> {
    pub(super) static_storages: StrongStaticStorageSemanticPlanSetV1,
    pub(super) units: Vec<StrongInitializationUnitSemanticPlan<D>>,
}

impl<D> StrongInitializationUnitSemanticPlanSet<D> {
    pub(crate) const fn from_artifact(
        static_storages: StrongStaticStorageSemanticPlanSetV1,
        units: Vec<StrongInitializationUnitSemanticPlan<D>>,
    ) -> Self {
        Self {
            static_storages,
            units,
        }
    }

    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.static_storages.producer()
    }

    pub const fn static_storages(&self) -> &StrongStaticStorageSemanticPlanSetV1 {
        &self.static_storages
    }

    pub fn units(&self) -> &[StrongInitializationUnitSemanticPlan<D>] {
        &self.units
    }
}
