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
            let expression = self.lower_imported_dependency_constant(binding, span)?;
            self.observe_property_reference(
                span,
                hir::SemanticProperty::Imported(property.interface().declaration()),
                expression.ty,
                hir::EditingPropertyAccess::Read,
            );
            return Some(ImportedDependencyPropertyRead {
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
        self.observe_property_reference(
            span,
            hir::SemanticProperty::Imported(property.interface().declaration()),
            result_type,
            hir::EditingPropertyAccess::Read,
        );
        if let Some(place) = self.imported_property_native_place(&property, span)? {
            self.require_unsafe_operation(span, "reading an extern global");
            let pointer = self.external_storage_pointer(place, span);
            return Some(ImportedDependencyPropertyRead {
                expression: hir::Expr {
                    kind: hir::ExprKind::PtrLoad {
                        pointer: Box::new(pointer),
                        offset: None,
                    },
                    ty: result_type,
                    span,
                    origin: self.expression_origin(span),
                },
                has_setter: property.interface().accessors().setter().is_some(),
            });
        }
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
            has_setter: property.interface().accessors().setter().is_some(),
        })
    }
}
