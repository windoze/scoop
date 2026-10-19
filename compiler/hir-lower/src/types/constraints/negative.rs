use super::*;

impl Lowerer {
    pub(super) fn apply_negative_interface_bound(
        &mut self,
        params: &mut [hir::TypeParamDecl],
        index: usize,
        reference: &ast::TypeRef,
        span: ast::Span,
    ) {
        self.type_params_in_scope = params.to_vec();
        let Some(ty) = self.resolve_type_ref(reference) else {
            return;
        };
        if !matches!(self.types[ty], Type::Interface(_)) {
            self.error(
                reference.span,
                format!(
                    "negative bound of type parameter `{}` must be an interface, found `{}`",
                    params[index].name,
                    self.type_name(ty),
                ),
            );
            return;
        }
        if let Some(previous) = params[index]
            .excluded_interfaces
            .iter()
            .find(|bound| self.types_equal(bound.ty, ty))
        {
            let mut diagnostic = ast::Diagnostic::at_file(
                self.current_file,
                span,
                format!(
                    "duplicate negative interface bound `!{}` for type parameter `{}`",
                    self.type_name(ty),
                    params[index].name,
                ),
            );
            diagnostic.notes.push(ast::DiagnosticNote::at(
                self.current_file,
                previous.span,
                "previous negative bound is here",
            ));
            self.diagnostics.push(diagnostic);
            return;
        }
        params[index]
            .excluded_interfaces
            .push(hir::InterfaceUpperBound { ty, span });
    }

    pub(super) fn check_negative_bound_contradictions(&mut self, params: &[hir::TypeParamDecl]) {
        for parameter in params {
            for excluded in &parameter.excluded_interfaces {
                for positive in parameter.nominal_bounds_in_source_order() {
                    if self.is_subtype(positive.ty(), excluded.ty) {
                        let mut diagnostic = ast::Diagnostic::at_file(
                            self.current_file,
                            excluded.span,
                            format!(
                                "negative interface bound `!{}` contradicts upper bound `{}` of type parameter `{}`",
                                self.type_name(excluded.ty),
                                self.type_name(positive.ty()),
                                parameter.name,
                            ),
                        );
                        diagnostic.notes.push(ast::DiagnosticNote::at(
                            self.current_file,
                            positive.span(),
                            "conflicting upper bound is here",
                        ));
                        self.diagnostics.push(diagnostic);
                    }
                }
            }
        }
    }
}
