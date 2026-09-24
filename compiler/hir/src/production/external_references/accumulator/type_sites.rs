use super::*;
use crate::concrete::ExecutableExpressionVisitError;
use crate::{HirDependencyTypeSiteV1, HirTypeSiteExactError, collect_type_site_nominals};

impl<A> ExternalReferenceAccumulator<'_, A> {
    pub(in crate::production::external_references) fn add_type_sites<E>(
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
        module
            .visit_executable_expressions(meter, |occurrence, meter| {
                for (role, ty) in occurrence.expression.type_uses() {
                    let exact = identities
                        .get(ty)
                        .ok_or(Error::ExpressionType {
                            position: occurrence.position,
                            ty,
                        })?
                        .id();
                    let owners = collect_type_site_nominals(
                        exact,
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
                        let target = ExternalHirTargetV1::Nominal(owner);
                        let Some(pending) = self.observe_pending(
                            target,
                            ExternalHirReferenceRoleV1::ExecutableTypeDependency,
                        )?
                        else {
                            continue;
                        };
                        let origin = super::super::origins::project(
                            output,
                            occurrence.expression.origin,
                            meter,
                        )?;
                        meter
                            .charge_owned_bytes(
                                (std::mem::size_of::<HirDependencyTypeSiteV1>()
                                    + std::mem::size_of::<crate::HirExpressionTypeSiteV1>())
                                    as u64,
                                &path,
                            )
                            .map_err(Error::Resource)?;
                        meter
                            .try_reserve_collection_slots(&mut pending.type_sites, 1, &path)
                            .map_err(Error::Resource)?;
                        pending.type_sites.push(HirDependencyTypeSiteV1::new(
                            occurrence.position,
                            origin,
                            role,
                            exact,
                        ));
                    }
                }
                Ok(())
            })
            .map_err(|error| match error {
                ExecutableExpressionVisitError::Structure(error) => Error::TypeOccurrences(error),
                ExecutableExpressionVisitError::Visitor(error) => error,
            })
    }
}
