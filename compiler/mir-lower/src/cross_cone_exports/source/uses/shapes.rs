use super::*;
use hir::concrete::ExecutableExpressionVisitError;
use scoop_identity::{ExactTypeKey, SourceDeclarationKey};

pub(super) fn project(
    input: MirTypeBridgeExportInputV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<Vec<mir::MirTypeBridgeDependencyV1>, Error> {
    let module = input.hir.output().local.module();
    let path = WirePath::root();
    let mut actual = Vec::new();
    module
        .visit_executable_expressions(meter, |occurrence, meter| {
            for (role, ty) in occurrence.expression.type_uses() {
                meter.charge_work(1, &path)?;
                if !role.requires_shape_support() {
                    continue;
                }
                let exact = &module.exact_type_identities[ty];
                let ExactTypeKey::Nominal(owner) = exact.key() else {
                    continue;
                };
                meter.charge_work(
                    1 + u64::from(input.identities.identity_count().max(1).ilog2()),
                    &path,
                )?;
                let declaration = input
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(*owner)
                    .map_err(Error::Identity)?;
                if declaration.origin() == module.cone {
                    continue;
                }
                push(
                    &mut actual,
                    mir::MirTypeBridgeDependencyV1::new(
                        declaration.origin(),
                        mir::MirTypeBridgeTargetV1::ShapeSupport(*owner),
                    ),
                    meter,
                )?;
            }
            Ok(())
        })
        .map_err(|error| match error {
            ExecutableExpressionVisitError::Structure(error) => Error::ExecutableExpressions(error),
            ExecutableExpressionVisitError::Visitor(error) => error,
        })?;
    inventory::sort_cost(actual.len(), meter)?;
    actual.sort_unstable();
    actual.dedup();
    let shared = input
        .public
        .external_references()
        .materialized_shape_dependencies(module.cone, input.identities, meter)
        .map_err(|error| Error::SharedTypeOccurrences(Box::new(error)))?;
    meter.charge_work(actual.len() as u64 + shared.len() as u64, &path)?;
    if !actual
        .iter()
        .map(|usage| (usage.provider(), usage.target()))
        .eq(shared
            .into_iter()
            .map(|(provider, owner)| (provider, mir::MirTypeBridgeTargetV1::ShapeSupport(owner))))
    {
        return Err(Error::ShapeOccurrenceInventory);
    }
    Ok(actual)
}
