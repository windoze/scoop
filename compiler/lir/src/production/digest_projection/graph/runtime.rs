use super::*;

impl DigestGraphWriter<'_> {
    pub(super) fn project_types(
        &mut self,
        semantics: impl IntoIterator<
            Item = (
                scoop_identity::PersistentExactTypeId,
                scoop_identity::PersistentLayoutId,
            ),
        >,
    ) -> Result<(), DigestProjectionError> {
        for (exact, layout) in semantics {
            let registration = self.definition(
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);

            let descriptor = self.definition(
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeDescriptor,
            )?;
            let descriptor_node = DigestNodeKey::object_definition(descriptor.primary);
            self.ensure(descriptor_node);
            self.patch(
                descriptor_node,
                registration.plan,
                DigestSemanticFieldRole::DescriptorDefinition,
            )?;

            let layout_node = DigestNodeKey::layout(layout);
            self.ensure(layout_node);
            self.patch(
                layout_node,
                registration.plan,
                DigestSemanticFieldRole::Layout,
            )?;
            self.registration(
                registration.plan,
                [registration_object, descriptor_node, layout_node],
            )?;
        }
        Ok(())
    }

    pub(super) fn project_immortal_objects(
        &mut self,
        semantics: impl IntoIterator<Item = scoop_identity::PersistentImmortalObjectId>,
    ) -> Result<(), DigestProjectionError> {
        for object in semantics {
            let definition = self.definition(
                StrongDefinitionEntity::immortal_object(object),
                StrongDefinitionRole::ImmortalObject,
            )?;
            let object_node = DigestNodeKey::object_definition(definition.primary);
            self.ensure(object_node);

            let registration = self.definition(
                StrongDefinitionEntity::immortal_object(object),
                StrongDefinitionRole::ImmortalRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);
            let record = self
                .foundation
                .definition_plan(registration.plan)
                .expect("the selected registration retains its definition plan");
            let node = self.foundation.registration_digest_key(record);
            if node.kind() == DigestKind::OdrDefinition {
                self.registration(registration.plan, [registration_object])?;
                self.input(node, object_node);
            } else {
                self.registration(registration.plan, [registration_object, object_node])?;
            }
        }
        Ok(())
    }

    pub(super) fn project_static_storages(
        &mut self,
        storages: impl IntoIterator<
            Item = (
                scoop_identity::PersistentStaticStorageId,
                scoop_identity::PersistentLayoutId,
                scoop_identity::PersistentScanId,
            ),
        >,
    ) -> Result<(), DigestProjectionError> {
        for (storage, layout, scan) in storages {
            let definition = self.definition(
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::StaticStorage,
            )?;
            let storage_node = DigestNodeKey::object_definition(definition.primary);
            self.ensure(storage_node);

            let registration = self.definition(
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::RootRegistration,
            )?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);

            let layout_node = DigestNodeKey::layout(layout);
            self.ensure(layout_node);
            self.patch(
                layout_node,
                registration.plan,
                DigestSemanticFieldRole::Layout,
            )?;

            let scan_node = DigestNodeKey::scan(scan);
            self.ensure(scan_node);
            self.patch(scan_node, registration.plan, DigestSemanticFieldRole::Scan)?;
            self.registration(
                registration.plan,
                [registration_object, storage_node, layout_node, scan_node],
            )?;
        }
        Ok(())
    }

    pub(super) fn project_initialization_units(
        &mut self,
        semantics: impl IntoIterator<
            Item = (
                scoop_identity::PersistentInitializationUnitId,
                StrongInitializationSchedulePlanV1,
            ),
        >,
    ) -> Result<(), DigestProjectionError> {
        for (unit, schedule) in semantics {
            let entity = StrongDefinitionEntity::initialization_unit(unit);
            let registration =
                self.definition(entity, StrongDefinitionRole::InitializationRegistration)?;
            let registration_object = DigestNodeKey::object_definition(registration.primary);
            self.ensure(registration_object);

            let cell = self.definition(entity, StrongDefinitionRole::InitializationCell)?;
            let cell_node = DigestNodeKey::object_definition(cell.primary);
            self.ensure(cell_node);

            let descriptor =
                self.definition(entity, StrongDefinitionRole::InitializationDescriptor)?;
            let descriptor_node = DigestNodeKey::object_definition(descriptor.primary);
            self.ensure(descriptor_node);

            let mut inputs = vec![registration_object, cell_node, descriptor_node];
            if let StrongInitializationSchedulePlanV1::EagerStartup { gateway } = schedule {
                let gateway = self.definition(
                    StrongDefinitionEntity::callable_body(gateway),
                    StrongDefinitionRole::CallableBody,
                )?;
                let gateway_node = DigestNodeKey::object_definition(gateway.primary);
                self.ensure(gateway_node);
                self.patch(
                    gateway_node,
                    registration.plan,
                    DigestSemanticFieldRole::GatewayDefinition,
                )?;
                inputs.push(gateway_node);
            }
            self.registration(registration.plan, inputs)?;
        }
        Ok(())
    }

    pub(super) fn project_image(&mut self) -> Result<(), DigestProjectionError> {
        let image = self.definition(
            StrongDefinitionEntity::cone_image(self.foundation.producer()),
            StrongDefinitionRole::ImageDescriptor,
        )?;
        let image_node = DigestNodeKey::runtime_image(self.foundation.producer());
        self.ensure(image_node);
        for registration in self.registration_nodes.clone() {
            self.input(image_node, registration);
        }
        self.patch(
            image_node,
            image.plan,
            DigestSemanticFieldRole::RuntimeImage,
        )
    }
}
