//! Replays provider lookup visibility from artifact-bound declaration sources.
use super::*;
use scoop_identity::{
    ExactTypeKey, PersistentExactTypeId, PersistentGenericTypeId, SourceDeclarationKind,
};

mod errors;
mod replay;
pub use errors::DefaultSourceDirectDomainError;
type DomainError = DefaultSourceDirectDomainError;

pub(super) fn validate(
    provider: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    template: &DefaultSourceTemplateV1,
    references: &DefaultSourceReferenceClosureV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<DefaultSourceAccessDomainV1, Error> {
    let foundation = provider.members().nominals.foundation;
    let owner = template.definition_root().declaration();
    let access = match owner {
        CallableTemplateOrigin::Constructor(id) => {
            sources::query(provider.constructors().table().records().len(), meter, path)?;
            provider
                .constructors()
                .constructor_source(id)?
                .declaration_access()
        }
        CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_) => {
            sources::query(provider.members().callables().records().len(), meter, path)?;
            provider
                .members()
                .callable_source(owner)?
                .declaration_access()
        }
        CallableTemplateOrigin::VariantConstructor(_) => {
            sources::query(provider.members().callables().records().len(), meter, path)?;
            let nominal = provider.members().callable_source(owner)?.payload().owner();
            replay::query(foundation, meter, path)?;
            foundation
                .nominal_source(nominal)
                .map_err(DomainError::Foundation)?
                .access()
        }
        CallableTemplateOrigin::Accessor(_) => return Err(Error::Declaration(owner)),
    };
    let mut replay =
        replay::Replay::new(foundation, access.lexical_owners().len() + 1, meter, path)?;
    replay.declared(access)?;
    for owner in access.lexical_owners() {
        replay::query(foundation, replay.meter, path)?;
        let outer = foundation
            .nominal_source(*owner)
            .map_err(DomainError::Foundation)?
            .access();
        replay.declared(outer)?;
    }
    let expected = replay.finish()?;
    let expected_bytes = scoop_wire::encoded_length(&expected).map_err(DomainError::Encoding)?;
    for occurrence in references.occurrences() {
        let actual = occurrence.source().witness().direct_call_domain();
        let actual_bytes = scoop_wire::encoded_length(actual).map_err(DomainError::Encoding)?;
        meter.charge_work(expected_bytes.saturating_add(actual_bytes), path)?;
        if actual != &expected {
            return Err(DomainError::Witness {
                kind: occurrence.source().kind(),
                index: occurrence.index(),
            }
            .into());
        }
    }
    Ok(expected)
}
