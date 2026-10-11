use super::*;

impl Lowerer {
    pub(super) fn infer_property_getter(&mut self, property: hir::PropertyId) -> Option<TypeId> {
        let source = self.property_accessor_source(property, PropertyAccessorKind::Getter);
        let body = if source.body_kind == PropertyAccessorBodyKind::Delegate {
            self.lower_generated_delegate_accessor(&source)
        } else {
            self.lower_body(source.function, &source.declaration)
        };
        let mut body = body;
        self.prepend_accessor_initialization_ensure(&source, &mut body);
        self.functions[source.function].kind = hir::FunctionKind::User(body);
        if self.signatures[&source.function].result_pending {
            return None;
        }
        self.declaration_inference
            .lowered_accessors
            .insert(source.function);
        Some(self.property_type_from_accessor(property, source.function))
    }

    pub(crate) fn property_accessor_was_lowered(&self, function: FunctionId) -> bool {
        self.declaration_inference
            .lowered_accessors
            .contains(&function)
    }
}
