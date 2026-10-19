//! Validate the bounded String representation operations at the source boundary.
use super::*;

impl Lowerer {
    pub(crate) fn validate_string_intrinsics(&mut self, files: &[ast::SourceFile]) {
        let Ok(character) = self.core_character_type() else {
            return;
        };
        let string = self.intern_type(hir::Type::String);
        let long = self.integer_type(hir::IntegerKind::SIGNED_64);
        let byte = self.integer_type(hir::IntegerKind::SIGNED_8);
        for kind in hir::StringIntrinsic::ALL {
            let Some(function) =
                self.require_intrinsic(hir::IntrinsicFunctionKind::String(kind), files)
            else {
                continue;
            };
            let (parameters, result) = match kind {
                hir::StringIntrinsic::ByteLength => (vec![string], long),
                hir::StringIntrinsic::ByteAt => (vec![string, long], byte),
                hir::StringIntrinsic::CharacterAtByte => (vec![string, long], character),
            };
            let signature = &self.signatures[&function];
            let valid = !signature.is_suspend
                && signature.type_params.is_empty()
                && signature.owner_type_param_count == 0
                && signature.params.iter().map(|p| p.ty).eq(parameters)
                && signature.return_ty == result
                && !self.function_owner.contains_key(&function)
                && !self.extension_receivers.contains_key(&function);
            if !valid {
                self.current_file = self.function_files[&function];
                self.error(
                    self.functions[function].span,
                    format!("malformed core String intrinsic `{}`", kind.name()),
                );
            }
        }
    }
}
