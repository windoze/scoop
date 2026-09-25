use super::*;

impl Lowerer {
    pub(crate) fn lower_imported_dependency_property_read(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        receiver: Option<PropertyCallReceiver>,
        span: ast::Span,
    ) -> Option<ImportedDependencyPropertyRead> {
        let property = self.imported_dependency_property_candidate(binding, span)?;
        if property.interface().representation() == hir::PropertyRepresentationV1::Const {
            if receiver.is_some() {
                self.error(
                    span,
                    "invalid imported dependency const property receiver".to_string(),
                );
                return None;
            }
            return self
                .lower_imported_dependency_constant(binding, span)
                .map(|expression| ImportedDependencyPropertyRead {
                    expression,
                    has_setter: false,
                });
        }

        let candidate = self.imported_dependency_property_accessor(
            &property,
            hir::ImportedDependencyPropertyAccessorKind::Getter,
            "dependency property getter",
            span,
        )?;
        let receiver = self.validate_imported_property_receiver(&property, receiver, span)?;
        let result_type = self.imported_property_value_type(&property, span)?;
        let source_receiver = PropertyCallReceiver::source_type(&receiver);
        let expression = self.emit_imported_property_accessor(
            candidate,
            receiver
                .map(|receiver| receiver.value)
                .into_iter()
                .collect(),
            source_receiver,
            result_type,
            span,
            "reading an unsafe dependency property",
        )?;
        Some(ImportedDependencyPropertyRead {
            expression,
            has_setter: property.interface().capability().setter().is_some(),
        })
    }
}
