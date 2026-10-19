use super::*;

impl Lowerer {
    pub(super) fn requirement_receiver(
        &mut self,
        receiver: &ast::Ident,
        explicit_companion: bool,
        virtuals: &HashMap<String, TypeId>,
        parameters: &[hir::TypeParamDecl],
        own: &HashMap<String, usize>,
    ) -> Option<TypeId> {
        if !explicit_companion && let Some(&ty) = virtuals.get(&receiver.text) {
            return Some(ty);
        }
        let Some(&index) = own.get(&receiver.text) else {
            self.error(receiver.span, "a member requirement receiver must name a virtual parameter or this function's type parameter".into());
            return None;
        };
        Some(self.intern_type(Type::Companion(parameters[index].id)))
    }
}
