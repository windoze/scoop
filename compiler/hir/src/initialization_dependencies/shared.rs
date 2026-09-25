use std::collections::BTreeMap;

use scoop_identity::CallableTemplateOrigin;

use super::*;
use crate::{ExternalHirTargetV1, SharedTypeMetadataV1};

impl SharedTypeMetadataV1<'_> {
    /// Replays the same direct property uses from the original shared HIR
    /// payloads. Neither MIR selections nor candidate use arrays are inputs.
    pub fn materialized_property_initialization_uses(
        self,
        dependencies: &[SharedTypeMetadataV1<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<Vec<HirPropertyInitializationUseV1>, Error> {
        let path = WirePath::root();
        let mut providers = BTreeMap::new();
        for dependency in dependencies {
            meter.charge_work(u64::from(providers.len().max(1).ilog2()) + 1, &path)?;
            meter.check_table_entries(providers.len() as u64 + 1, &path)?;
            meter.charge_collection_slots(1, &path)?;
            meter.charge_owned_bytes(
                std::mem::size_of::<(ConeIdentity, &SharedTypeMetadataV1<'_>)>() as u64,
                &path,
            )?;
            if dependency.provider == self.provider {
                return Err(Error::LocalProvider(self.provider));
            }
            if providers.insert(dependency.provider, dependency).is_some() {
                return Err(Error::DuplicateProvider(dependency.provider));
            }
        }
        let units = local_unit_ids(
            self.source_initialization_units()
                .iter()
                .map(|unit| unit.id()),
            meter,
        )?;
        let mut uses = Vec::new();
        for reference in self.public.external_references().records() {
            meter.charge_work(1, &path)?;
            let ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(accessor)) =
                reference.target()
            else {
                continue;
            };
            for site in reference.call_sites().records() {
                let Some(local_unit) =
                    initializer_root(site.position().root, self.identities, &units, meter)?
                else {
                    continue;
                };
                meter.charge_work(u64::from(providers.len().max(1).ilog2()) + 1, &path)?;
                let dependency = providers
                    .get(&reference.origin())
                    .ok_or(Error::MissingProvider(reference.origin()))?;
                let properties = dependency.public.property_interfaces();
                let dependency_units = dependency.source_initialization_units();
                meter.charge_work(
                    (properties.declaration_count() as u64)
                        .saturating_add(dependency_units.len() as u64),
                    &path,
                )?;
                let Some(dependency_unit) =
                    accessor_initialization_unit(accessor, properties, dependency_units)?
                else {
                    continue;
                };
                push(
                    &mut uses,
                    HirPropertyInitializationUseV1 {
                        local_unit,
                        provider: reference.origin(),
                        dependency_unit,
                        accessor,
                    },
                    self.provider,
                    meter,
                )?;
            }
        }
        canonicalize(uses, meter)
    }
}
