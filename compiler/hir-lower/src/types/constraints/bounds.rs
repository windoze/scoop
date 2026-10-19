use super::companions::{subject_bounds_mut, subject_name};
use super::*;

impl Lowerer {
    pub(super) fn apply_type_parameter_constraint(
        &mut self,
        params: &mut [hir::TypeParamDecl],
        subject: (usize, bool),
        bound: &ast::TypeBound,
        span: ast::Span,
        function_conditions: bool,
        target: &str,
    ) {
        let (index, companion) = subject;
        let name = subject_name(&params[index], companion);
        if companion && !function_conditions {
            self.error(
                span,
                format!("companion conditions are only allowed on generic functions, not {target}"),
            );
            return;
        }
        if companion && matches!(bound, ast::TypeBound::Kind(ast::TypeParamKindBound::Value)) {
            self.error(
                span,
                format!("`{name}` is a reference singleton and cannot satisfy a value bound"),
            );
            return;
        }
        match bound {
            ast::TypeBound::ExcludedInterface(reference) => {
                if !function_conditions {
                    self.error(span, format!("negative interface bounds are only allowed on generic functions, not {target}"));
                } else {
                    self.apply_negative_interface_bound(params, subject, reference, span);
                }
            }
            ast::TypeBound::Kind(kind) => {
                let next = match kind {
                    ast::TypeParamKindBound::Value => hir::TypeParamBounds::Value { span },
                    ast::TypeParamKindBound::Ref => hir::TypeParamBounds::Ref { span },
                };
                let bounds = subject_bounds_mut(&mut params[index], companion);
                match bounds {
                    hir::TypeParamBounds::Unconstrained => *bounds = next,
                    hir::TypeParamBounds::Nominal(_) => self.error(
                        span,
                        format!(
                            "type parameter `{}` of {target} cannot combine a kind bound with nominal upper bounds",
                            name
                        ),
                    ),
                    hir::TypeParamBounds::Value { .. }
                    | hir::TypeParamBounds::Ref { .. } => self.error(
                        span,
                        format!(
                            "duplicate kind bound for type parameter `{}` of {target}",
                            name
                        ),
                    ),
                }
            }
            ast::TypeBound::Upper(reference) => {
                // Keep the complete parameter namespace visible while the
                // upper application (including F-bound arguments) resolves.
                self.type_params_in_scope = params.to_vec();
                let Some(ty) = self.resolve_type_ref(reference) else {
                    return;
                };
                let bound_name = self.type_name(ty);
                if !matches!(self.types[ty], Type::Class(_) | Type::Interface(_)) {
                    self.error(
                        reference.span,
                        format!(
                            "upper bound of type parameter `{}` must be a class or interface, found `{bound_name}`",
                            name
                        ),
                    );
                    return;
                }
                let positive = subject_bounds_mut(&mut params[index], companion);
                if matches!(positive, hir::TypeParamBounds::Unconstrained) {
                    *positive = hir::TypeParamBounds::Nominal(hir::NominalBounds {
                        class: None,
                        interfaces: Vec::new(),
                    });
                }
                let hir::TypeParamBounds::Nominal(bounds) = positive else {
                    self.error(
                        span,
                        format!(
                            "type parameter `{}` of {target} cannot combine nominal upper bounds with a kind bound",
                            name
                        ),
                    );
                    return;
                };
                match self.types[ty] {
                    Type::Class(_) => {
                        if bounds.class.is_some() {
                            self.error(
                                span,
                                format!(
                                    "type parameter `{}` of {target} cannot have more than one class upper bound; found `{bound_name}`",
                                    name
                                ),
                            );
                        } else {
                            bounds.class = Some(hir::ClassUpperBound { ty, span });
                        }
                    }
                    Type::Interface(_) => {
                        let duplicate = bounds
                            .interfaces
                            .iter()
                            .any(|bound| self.types_equal(bound.ty, ty));
                        if duplicate {
                            self.error(
                                span,
                                format!(
                                    "duplicate interface upper bound `{bound_name}` for type parameter `{}` of {target}",
                                    name
                                ),
                            );
                        } else {
                            bounds
                                .interfaces
                                .push(hir::InterfaceUpperBound { ty, span });
                        }
                    }
                    _ => {
                        unreachable!("a nominal upper bound has a checked class or interface type")
                    }
                }
            }
        }
        self.type_params_in_scope = params.to_vec();
    }
}
