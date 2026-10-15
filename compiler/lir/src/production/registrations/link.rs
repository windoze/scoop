use super::*;
use crate::{LinkDataError, link_data::link_error};
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedRegistrationIdentitySurfaceV1 {
    pub(crate) fn read_link(
        self,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<[Vec<(ObjectDefinitionPlanId, PersistentSymbolRequest)>; 6], LinkDataError> {
        Ok([
            table(
                self.static_storages,
                PersistentSymbolKey::RootRegistration,
                foundation,
                identities,
            )?,
            table(
                self.immortal_objects,
                PersistentSymbolKey::ImmortalRegistration,
                foundation,
                identities,
            )?,
            table(
                self.initialization_units,
                PersistentSymbolKey::InitializationRegistration,
                foundation,
                identities,
            )?,
            table(
                self.type_registrations,
                PersistentSymbolKey::TypeRegistration,
                foundation,
                identities,
            )?,
            table(
                self.safepoints,
                PersistentSymbolKey::SafepointRegistration,
                foundation,
                identities,
            )?,
            table(
                self.callables,
                PersistentSymbolKey::CallableRegistration,
                foundation,
                identities,
            )?,
        ])
    }
}

fn table<I: PersistentId>(
    entries: Vec<DecodedRegistrationIdentityV1<I>>,
    key: impl Fn(I) -> PersistentSymbolKey,
    foundation: &ConeLirFoundation,
    identities: &mut ValidatedIdentityGraph,
) -> Result<Vec<(ObjectDefinitionPlanId, PersistentSymbolRequest)>, LinkDataError>
where
    ValidatedIdentityGraph: PersistentIdResolver<I>,
    <ValidatedIdentityGraph as PersistentIdResolver<I>>::Error: std::fmt::Display,
{
    entries
        .into_iter()
        .map(|entry| {
            let id = identities.resolve(entry.semantic_id).map_err(link_error)?;
            let definition = <ValidatedIdentityGraph as PersistentIdResolver<
                ObjectDefinitionPlanId,
            >>::resolve(identities, entry.definition_plan)
            .map_err(link_error)?;
            let owner = foundation
                .definition_plan(definition)
                .ok_or_else(|| link_error(format!("missing registration definition {definition}")))?
                .key()
                .owner();
            let linkage = match owner {
                ObjectDefinitionPlanOwner::Strong { .. } => LinkageClass::ConeStrong,
                ObjectDefinitionPlanOwner::Odr { .. } => LinkageClass::OdrWeak,
            };
            Ok((
                definition,
                PersistentSymbolRequest::new(key(id), linkage).map_err(link_error)?,
            ))
        })
        .collect()
}
