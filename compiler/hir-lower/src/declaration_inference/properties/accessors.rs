use super::*;

impl Lowerer {
    pub(super) fn infer_property_getter(&mut self, property: hir::PropertyId) -> Option<TypeId> {
        let source = self.property_accessor_source(property, PropertyAccessorKind::Getter);
        self.lower_property_accessor_body(&source);
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
