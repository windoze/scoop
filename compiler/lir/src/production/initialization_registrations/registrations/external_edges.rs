use super::*;
use scoop_identity::ConeIdentity;
use scoop_wire::{BudgetMeter, WireError, WirePath};

impl StrongInitializationUnitRegistrationPlanSetV2 {
    /// Borrow the complete external edge inventory without dropping its provider.
    pub fn external_dependency_edges(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<
        impl Iterator<
            Item = (
                PersistentInitializationUnitId,
                ConeIdentity,
                PersistentInitializationUnitId,
            ),
        > + '_,
        WireError,
    > {
        for unit in self.registrations() {
            meter.charge_work(
                unit.semantic().dependencies().len() as u64 + 1,
                &WirePath::root(),
            )?;
        }
        Ok(self.registrations().iter().flat_map(|unit| {
            unit.semantic()
                .dependencies()
                .iter()
                .filter_map(|dependency| match dependency.kind() {
                    crate::StrongInitializationDependencyKindV2::LocalUnit(_) => None,
                    crate::StrongInitializationDependencyKindV2::DependencyExternalUnit {
                        provider,
                        unit_ref,
                    } => Some((unit.semantic().unit(), provider, unit_ref.unit())),
                })
        }))
    }
}
