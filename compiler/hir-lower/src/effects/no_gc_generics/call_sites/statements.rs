use super::*;

impl Lowerer {
    pub(in crate::effects) fn collect_generic_calls_in_statements(
        &self,
        statements: &[hir::Statement],
        out: &mut Vec<GenericCall>,
    ) {
        for statement in statements {
            match &statement.kind {
                hir::StatementKind::InitializationEnsure(_)
                | hir::StatementKind::GenericDelegateEnsure(_) => {}
                hir::StatementKind::Expr(expr) | hir::StatementKind::Throw(expr) => {
                    self.collect_generic_calls_in_expr(expr, out);
                }
                hir::StatementKind::LocalFunction(_)
                | hir::StatementKind::Break { .. }
                | hir::StatementKind::Continue { .. } => {}
                hir::StatementKind::Return { value } => {
                    if let Some(value) = value {
                        self.collect_generic_calls_in_expr(value, out);
                    }
                }
                hir::StatementKind::ValDecl { pattern, init } => {
                    self.collect_generic_calls_in_pattern(pattern, out);
                    self.collect_generic_calls_in_expr(init, out);
                }
                hir::StatementKind::Assign { target, value } => {
                    match target {
                        hir::AssignTarget::Local(_)
                        | hir::AssignTarget::Global(_)
                        | hir::AssignTarget::GenericDelegateStorage(_)
                        | hir::AssignTarget::SingletonPublishedRoot(_) => {}
                        hir::AssignTarget::Index { array, index } => {
                            self.collect_generic_calls_in_expr(array, out);
                            self.collect_generic_calls_in_expr(index, out);
                        }
                        hir::AssignTarget::Field { receiver, .. } => {
                            self.collect_generic_calls_in_expr(receiver, out);
                        }
                        hir::AssignTarget::InitializingClassField { .. } => {}
                    }
                    self.collect_generic_calls_in_expr(value, out);
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    self.collect_generic_calls_in_expr(cond, out);
                    self.collect_generic_calls_in_statements(then_body, out);
                    if let Some(else_body) = else_body {
                        self.collect_generic_calls_in_statements(else_body, out);
                    }
                }
                hir::StatementKind::While {
                    target: _,
                    condition_setup,
                    cond,
                    body,
                } => {
                    self.collect_generic_calls_in_statements(condition_setup, out);
                    self.collect_generic_calls_in_expr(cond, out);
                    self.collect_generic_calls_in_statements(body, out);
                }
                hir::StatementKind::When(when) => {
                    self.collect_generic_calls_in_expr(&when.subject, out);
                    for arm in &when.arms {
                        self.collect_generic_calls_in_pattern(&arm.pattern, out);
                        if let Some(guard) = &arm.guard {
                            self.collect_generic_calls_in_statements(&guard.setup, out);
                            self.collect_generic_calls_in_expr(&guard.condition, out);
                        }
                        self.collect_generic_calls_in_statements(&arm.body, out);
                    }
                    if let hir::WhenFallback::Else(body) = &when.fallback {
                        self.collect_generic_calls_in_statements(body, out);
                    }
                }
                hir::StatementKind::Try(try_) => {
                    self.collect_generic_calls_in_statements(&try_.body, out);
                    for catch in &try_.catches {
                        self.collect_generic_calls_in_statements(&catch.body, out);
                    }
                    if let Some(finally_body) = &try_.finally_body {
                        self.collect_generic_calls_in_statements(finally_body, out);
                    }
                }
            }
        }
    }

    pub(in crate::effects) fn collect_generic_calls_in_pattern(
        &self,
        pattern: &hir::Pattern,
        out: &mut Vec<GenericCall>,
    ) {
        match pattern {
            hir::Pattern::Literal { value, .. } => self.collect_generic_calls_in_expr(value, out),
            hir::Pattern::Variant { fields, .. }
            | hir::Pattern::ImportedVariant { fields, .. }
            | hir::Pattern::Struct { fields, .. } => {
                for (_, pattern) in fields {
                    self.collect_generic_calls_in_pattern(pattern, out);
                }
            }
            hir::Pattern::Tuple(elements) => {
                for pattern in elements {
                    self.collect_generic_calls_in_pattern(pattern, out);
                }
            }
            hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
        }
    }
}
