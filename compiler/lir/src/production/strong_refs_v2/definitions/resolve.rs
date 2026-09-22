use super::*;
use crate::{
    OdrFreeLirFoundation, StrongExternalLirBridgeSurfaceV1, StrongRegistrationIdentitySurfaceV1,
};

type Error = StrongTypeReferenceResolutionErrorV2;

impl StrongTypeReferenceDefinitionsV2 {
    pub fn resolve_descriptor(
        &self,
        decoded: DecodedStrongTypeDescriptorRefV2,
        foundation: &OdrFreeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
        external: &StrongExternalLirBridgeSurfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<StrongTypeDescriptorRefV2, Error> {
        self.check_producer(foundation.producer())?;
        self.check_producer(external.producer())?;
        let path = WirePath::root();
        match decoded {
            DecodedStrongTypeDescriptorRefV2::Local(exact) => {
                meter.charge_work(registrations.type_registrations().len() as u64, &path)?;
                let exact = registrations
                    .type_registrations()
                    .iter()
                    .find(|record| record.semantic_id().as_array() == exact.as_array())
                    .map(|record| record.semantic_id())
                    .ok_or(Error::UnknownLocalDescriptor(exact))?;
                StrongShapeDefinitionRefV1::from_foundation(
                    ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
                    foundation,
                    meter,
                )
                .map_err(Error::LocalDefinition)?;
                Ok(StrongTypeDescriptorRefV2::Local(exact))
            }
            DecodedStrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
                meter.charge_work(foundation.definition_plans().len() as u64, &path)?;
                for definition in foundation.definition_plans() {
                    if let Some(PersistentSymbolKey::TypeDescriptor(candidate)) =
                        definition.key().primary_symbol_key()
                        && candidate.as_array() == exact.as_array()
                    {
                        return Err(Error::LocalDescriptorPartition(candidate));
                    }
                }
                meter.charge_work(external.bridges().len() as u64, &path)?;
                let service = external
                    .resolve_descriptor_reference(provider, exact)
                    .map(|descriptor| (descriptor.provider(), descriptor.target()));
                meter.charge_work(self.descriptors.len() as u64, &path)?;
                let layout = self.descriptors.iter().find_map(|definition| {
                    let ExternalStrongShapeSubjectV1::TypeDescriptor(candidate) =
                        definition.subject()
                    else {
                        return None;
                    };
                    (definition.provider().as_array() == provider.as_array()
                        && candidate.as_array() == exact.as_array())
                    .then_some((definition.provider(), candidate))
                });
                let (provider, exact) = match (service, layout) {
                    (Some((_, exact)), Some(_)) => {
                        return Err(Error::ConflictingDescriptorSources(exact));
                    }
                    (Some(reference), None) | (None, Some(reference)) => reference,
                    (None, None) => {
                        return Err(Error::UnknownDependencyDescriptor { provider, exact });
                    }
                };
                Ok(StrongTypeDescriptorRefV2::DependencyExternal { provider, exact })
            }
        }
    }

    pub fn resolve_optional_descriptor(
        &self,
        decoded: DecodedOptionalStrongTypeDescriptorRefV2,
        foundation: &OdrFreeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
        external: &StrongExternalLirBridgeSurfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<Option<StrongTypeDescriptorRefV2>, Error> {
        self.check_producer(foundation.producer())?;
        self.check_producer(external.producer())?;
        let descriptor = match decoded {
            DecodedOptionalStrongTypeDescriptorRefV2::Absent => return Ok(None),
            DecodedOptionalStrongTypeDescriptorRefV2::Local(exact) => {
                DecodedStrongTypeDescriptorRefV2::Local(exact)
            }
            DecodedOptionalStrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
                DecodedStrongTypeDescriptorRefV2::DependencyExternal { provider, exact }
            }
        };
        self.resolve_descriptor(descriptor, foundation, registrations, external, meter)
            .map(Some)
    }

    pub fn resolve_dispatch_callable(
        &self,
        decoded: DecodedStrongTypeDispatchCallableRefV2,
        foundation: &OdrFreeLirFoundation,
        external: &StrongExternalLirBridgeSurfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<StrongTypeDispatchCallableRefV2, Error> {
        self.check_producer(foundation.producer())?;
        self.check_producer(external.producer())?;
        let path = WirePath::root();
        match decoded {
            DecodedStrongTypeDispatchCallableRefV2::Local(body) => {
                meter.charge_work(foundation.callable_bodies().len() as u64, &path)?;
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
                meter.charge_work(foundation.definition_plans().len() as u64, &path)?;
                for definition in foundation.definition_plans() {
                    if let Some(PersistentSymbolKey::CallableBody(candidate)) =
                        definition.key().primary_symbol_key()
                        && candidate.as_array() == body.as_array()
                    {
                        return Err(Error::LocalCallablePartition(candidate));
                    }
                }
                meter.charge_work(external.bridges().len() as u64, &path)?;
                let service = external.resolve_callable_reference(provider, body);
                meter.charge_work(self.callables.len() as u64, &path)?;
                let layout = self.callables.iter().find_map(|definition| {
                    let PersistentSymbolKey::CallableBody(candidate) = definition.symbol().key()
                    else {
                        return None;
                    };
                    (definition.provider().as_array() == provider.as_array()
                        && candidate.as_array() == body.as_array())
                    .then_some((definition.provider(), candidate))
                });
                let (provider, body) = match (service, layout) {
                    (Some((_, body)), Some(_)) => {
                        return Err(Error::ConflictingCallableSources(body));
                    }
                    (Some(reference), None) | (None, Some(reference)) => reference,
                    (None, None) => {
                        return Err(Error::UnknownDependencyCallable { provider, body });
                    }
                };
                Ok(StrongTypeDispatchCallableRefV2::DependencyExternal { provider, body })
            }
        }
    }
}
