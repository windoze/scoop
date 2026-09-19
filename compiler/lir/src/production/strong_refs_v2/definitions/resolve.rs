use super::*;
use crate::{
    OdrFreeLirFoundation, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1,
    StrongRegistrationIdentitySurfaceV1,
};

type Error = StrongTypeReferenceResolutionErrorV2;

impl StrongTypeReferenceDefinitionsV2 {
    pub fn resolve_descriptor(
        &self,
        decoded: DecodedStrongTypeDescriptorRefV2,
        foundation: &OdrFreeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
        core: &StrongExternalLirBridgeSurfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<StrongTypeDescriptorRefV2, Error> {
        self.check_producer(foundation.producer())?;
        self.check_producer(core.producer())?;
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
            DecodedStrongTypeDescriptorRefV2::CoreExternal(exact) => {
                meter.charge_work(core.bridges().len() as u64, &path)?;
                core_descriptor(core, exact)
                    .map(StrongTypeDescriptorRefV2::CoreExternal)
                    .ok_or(Error::UnknownCoreDescriptor(exact))
            }
            DecodedStrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
                meter.charge_work(core.bridges().len() as u64, &path)?;
                if let Some(exact) = core_descriptor(core, exact) {
                    return Err(Error::CoreDescriptorPartition(exact));
                }
                meter.charge_work(self.descriptors.len() as u64, &path)?;
                self.descriptors
                    .iter()
                    .find_map(|definition| {
                        let ExternalStrongShapeSubjectV1::TypeDescriptor(candidate) =
                            definition.subject()
                        else {
                            return None;
                        };
                        (definition.provider().as_array() == provider.as_array()
                            && candidate.as_array() == exact.as_array())
                        .then_some(StrongTypeDescriptorRefV2::DependencyExternal {
                            provider: definition.provider(),
                            exact: candidate,
                        })
                    })
                    .ok_or(Error::UnknownDependencyDescriptor { provider, exact })
            }
        }
    }

    pub fn resolve_optional_descriptor(
        &self,
        decoded: DecodedOptionalStrongTypeDescriptorRefV2,
        foundation: &OdrFreeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
        core: &StrongExternalLirBridgeSurfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<Option<StrongTypeDescriptorRefV2>, Error> {
        // Even an absent reference must not move a catalog between consumers.
        self.check_producer(foundation.producer())?;
        self.check_producer(core.producer())?;
        let descriptor = match decoded {
            DecodedOptionalStrongTypeDescriptorRefV2::Absent => return Ok(None),
            DecodedOptionalStrongTypeDescriptorRefV2::Local(exact) => {
                DecodedStrongTypeDescriptorRefV2::Local(exact)
            }
            DecodedOptionalStrongTypeDescriptorRefV2::CoreExternal(exact) => {
                DecodedStrongTypeDescriptorRefV2::CoreExternal(exact)
            }
            DecodedOptionalStrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
                DecodedStrongTypeDescriptorRefV2::DependencyExternal { provider, exact }
            }
        };
        self.resolve_descriptor(descriptor, foundation, registrations, core, meter)
            .map(Some)
    }

    pub fn resolve_dispatch_callable(
        &self,
        decoded: DecodedStrongTypeDispatchCallableRefV2,
        foundation: &OdrFreeLirFoundation,
        core: &StrongExternalLirBridgeSurfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<StrongTypeDispatchCallableRefV2, Error> {
        self.check_producer(foundation.producer())?;
        self.check_producer(core.producer())?;
        let path = WirePath::root();
        match decoded {
            DecodedStrongTypeDispatchCallableRefV2::Local(body) => {
                meter.charge_work(foundation.callable_bodies().len() as u64, &path)?;
                let record = foundation
                    .callable_bodies()
                    .iter()
                    .find(|record| record.id().as_array() == body.as_array())
                    .ok_or(Error::UnknownLocalCallable(body))?;
                // The complete local callable-registration table checks this
                // body's definition; dependency bodies use the catalog proof.
                Ok(StrongTypeDispatchCallableRefV2::Local(record.id()))
            }
            DecodedStrongTypeDispatchCallableRefV2::CoreExternal(body) => {
                meter.charge_work(core.bridges().len() as u64, &path)?;
                core_callable(core, body)
                    .map(StrongTypeDispatchCallableRefV2::CoreExternal)
                    .ok_or(Error::UnknownCoreCallable(body))
            }
            DecodedStrongTypeDispatchCallableRefV2::Runtime(function) => {
                Ok(StrongTypeDispatchCallableRefV2::Runtime(function))
            }
            DecodedStrongTypeDispatchCallableRefV2::DependencyExternal { provider, body } => {
                meter.charge_work(core.bridges().len() as u64, &path)?;
                if let Some(body) = core_callable(core, body) {
                    return Err(Error::CoreCallablePartition(body));
                }
                meter.charge_work(self.callables.len() as u64, &path)?;
                self.callables
                    .iter()
                    .find_map(|definition| {
                        let PersistentSymbolKey::CallableBody(candidate) =
                            definition.symbol().key()
                        else {
                            return None;
                        };
                        (definition.provider().as_array() == provider.as_array()
                            && candidate.as_array() == body.as_array())
                        .then_some(StrongTypeDispatchCallableRefV2::DependencyExternal {
                            provider: definition.provider(),
                            body: candidate,
                        })
                    })
                    .ok_or(Error::UnknownDependencyCallable { provider, body })
            }
        }
    }
}

fn core_descriptor(
    core: &StrongExternalLirBridgeSurfaceV1,
    exact: DecodedPersistentId<PersistentExactTypeId>,
) -> Option<PersistentExactTypeId> {
    core.bridges().iter().find_map(|bridge| match bridge {
        StrongExternalLirBridgeV1::TypeDescriptor(bridge)
            if bridge.target().as_array() == exact.as_array() =>
        {
            Some(bridge.target())
        }
        _ => None,
    })
}

fn core_callable(
    core: &StrongExternalLirBridgeSurfaceV1,
    body: DecodedPersistentId<PersistentCallableBodyId>,
) -> Option<PersistentCallableBodyId> {
    core.bridges().iter().find_map(|bridge| match bridge {
        StrongExternalLirBridgeV1::Callable(bridge) => match bridge.expected_symbol().key() {
            PersistentSymbolKey::CallableBody(candidate)
                if candidate.as_array() == body.as_array() =>
            {
                Some(candidate)
            }
            _ => None,
        },
        StrongExternalLirBridgeV1::TypeDescriptor(_) => None,
    })
}
