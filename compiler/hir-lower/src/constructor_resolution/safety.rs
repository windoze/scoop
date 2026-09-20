use super::*;

impl Lowerer {
    pub(super) fn check_constructor_call_safety(
        &mut self,
        source: NominalConstructorSource,
        span: ast::Span,
    ) {
        let (safety, name) = match source {
            NominalConstructorSource::Class(id) => {
                let constructor = &self.class_constructors[id];
                (constructor.safety, &self.classes[constructor.owner].name)
            }
            NominalConstructorSource::Struct(id) => {
                let constructor = &self.struct_constructors[id];
                (constructor.safety, &self.structs[constructor.owner].name)
            }
            NominalConstructorSource::IntrinsicClass(_) | NominalConstructorSource::Variant(_) => {
                return;
            }
        };
        if safety == hir::Safety::Unsafe {
            self.require_unsafe_operation(span, &format!("unsafe constructor `{name}`"));
        }
    }
}
