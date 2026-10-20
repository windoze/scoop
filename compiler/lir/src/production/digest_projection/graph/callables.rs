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
            let stackmap = DigestNodeKey::stackmap_record(site);
            self.ensure(stackmap);

            self.image_inputs.extend([stackmap]);

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

            self.image_inputs.extend([body_node]);
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
        let source_node = DigestNodeKey::source_signature(entry.main().body());
        self.ensure(source_node);

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

        Ok(())
    }
}
