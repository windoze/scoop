use super::*;

impl Lowerer {
    pub(crate) fn validate_gc_control_intrinsics(&mut self, files: &[ast::SourceFile]) {
        for (kind, result) in [
            (hir::IntrinsicFunctionKind::GcCollect, self.unit),
            (
                hir::IntrinsicFunctionKind::GcStats,
                self.integer_type(hir::IntegerKind::UNSIGNED_64),
            ),
        ] {
            let function = if self.current_cone() == scoop_identity::ConeIdentity::CORE {
                self.require_intrinsic(kind, files)
            } else {
                self.intrinsic_functions
                    .get(&kind)
                    .map(|(function, _)| *function)
            };
            let Some(function) = function else {
                continue;
            };
            let signature = &self.signatures[&function];
            let valid = !signature.is_suspend
                && signature.type_params.is_empty()
                && signature.owner_type_param_count == 0
                && signature.params.is_empty()
                && signature.return_ty == result
                && !self.function_owner.contains_key(&function)
                && !self.extension_receivers.contains_key(&function);
            if !valid {
                self.current_file = self.function_files[&function];
                self.error(
                    self.functions[function].span,
                    format!("malformed core GC control intrinsic `{}`", kind.name(),),
                );
            }
        }
    }
}
