use super::*;
use scoop_identity::CallableTemplateOrigin;

pub(super) fn local(
    domains: &DefaultSourceDomainsV1<'_, '_, '_, '_>,
    function: &DefaultLocalFunctionV1,
    provider: &SignatureBinderScopeV1,

    path: &WirePath,
) -> Result<SignatureBinderScopeV1, Error> {
    // The declaration transaction already bound this typed attachment's exact
    // artifact key, role, parent and path. Query the same graph, not its ABI.
    let identities = domains.current.foundation.identities;

    let key = match function.declaration() {
        CallableTemplateOrigin::Function(id) => {
            identities.canonical_key::<_, SourceDeclarationKey>(id)
        }
        CallableTemplateOrigin::GenericFunction(id) => {
            identities.canonical_key::<_, SourceDeclarationKey>(id)
        }
        other => {
            return Err(Error::Identity(format!(
                "local signature {other:?} is not a function"
            )));
        }
    }
    .map_err(|error| Error::Identity(error.to_string()))?;

    provider
        .with_inner_frame(key.duplicate_signature().type_parameter_count(), path)
        .map_err(Error::from)
}
