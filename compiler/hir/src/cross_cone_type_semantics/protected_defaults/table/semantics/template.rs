use super::*;
use crate::{
    CheckedProtectedSourceProtocolV1, ProtectedDefaultOwnerSourceV1, ProtectedDefaultTemplateV1,
    ProtectedDefaultWitnessSourceProfileV1,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate<A: ProtectedDefaultSemanticAuthority<E>, E>(
    template: &ProtectedDefaultTemplateV1,
    owner: ProtectedDefaultOwnerSourceV1<'_>,
    protocol: CheckedProtectedSourceProtocolV1<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    inheritance: CheckedNominalInheritanceInterfacesV1<'_>,
    authority: &mut A,

    path: &WirePath,
) -> Result<ProtectedDefaultWitnessSourceProfileV1, ProtectedDefaultSemanticError<E>> {
    use ProtectedDefaultSemanticError as Error;
    let provider = template
        .validate_contract_semantics_with_provider(owner, protocol, authority)
        .map_err(|error| Error::Contract(Box::new(error)))?;
    template
        .body()
        .validate_provider_envelope_semantics(provider, authority, path)
        .map_err(|error| Error::BodyEnvelope(Box::new(error)))?;
    template
        .validate_origin_semantics(authority)
        .map_err(|error| Error::Origin(Box::new(error)))?;
    template
        .validate_local_data_flow_semantics(authority, path)
        .map_err(|error| Error::LocalDataFlow(Box::new(error)))?;
    template
        .validate_operation_typing_semantics(authority, path)
        .map_err(|error| Error::OperationTyping(Box::new(error)))?;
    template
        .validate_nested_callable_abi_semantics(authority, path)
        .map_err(|error| Error::NestedCallableAbi(Box::new(error)))?;
    let references = template
        .validate_reference_access_semantics(owner, graph, inheritance, authority, path)
        .map_err(|error| Error::References(Box::new(error)))?;
    Ok(references.owner().profile())
}
