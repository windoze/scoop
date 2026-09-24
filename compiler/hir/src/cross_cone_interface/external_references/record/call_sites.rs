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
                CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::Accessor(_)
            )
        );
    match (selected, sites.is_empty()) {
        (true, true) => return Err(ExternalHirReferenceBuildError::MissingCallSites),
        (false, false) => return Err(ExternalHirReferenceBuildError::UnexpectedCallSites),
        _ => {}
    }
    for (site, record) in sites.records().iter().enumerate() {
        for index in record.witness_indices() {
            if *index as usize >= witnesses.witnesses().len() {
                return Err(ExternalHirReferenceBuildError::CallWitnessIndex {
                    site,
                    index: *index,
                });
            }
        }
    }
    Ok(())
}
