//! Versioned complete Strong registration surfaces.

use super::*;

pub type StrongRegistrationProductionSurfaceV1 = StrongRegistrationProductionSurface<
    crate::StrongTypeDescriptorRefV1,
    crate::StrongTypeDispatchCallableRefV1,
    scoop_identity::PersistentInitializationUnitId,
>;
pub type StrongRegistrationProductionSurfaceV2 = StrongRegistrationProductionSurface<
    crate::StrongTypeDescriptorRefV2,
    crate::StrongTypeDispatchCallableRefV2,
    crate::StrongInitializationDependencyRefV2,
>;

/// Complete member-independent registration authority produced from one final
/// LIR module. Every table has already been checked against the same foundation
/// and digest graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongRegistrationProductionSurface<D, C, I> {
    pub(super) identities: RegistrationIdentitySurfaceV1,
    pub(super) safepoints: StrongSafepointRegistrationPlanSetV1,
    pub(super) callables: StrongCallableRegistrationPlanSetV1,
    pub(super) types: crate::StrongTypeRegistrationPlanSet<D, C>,
    pub(super) immortal_objects: StrongImmortalObjectRegistrationPlanSetV1,
    pub(super) static_storages: StrongStaticStorageRegistrationPlanSetV1,
    pub(super) initialization_units: crate::StrongInitializationUnitRegistrationPlanSet<I>,
}

impl<D, C, I> StrongRegistrationProductionSurface<D, C, I> {
    pub(crate) fn set_emitted_root_counts(
        &mut self,
        counts: &std::collections::BTreeMap<crate::SafepointId, u32>,
    ) -> Result<(), StrongSafepointRegistrationPlanBuildError> {
        self.safepoints.set_emitted_root_counts(counts)?;
        let retained = self
            .safepoints
            .registrations()
            .iter()
            .map(|plan| plan.site())
            .collect();
        self.identities.retain_safepoints(&retained);
        Ok(())
    }

    pub const fn identities(&self) -> &RegistrationIdentitySurfaceV1 {
        &self.identities
    }

    pub const fn safepoints(&self) -> &StrongSafepointRegistrationPlanSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &StrongCallableRegistrationPlanSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &crate::StrongTypeRegistrationPlanSet<D, C> {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &StrongImmortalObjectRegistrationPlanSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &StrongStaticStorageRegistrationPlanSetV1 {
        &self.static_storages
    }

    pub const fn initialization_units(
        &self,
    ) -> &crate::StrongInitializationUnitRegistrationPlanSet<I> {
        &self.initialization_units
    }

    pub fn safepoint_semantics(&self) -> StrongSafepointSemanticPlanSetV1 {
        StrongSafepointSemanticPlanSetV1::from_artifact(
            self.safepoints.producer(),
            self.safepoints
                .registrations()
                .iter()
                .map(|plan| {
                    StrongSafepointSemanticPlanV1::from_artifact(
                        plan.site(),
                        plan.safepoint(),
                        plan.owner(),
                        plan.role(),
                        plan.root_pair_count(),
                    )
                })
                .collect(),
        )
    }
}
