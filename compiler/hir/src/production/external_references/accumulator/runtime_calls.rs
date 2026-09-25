//! Implicit calls are projected only from their actual executable operation.

use super::*;
use crate::concrete::{ConcreteCoreProtocols, ExecutableExpressionVisitError, ExprKind};
use crate::{
    HirDependencyCallReasonV1, HirDependencyCallSiteV1, ImportedCoreProtocolCallableDefinition,
};
use scoop_identity::{CallableTemplateOrigin, ExactTypeKey, PersistentExactTypeId};

impl<A> ExternalReferenceAccumulator<'_, A> {
    pub(in crate::production::external_references) fn add_runtime_call_sites<E>(
        &mut self,
        output: &crate::DependencyHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<(), ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        use ExternalHirReferenceProductionError as Error;
        let module = output.output().local.module();
        let ConcreteCoreProtocols::Imported(protocols) = &module.core_protocols else {
            return Ok(());
        };
        let exceptions = protocols.exceptions();
        let callable = exceptions.class_cast_exception_constructor();
        let target = match callable.definition() {
            ImportedCoreProtocolCallableDefinition::Constructor(id) => {
                ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(id.persistent()))
            }
            ImportedCoreProtocolCallableDefinition::GeneratedCallable(id) => {
                ExternalHirTargetV1::GeneratedCallable(id.persistent())
            }
            definition => return Err(Error::RuntimeConstructor(definition)),
        };
        let result_key = ExactTypeKey::Nominal(exceptions.class_cast_exception().persistent());
        let path = WirePath::root();
        let role = ExternalHirReferenceRoleV1::RuntimeOperationDependency;
        module
            .visit_executable_expressions(meter, |occurrence, meter| {
                let ExprKind::Cast {
                    check_ty,
                    optional: false,
                    ..
                } = &occurrence.expression.kind
                else {
                    return Ok(());
                };
                let checked_type = module
                    .exact_type_identities
                    .get(*check_ty)
                    .ok_or(Error::ExpressionType {
                        position: occurrence.position,
                        ty: *check_ty,
                    })?
                    .id();
                let hash_size = PersistentExactTypeId::hash_stream_length(&result_key)
                    .map_err(|error| Error::RuntimeExact(error.to_string()))?;
                meter
                    .charge_sha256(hash_size, &path)
                    .map_err(Error::Resource)?;
                let result = PersistentExactTypeId::from_key(&result_key)
                    .map_err(|error| Error::RuntimeExact(error.to_string()))?;
                let origin =
                    super::super::origins::project(output, occurrence.expression.origin, meter)?;
                let site = HirDependencyCallSiteV1::try_new_with_reason(
                    occurrence.position,
                    origin,
                    Vec::new(),
                    result,
                    HirDependencyCallReasonV1::CastFailure { checked_type },
                )
                .map_err(Error::CallSite)?;
                let Some(pending) = self.observe_pending(target, role)? else {
                    return Ok(());
                };
                meter
                    .charge_owned_bytes(
                        std::mem::size_of::<super::super::calls::PendingCallSite<'_>>() as u64,
                        &path,
                    )
                    .map_err(Error::Resource)?;
                meter
                    .try_reserve_collection_slots(&mut pending.call_sites, 1, &path)
                    .map_err(Error::Resource)?;
                pending
                    .call_sites
                    .push(super::super::calls::PendingCallSite::Runtime(site));
                Ok(())
            })
            .map_err(|error| match error {
                ExecutableExpressionVisitError::Structure(error) => Error::TypeOccurrences(error),
                ExecutableExpressionVisitError::Visitor(error) => error,
            })
    }
}
