use super::*;

impl DigestGraphWriter<'_> {
    pub(super) fn project_safepoints(
        &mut self,
        semantics: impl IntoIterator<
            Item = (
                scoop_identity::PersistentSafepointSiteId,
                PersistentCallableBodyId,
            ),
        >,
    ) -> Result<(), DigestProjectionError> {
        for (site, owner) in semantics {
            let registration = self.definition(
                StrongDefinitionEntity::safepoint_site(site),
                StrongDefinitionRole::SafepointRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);

            let stackmap = DigestNodeKey::stackmap_record(site);
            self.ensure(stackmap);
            self.patch(
                stackmap,
                registration.plan,
                DigestSemanticFieldRole::NormalizedStackmap,
            )?;

            self.registration(registration.plan, [registration_object, stackmap])?;

            let body = self.definition(
                StrongDefinitionEntity::callable_body(owner),
                StrongDefinitionRole::CallableBody,
            )?;
            let body_node = DigestNodeKey::object_definition(body.primary);
            self.ensure(body_node);
            self.input(body_node, stackmap);
        }
        Ok(())
    }

    pub(super) fn project_callables(&mut self) -> Result<(), DigestProjectionError> {
        let bodies = self
            .foundation
            .callable_bodies()
            .iter()
            .map(|record| record.id())
            .collect::<Vec<_>>();
        for body in bodies {
            let definition = self.definition(
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableBody,
            )?;
            let body_node = DigestNodeKey::object_definition(definition.primary);
            self.ensure(body_node);

            let registration = self.definition(
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);
            self.patch(
                body_node,
                registration.plan,
                DigestSemanticFieldRole::CallableBodyDefinition,
            )?;
            self.registration(registration.plan, [registration_object, body_node])?;
        }
        Ok(())
    }

    pub(super) fn project_entry(
        &mut self,
        source: &EntryProductionSourceV1,
    ) -> Result<(), DigestProjectionError> {
        let EntryProductionSourceV1::Executable(entry) = source else {
            return Ok(());
        };
        if entry.root_cone() != self.foundation.producer() {
            return Err(DigestProjectionError::EntryProducerMismatch {
                entry: entry.root_cone(),
                foundation: self.foundation.producer(),
            });
        }
        let descriptor = self.definition(
            StrongDefinitionEntity::root_entry(entry.root_cone()),
            StrongDefinitionRole::RootEntryDescriptor,
        )?;
        let source_node = DigestNodeKey::source_signature(entry.main().body());
        self.ensure(source_node);
        self.patch(
            source_node,
            descriptor.plan,
            DigestSemanticFieldRole::SourceSignature,
        )?;

        let gateway = PersistentCallableBodyId::from_key(&CallableBodyKey::root_gateway(
            entry.root_cone(),
            entry.main(),
        ))
        .map_err(DigestProjectionError::Identity)?;
        let gateway = self.definition(
            StrongDefinitionEntity::callable_body(gateway),
            StrongDefinitionRole::CallableBody,
        )?;
        let gateway_node = DigestNodeKey::object_definition(gateway.primary);
        self.ensure(gateway_node);
        self.patch(
            gateway_node,
            descriptor.plan,
            DigestSemanticFieldRole::GatewayDefinition,
        )
    }
}
