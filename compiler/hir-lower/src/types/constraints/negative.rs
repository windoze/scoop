use super::companions::{subject_exclusions_mut, subject_name};
use super::*;

impl Lowerer {
    pub(super) fn apply_negative_interface_bound(
        &mut self,
        params: &mut [hir::TypeParamDecl],
        subject: (usize, bool),
        reference: &ast::TypeRef,
        span: ast::Span,
    ) {
        let (index, companion) = subject;
        let name = subject_name(&params[index], companion);
        self.type_params_in_scope = params.to_vec();
        let Some(ty) = self.resolve_type_ref(reference) else {
            return;
        };
        if !matches!(self.types[ty], Type::Interface(_)) {
            self.error(
                reference.span,
                format!(
                    "negative bound of type parameter `{}` must be an interface, found `{}`",
                    name,
                    self.type_name(ty),
                ),
            );
            return;
        }
        let excluded = subject_exclusions_mut(&mut params[index], companion);
        if let Some(previous) = excluded.iter().find(|bound| self.types_equal(bound.ty, ty)) {
            let mut diagnostic = ast::Diagnostic::at_file(
                self.current_file,
                span,
                format!(
                    "duplicate negative interface bound `!{}` for type parameter `{}`",
                    self.type_name(ty),
                    name,
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
        excluded.push(hir::InterfaceUpperBound { ty, span });
    }

    pub(super) fn check_negative_bound_contradictions(&mut self, params: &[hir::TypeParamDecl]) {
        for parameter in params {
            self.check_negative_conditions(
                &parameter.name,
                &parameter.bounds,
                &parameter.excluded_interfaces,
            );
            if let Some(companion) = &parameter.companion {
                self.check_negative_conditions(
                    &subject_name(parameter, true),
                    &companion.bounds,
                    &companion.excluded_interfaces,
                );
            }
        }
    }

    fn check_negative_conditions(
        &mut self,
        name: &str,
        bounds: &hir::TypeParamBounds,
        excluded: &[hir::InterfaceUpperBound],
    ) {
        let hir::TypeParamBounds::Nominal(bounds) = bounds else {
            return;
        };
        for excluded in excluded {
            for positive in bounds.in_source_order() {
                if self.is_subtype(positive.ty(), excluded.ty) {
                    let mut diagnostic = ast::Diagnostic::at_file(
                        self.current_file,
                        excluded.span,
                        format!(
                            "negative interface bound `!{}` contradicts upper bound `{}` of type parameter `{}`",
                            self.type_name(excluded.ty),
                            self.type_name(positive.ty()),
                            name,
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
