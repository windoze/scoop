use super::*;
use crate::{HirDependencyTypeSiteV1, HirTypeSiteExactError, collect_type_site_nominals};

impl<A> ExternalReferenceAccumulator<'_, A> {
    pub(in crate::production::external_references) fn add_declaration_type_sites<E>(
        &mut self,
        output: &crate::DependencyHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<(), ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        use ExternalHirReferenceProductionError as Error;
        let module = output.output().local.module();
        let identities = &module.exact_type_identities;
        let path = WirePath::root();
        for site in super::super::declaration_types::collect(&output.output().local, meter)? {
            let owners = collect_type_site_nominals(
                site.exact(),
                |exact| {
                    identities
                        .type_for_identity(exact)
                        .and_then(|ty| identities.get(ty))
                        .map(|record| record.key())
                        .ok_or(Error::MissingExactType(exact))
                },
                meter,
            )
            .map_err(|error| match error {
                HirTypeSiteExactError::Identity(error) => error,
                HirTypeSiteExactError::Resource(error) => Error::Resource(error),
            })?;
            for owner in owners {
                let Some(pending) = self.observe_pending(
                    ExternalHirTargetV1::Nominal(owner),
                    ExternalHirReferenceRoleV1::ExecutableTypeDependency,
                )?
                else {
                    continue;
                };
                meter
                    .charge_owned_bytes(
                        std::mem::size_of::<HirDependencyTypeSiteV1>() as u64,
                        &path,
                    )
                    .map_err(Error::Resource)?;
                meter
                    .try_reserve_collection_slots(&mut pending.type_sites, 1, &path)
                    .map_err(Error::Resource)?;
                pending.type_sites.push(site.clone());
            }
        }
        Ok(())
    }
}
