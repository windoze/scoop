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
            self.image_inputs.extend([descriptor_node, layout_node]);
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
            let registration = self.definition(
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::RootRegistration,
            )?;

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
            self.image_inputs.extend([layout_node, scan_node]);
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
                self.image_inputs.insert(gateway_node);
            }
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
        for registration in self.image_inputs.clone() {
            self.input(image_node, registration);
        }
        self.patch(
            image_node,
            image.plan,
            DigestSemanticFieldRole::RuntimeImage,
        )
    }
}
