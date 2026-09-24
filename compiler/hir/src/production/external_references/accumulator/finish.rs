use super::*;

impl<A> ExternalReferenceAccumulator<'_, A> {
    pub(in crate::production::external_references) fn finish<E>(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalExternalHirReferencesV1, ExternalHirReferenceProductionError<E>> {
        let mut records = Vec::with_capacity(self.references.len());
        for (target, pending) in self.references {
            for role in [
                ExternalHirReferenceRoleV1::AliasTarget,
                ExternalHirReferenceRoleV1::DefaultDependency,
                ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            ] {
                if pending.roles.contains(&role)
                    && role.requires_source_name_witness(target)
                    && !pending.witnessed_roles.contains(&role)
                {
                    return Err(ExternalHirReferenceProductionError::MissingWitnessUse {
                        target,
                        role,
                    });
                }
            }
            let roles =
                CanonicalExternalHirReferenceRolesV1::try_new(pending.roles.into_iter().collect())
                    .map_err(ExternalHirReferenceProductionError::Roles)?;
            let witnesses = CanonicalDependencyBindingWitnessesV1::try_new(
                pending.witnesses.into_iter().collect(),
            )
            .map_err(ExternalHirReferenceProductionError::Witnesses)?;
            let mut call_sites = Vec::new();
            meter
                .charge_owned_bytes(
                    (pending.call_sites.len()
                        * std::mem::size_of::<crate::HirDependencyCallSiteV1>())
                        as u64,
                    &WirePath::root(),
                )
                .map_err(ExternalHirReferenceProductionError::Resource)?;
            meter
                .try_reserve_collection_slots(
                    &mut call_sites,
                    pending.call_sites.len(),
                    &WirePath::root(),
                )
                .map_err(ExternalHirReferenceProductionError::Resource)?;
            for site in pending.call_sites {
                call_sites.push(site.finish(&witnesses, meter)?);
            }
            let count = call_sites.len() as u64;
            meter
                .charge_work(
                    count.saturating_mul(2 + u64::from(count.max(1).ilog2())),
                    &WirePath::root(),
                )
                .map_err(ExternalHirReferenceProductionError::Resource)?;
            let call_sites = crate::CanonicalHirDependencyCallSitesV1::try_new(call_sites)
                .map_err(ExternalHirReferenceProductionError::CallSite)?;
            records.push(
                ExternalHirReferenceV1::try_new(
                    pending.origin,
                    target,
                    roles,
                    witnesses,
                    call_sites,
                    crate::CanonicalHirDependencyTypeSitesV1::try_new(pending.type_sites, meter)
                        .map_err(|error| {
                            ExternalHirReferenceProductionError::TypeSite(Box::new(error))
                        })?,
                )
                .map_err(ExternalHirReferenceProductionError::Record)?,
            );
        }
        CanonicalExternalHirReferencesV1::try_new_metered(records, meter)
            .map_err(ExternalHirReferenceProductionError::Table)
    }
}
