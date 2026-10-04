//! Validate the four source scalar operations required by Char.

use super::*;

impl Lowerer {
    pub(super) fn validate_char_intrinsics(&mut self, files: &[ast::SourceFile]) {
        let Ok(character) = self.core_character_type() else {
            return;
        };
        let int = self.integer_type(hir::IntegerKind::SIGNED_32);
        let long = self.integer_type(hir::IntegerKind::SIGNED_64);
        for kind in hir::CharIntrinsic::ALL {
            let Some(function) =
                self.require_intrinsic(hir::IntrinsicFunctionKind::Char(kind), files)
            else {
                continue;
            };
            let (owner, result, operator, arity) = match kind {
                hir::CharIntrinsic::Code => (hir::IntrinsicTypeKind::Char, int, None, 0),
                hir::CharIntrinsic::FromCodeUnchecked => (
                    hir::IntrinsicTypeKind::Integer(hir::IntegerKind::SIGNED_32),
                    character,
                    None,
                    0,
                ),
                hir::CharIntrinsic::Equals => (
                    hir::IntrinsicTypeKind::Char,
                    self.boolean,
                    Some(hir::OperatorKind::Equals),
                    1,
                ),
                hir::CharIntrinsic::CompareTo => (
                    hir::IntrinsicTypeKind::Char,
                    long,
                    Some(hir::OperatorKind::CompareTo),
                    1,
                ),
            };
            let signature = &self.signatures[&function];
            let valid = self.function_has_intrinsic_owner(function, owner)
                && signature.params.len() == arity
                && signature
                    .params
                    .iter()
                    .all(|parameter| parameter.ty == character)
                && signature.type_params.is_empty()
                && !signature.is_suspend
                && signature.return_ty == result
                && signature.modifiers.operator == operator
                && !signature.modifiers.is_infix;
            if !valid {
                self.current_file = self.function_files[&function];
                self.error(
                    self.functions[function].span,
                    format!("malformed core Char intrinsic `{}`", kind.name()),
                );
            }
        }
    }
}
