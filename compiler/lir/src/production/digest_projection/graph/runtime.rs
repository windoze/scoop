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
            let descriptor = self.definition(
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeDescriptor,
            )?;
            let descriptor_node = DigestNodeKey::object_definition(descriptor.primary);
            self.ensure(descriptor_node);

            let layout_node = DigestNodeKey::layout(layout);
            self.ensure(layout_node);

            self.image_inputs.extend([descriptor_node, layout_node]);
        }
        Ok(())
    }

    pub(super) fn project_static_storages(
        &mut self,
        storages: impl IntoIterator<
            Item = (
                scoop_identity::PersistentLayoutId,
                scoop_identity::PersistentScanId,
            ),
        >,
    ) {
        for (layout, scan) in storages {
            let layout_node = DigestNodeKey::layout(layout);
            self.ensure(layout_node);

            let scan_node = DigestNodeKey::scan(scan);
            self.ensure(scan_node);

            self.image_inputs.extend([layout_node, scan_node]);
        }
    }

    pub(super) fn project_initialization_units(
        &mut self,
        semantics: impl IntoIterator<Item = StrongInitializationSchedulePlanV1>,
    ) -> Result<(), DigestProjectionError> {
        for schedule in semantics {
            if let StrongInitializationSchedulePlanV1::EagerStartup { gateway } = schedule {
                let gateway = self.definition(
                    StrongDefinitionEntity::callable_body(gateway),
                    StrongDefinitionRole::CallableBody,
                )?;
                let gateway_node = DigestNodeKey::object_definition(gateway.primary);
                self.ensure(gateway_node);

                self.image_inputs.insert(gateway_node);
            }
        }
        Ok(())
    }

    pub(super) fn project_image(&mut self) {
        let image_node = DigestNodeKey::runtime_image(self.foundation.producer());
        self.ensure(image_node);
        for registration in self.image_inputs.clone() {
            self.input(image_node, registration);
        }
    }
}
