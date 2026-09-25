use super::*;
use crate::SignatureBinderScopeV1;

pub(super) fn validate<A: NominalInterfaceShapeAuthority<E>, E>(
    template: &ProtectedDefaultTemplateV1,
    provider: DefaultTemplateProviderShapeV1,
    owner_scope: &SignatureBinderScopeV1,
    authority: &mut A,
) -> Result<(), ProtectedDefaultTemplateContractSemanticError<E>> {
    use ProtectedDefaultTemplateContractSemanticError as Error;
    if template.type_parameters().len_u32() != provider.binder_arity() {
        return Err(Error::MappingArity {
            expected: provider.binder_arity(),
            actual: template.type_parameters().len_u32(),
        });
    }
    for (index, argument) in template.type_parameters().arguments().iter().enumerate() {
        owner_scope
            .validate_signature_semantics(argument, authority)
            .map_err(|error| Error::MappingArgument { index, error })?;
    }

    template
        .locals()
        .validate_definition_path(template.definition_path())
        .map_err(Error::LocalScope)?;
    let scope = provider.signature_scope();
    for (index, local) in template.locals().records().iter().enumerate() {
        scope
            .validate_signature_semantics(local.value_type(), authority)
            .map_err(|error| Error::LocalType { index, error })?;
    }
    scope
        .validate_signature_semantics(template.result(), authority)
        .map_err(Error::ResultType)
}
