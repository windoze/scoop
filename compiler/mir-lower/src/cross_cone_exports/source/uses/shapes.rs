use super::*;
use hir::concrete::ExecutableExpressionVisitError;
use scoop_identity::{ExactTypeKey, SourceDeclarationKey};

pub(super) fn project(
    input: MirTypeBridgeExportInputV1<'_>,
) -> Result<Vec<mir::MirTypeBridgeDependencyV1>, Error> {
    let module = input.hir.output().local.module();

    let mut actual = Vec::new();
    module
        .visit_executable_expressions(|occurrence| {
            for (role, ty) in occurrence.expression.type_uses() {
                if !role.requires_shape_support() {
                    continue;
                }
                let exact = &module.exact_type_identities[ty];
                let ExactTypeKey::Nominal(owner) = exact.key() else {
                    continue;
                };

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
                )?;
            }
            Ok(())
        })
        .map_err(|error| match error {
            ExecutableExpressionVisitError::Structure(error) => Error::ExecutableExpressions(error),
            ExecutableExpressionVisitError::Visitor(error) => error,
        })?;

    actual.sort_unstable();
    actual.dedup();
    let shared = input
        .public
        .external_references()
        .materialized_shape_dependencies(module.cone, input.identities)
        .map_err(|error| Error::SharedTypeOccurrences(Box::new(error)))?;

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
