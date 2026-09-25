//! Replays provider lookup visibility from artifact-bound declaration sources.
use super::super::default_access::lookup_domain;
use super::*;
use scoop_identity::{PersistentExactTypeId, PersistentGenericTypeId};

mod errors;
pub use errors::DefaultSourceDirectDomainError;
type DomainError = DefaultSourceDirectDomainError;

pub(super) fn validate(
    provider: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    template: &DefaultSourceTemplateV1,
    references: &DefaultSourceReferenceClosureV1<'_>,

    path: &WirePath,
) -> Result<DefaultSourceAccessDomainV1, Error> {
    let owner = template.definition_root().declaration();
    let expected = source_domain(provider, owner, path)?;

    for occurrence in references.occurrences() {
        let actual = occurrence.source().witness().direct_call_domain();

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

pub(super) fn source_domain(
    provider: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    owner: CallableTemplateOrigin,

    path: &WirePath,
) -> Result<DefaultSourceAccessDomainV1, Error> {
    let foundation = provider.members().nominals.foundation;
    let access = match owner {
        CallableTemplateOrigin::Constructor(id) => provider
            .constructors()
            .constructor_source(id)?
            .declaration_access(),
        CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_) => {
            provider
                .members()
                .callable_source(owner)?
                .declaration_access()
        }
        CallableTemplateOrigin::VariantConstructor(_) => {
            let nominal = provider.members().callable_source(owner)?.payload().owner();

            foundation
                .nominal_source(nominal)
                .map_err(DomainError::Foundation)?
                .access()
        }
        CallableTemplateOrigin::Accessor(_) => return Err(Error::Declaration(owner)),
    };
    lookup_domain(access, foundation, path)
        .map_err(DomainError::from)
        .map_err(Error::from)
}
