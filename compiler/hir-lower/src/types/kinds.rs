use super::*;

impl Lowerer {
    pub(crate) fn check_type_argument_kinds(
        &mut self,
        params: &[hir::TypeParamDecl],
        args: &[TypeId],
        span: ast::Span,
        target: &str,
    ) -> bool {
        let mut valid = true;
        for (param, &arg) in params.iter().zip(args) {
            if !self.type_satisfies_kind(arg, param.kind()) {
                let required = match param.kind() {
                    hir::TypeParamKind::Any => continue,
                    hir::TypeParamKind::Value => "value",
                    hir::TypeParamKind::Ref => "ref",
                };
                let found = self.type_name(arg);
                self.error(
                    span,
                    format!(
                        "type argument `{found}` for `{}` of {target} must satisfy `{required}`",
                        param.name
                    ),
                );
                valid = false;
            }
        }
        for (param, &arg) in params.iter().zip(args) {
            for bound in param.interface_bounds() {
                let bound = self.interface_applications[bound.application].canonical_type;
                let required = self.instantiate_ty(bound, args);
                if self.is_subtype(arg, required) {
                    continue;
                }
                let found = self.type_name(arg);
                let required = self.type_name(required);
                self.error(
                    span,
                    format!(
                        "type argument `{found}` for `{}` of {target} must satisfy interface upper bound `{required}`",
                        param.name
                    ),
                );
                valid = false;
            }
        }
        valid
    }

    pub(crate) fn type_arguments_satisfy_kinds(
        &mut self,
        params: &[hir::TypeParamDecl],
        args: &[TypeId],
    ) -> bool {
        for (param, &arg) in params.iter().zip(args) {
            if !self.type_satisfies_kind(arg, param.kind()) {
                return false;
            }
            for bound in param.interface_bounds() {
                let bound = self.interface_applications[bound.application].canonical_type;
                let required = self.instantiate_ty(bound, args);
                if !self.is_subtype(arg, required) {
                    return false;
                }
            }
        }
        true
    }

    pub(crate) fn type_satisfies_kind(&self, ty: TypeId, required: hir::TypeParamKind) -> bool {
        if required == hir::TypeParamKind::Any {
            return true;
        }
        if let Type::Param(index) = self.types[ty] {
            let actual = self
                .type_params_in_scope
                .iter()
                .find(|parameter| parameter.id == index)
                .map(|param| param.kind())
                .unwrap_or(hir::TypeParamKind::Any);
            return actual == required;
        }
        match required {
            hir::TypeParamKind::Any => true,
            hir::TypeParamKind::Value => self.is_value_ty(ty),
            hir::TypeParamKind::Ref => self.is_ref_ty(ty),
        }
    }
}
