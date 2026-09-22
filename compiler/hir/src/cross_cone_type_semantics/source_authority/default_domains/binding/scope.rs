use super::*;
use scoop_identity::CallableTemplateOrigin;

pub(super) fn local(
    domains: &DefaultSourceDomainsV1<'_, '_, '_, '_>,
    function: &DefaultLocalFunctionV1,
    provider: &SignatureBinderScopeV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<SignatureBinderScopeV1, Error> {
    // The declaration transaction already bound this typed attachment's exact
    // artifact key, role, parent and path. Query the same graph, not its ABI.
    let identities = domains.current.foundation.identities;
    meter.charge_edges(1, path)?;
    meter.charge_work(
        (u64::from(identities.identity_count().max(1).ilog2()) + 1) * 65,
        path,
    )?;
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
    NominalRepresentationSupportV1::charge_source_key_resources(&key, meter, path)?;
    provider
        .with_inner_frame(
            key.duplicate_signature().type_parameter_count(),
            meter,
            path,
        )
        .map_err(Error::from)
}
