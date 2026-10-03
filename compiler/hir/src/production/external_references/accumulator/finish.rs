use super::*;

impl<A> ExternalReferenceAccumulator<'_, A> {
    pub(in crate::production::external_references) fn finish<E>(
        self,
    ) -> Result<CanonicalExternalHirReferencesV1, ExternalHirReferenceProductionError<E>> {
        let mut records = Vec::with_capacity(self.references.len());
        for (target, pending) in self.references {
            for role in [
                ExternalHirReferenceRoleV1::DefaultDependency,
                ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            ] {
                if pending.roles.contains(&role)
                    && !(role == ExternalHirReferenceRoleV1::ConcreteSelectedUse
                        && !pending.call_sites.is_empty())
                    && role.requires_source_name_witness()
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

            scoop_wire::allocation::try_reserve(
                &mut call_sites,
                pending.call_sites.len(),
                &WirePath::root(),
            )
            .map_err(ExternalHirReferenceProductionError::Resource)?;
            for site in pending.call_sites {
                call_sites.push(site.finish(&witnesses)?);
            }

            let call_sites = crate::CanonicalHirDependencyCallSitesV1::try_new(call_sites)
                .map_err(ExternalHirReferenceProductionError::CallSite)?;
            records.push(
                ExternalHirReferenceV1::try_new(
                    pending.origin,
                    target,
                    roles,
                    witnesses,
                    call_sites,
                    crate::CanonicalHirDependencyTypeSitesV1::try_new(pending.type_sites).map_err(
                        |error| ExternalHirReferenceProductionError::TypeSite(Box::new(error)),
                    )?,
                )
                .map_err(ExternalHirReferenceProductionError::Record)?,
            );
        }
        CanonicalExternalHirReferencesV1::try_new(records)
            .map_err(ExternalHirReferenceProductionError::Table)
    }
}
