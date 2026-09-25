use super::*;

pub(super) fn validate_call_sites(
    target: ExternalHirTargetV1,
    roles: &CanonicalExternalHirReferenceRolesV1,
    witnesses: &CanonicalDependencyBindingWitnessesV1,
    sites: &CanonicalHirDependencyCallSitesV1,
) -> Result<(), ExternalHirReferenceBuildError> {
    use scoop_identity::CallableTemplateOrigin;
    let selected = roles.contains(crate::ExternalHirReferenceRoleV1::ConcreteSelectedUse)
        && matches!(
            target,
            ExternalHirTargetV1::Callable(
                CallableTemplateOrigin::Function(_)
                    | CallableTemplateOrigin::Accessor(_)
                    | CallableTemplateOrigin::Constructor(_)
                    | CallableTemplateOrigin::VariantConstructor(_)
            )
        );
    let runtime = roles.contains(crate::ExternalHirReferenceRoleV1::RuntimeOperationDependency);
    if runtime
        && !matches!(
            target,
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(_))
                | ExternalHirTargetV1::GeneratedCallable(_)
        )
    {
        return Err(ExternalHirReferenceBuildError::RuntimeTarget);
    }
    match (selected || runtime, sites.is_empty()) {
        (true, true) => return Err(ExternalHirReferenceBuildError::MissingCallSites),
        (false, false) => return Err(ExternalHirReferenceBuildError::UnexpectedCallSites),
        _ => {}
    }
    let mut has_source_call = false;
    let mut has_runtime_call = false;
    for (site, record) in sites.records().iter().enumerate() {
        match record.reason() {
            crate::HirDependencyCallReasonV1::SourceBinding(_) if selected => {
                has_source_call = true
            }
            crate::HirDependencyCallReasonV1::CastFailure { .. } if runtime => {
                has_runtime_call = true
            }
            _ => return Err(ExternalHirReferenceBuildError::CallReason { site }),
        }
        for index in record.witness_indices() {
            if *index as usize >= witnesses.witnesses().len() {
                return Err(ExternalHirReferenceBuildError::CallWitnessIndex {
                    site,
                    index: *index,
                });
            }
        }
    }
    if (selected && !has_source_call) || (runtime && !has_runtime_call) {
        return Err(ExternalHirReferenceBuildError::MissingCallSites);
    }
    Ok(())
}
