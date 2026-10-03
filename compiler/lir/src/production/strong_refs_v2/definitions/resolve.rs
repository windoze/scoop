use super::*;
use crate::{ConeLirFoundation, RegistrationIdentitySurfaceV1};

type Error = StrongTypeReferenceResolutionErrorV2;

impl StrongTypeReferenceDefinitionsV2 {
    pub fn resolve_descriptor(
        &self,
        decoded: DecodedStrongTypeDescriptorRefV2,
        foundation: &ConeLirFoundation,
        registrations: &RegistrationIdentitySurfaceV1,
    ) -> Result<StrongTypeDescriptorRefV2, Error> {
        self.check_producer(foundation.producer())?;

        match decoded {
            DecodedStrongTypeDescriptorRefV2::Local(exact) => {
                let exact = registrations
                    .type_registrations()
                    .iter()
                    .find(|record| record.semantic_id().as_array() == exact.as_array())
                    .map(|record| record.semantic_id())
                    .ok_or(Error::UnknownLocalDescriptor(exact))?;
                StrongShapeDefinitionRefV1::from_foundation(
                    ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
                    foundation,
                )
                .map_err(Error::LocalDefinition)?;
                Ok(StrongTypeDescriptorRefV2::Local(exact))
            }
            DecodedStrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
                for definition in foundation.definition_plans() {
                    if let Some(PersistentSymbolKey::TypeDescriptor(candidate)) =
                        definition.key().primary_symbol_key()
                        && candidate.as_array() == exact.as_array()
                    {
                        return Err(Error::LocalDescriptorPartition(candidate));
                    }
                }

                let (provider, exact) = self
                    .resolve_external_descriptor(provider, exact)
                    .ok_or(Error::UnknownDependencyDescriptor { provider, exact })?;
                Ok(StrongTypeDescriptorRefV2::DependencyExternal { provider, exact })
            }
        }
    }

    pub fn resolve_optional_descriptor(
        &self,
        decoded: DecodedOptionalStrongTypeDescriptorRefV2,
        foundation: &ConeLirFoundation,
        registrations: &RegistrationIdentitySurfaceV1,
    ) -> Result<Option<StrongTypeDescriptorRefV2>, Error> {
        self.check_producer(foundation.producer())?;
        let descriptor = match decoded {
            DecodedOptionalStrongTypeDescriptorRefV2::Absent => return Ok(None),
            DecodedOptionalStrongTypeDescriptorRefV2::Local(exact) => {
                DecodedStrongTypeDescriptorRefV2::Local(exact)
            }
            DecodedOptionalStrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
                DecodedStrongTypeDescriptorRefV2::DependencyExternal { provider, exact }
            }
        };
        self.resolve_descriptor(descriptor, foundation, registrations)
            .map(Some)
    }

    pub fn resolve_dispatch_callable(
        &self,
        decoded: DecodedStrongTypeDispatchCallableRefV2,
        foundation: &ConeLirFoundation,
    ) -> Result<StrongTypeDispatchCallableRefV2, Error> {
        self.check_producer(foundation.producer())?;

        match decoded {
            DecodedStrongTypeDispatchCallableRefV2::Local(body) => {
                let record = foundation
                    .callable_bodies()
                    .iter()
                    .find(|record| record.id().as_array() == body.as_array())
                    .ok_or(Error::UnknownLocalCallable(body))?;
                Ok(StrongTypeDispatchCallableRefV2::Local(record.id()))
            }
            DecodedStrongTypeDispatchCallableRefV2::Runtime(function) => {
                Ok(StrongTypeDispatchCallableRefV2::Runtime(function))
            }
            DecodedStrongTypeDispatchCallableRefV2::DependencyExternal { provider, body } => {
                for definition in foundation.definition_plans() {
                    if let Some(PersistentSymbolKey::CallableBody(candidate)) =
                        definition.key().primary_symbol_key()
                        && candidate.as_array() == body.as_array()
                    {
                        return Err(Error::LocalCallablePartition(candidate));
                    }
                }

                let (provider, body) = self
                    .callables
                    .iter()
                    .find_map(|definition| {
                        let PersistentSymbolKey::CallableBody(candidate) =
                            definition.symbol().key()
                        else {
                            return None;
                        };
                        (definition.provider().as_array() == provider.as_array()
                            && candidate.as_array() == body.as_array())
                        .then_some((definition.provider(), candidate))
                    })
                    .ok_or(Error::UnknownDependencyCallable { provider, body })?;
                Ok(StrongTypeDispatchCallableRefV2::DependencyExternal { provider, body })
            }
        }
    }
}
