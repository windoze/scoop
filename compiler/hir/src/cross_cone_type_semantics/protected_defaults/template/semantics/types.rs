use super::*;
use crate::SignatureBinderScopeV1;
use scoop_identity::LocalValueSelector;
use scoop_wire::WirePath;

pub(super) fn validate<A: NominalInterfaceShapeAuthority<E>, E>(
    template: &ProtectedDefaultTemplateV1,
    provider: DefaultTemplateProviderShapeV1,
    owner_scope: &SignatureBinderScopeV1,
    authority: &mut A,
    meter: &mut BudgetMeter,
) -> Result<(), ProtectedDefaultTemplateContractSemanticError<E>> {
    use ProtectedDefaultTemplateContractSemanticError as Error;
    let path = WirePath::root();
    if template.type_parameters().len_u32() != provider.binder_arity() {
        return Err(Error::MappingArity {
            expected: provider.binder_arity(),
            actual: template.type_parameters().len_u32(),
        });
    }
    for (index, argument) in template.type_parameters().arguments().iter().enumerate() {
        owner_scope
            .validate_signature_semantics_metered(argument, authority, meter, &path)
            .map_err(|error| Error::MappingArgument { index, error })?;
    }
    let definition_len = template.definition_path().segments().len() as u64;
    meter
        .charge_work(definition_len, &path)
        .map_err(Error::Resource)?;
    for local in template.locals().records() {
        let length = match local.selector() {
            LocalValueSelector::This | LocalValueSelector::Parameter { .. } => 0,
            LocalValueSelector::LocalDeclaration { path }
            | LocalValueSelector::BoundReceiver { path }
            | LocalValueSelector::Synthetic { path, .. } => path.segments().len() as u64,
            LocalValueSelector::SuspensionResult { site } => site.segments().len() as u64,
        };
        meter
            .charge_work(
                length.saturating_add(definition_len).saturating_add(1),
                &path,
            )
            .map_err(Error::Resource)?;
        // Reserve the existing scope diagnostic's owned selector on failure.
        meter
            .charge_nodes(length.saturating_add(1), &path)
            .map_err(Error::Resource)?;
        meter
            .charge_collection_slots(length, &path)
            .map_err(Error::Resource)?;
    }
    template
        .locals()
        .validate_definition_path(template.definition_path())
        .map_err(Error::LocalScope)?;
    let scope = provider.signature_scope();
    for (index, local) in template.locals().records().iter().enumerate() {
        scope
            .validate_signature_semantics_metered(local.value_type(), authority, meter, &path)
            .map_err(|error| Error::LocalType { index, error })?;
    }
    scope
        .validate_signature_semantics_metered(template.result(), authority, meter, &path)
        .map_err(Error::ResultType)
}
