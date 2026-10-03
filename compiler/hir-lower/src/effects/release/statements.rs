use super::*;

impl Lowerer {
    pub(super) fn release_body(
        &self,
        body: &hir::Body,
        values: &ReleaseValueFacts,
        facts: &mut BodyFacts,
    ) {
        facts.require(values.values(self, body.locals.values().map(|local| local.ty)));
        self.release_statements(&body.statements, values, facts);
    }

    pub(super) fn release_statements(
        &self,
        statements: &[hir::Statement],
        values: &ReleaseValueFacts,
        facts: &mut BodyFacts,
    ) {
        for statement in statements {
            match &statement.kind {
                hir::StatementKind::Expr(expression)
                | hir::StatementKind::ValDecl {
                    init: expression, ..
                } => {
                    self.release_expression(expression, values, facts);
                }
                hir::StatementKind::Return { value } => {
                    if let Some(value) = value {
                        self.release_expression(value, values, facts);
                    }
                }
                hir::StatementKind::Assign { target, value } => {
                    if !match target {
                        hir::AssignTarget::Local(_) => true,
                        hir::AssignTarget::Global(global) => self.release_global(*global),
                        _ => false,
                    } {
                        facts.requirements = None;
                    }
                    self.release_expression(value, values, facts);
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    self.release_expression(cond, values, facts);
                    self.release_statements(then_body, values, facts);
                    if let Some(body) = else_body {
                        self.release_statements(body, values, facts);
                    }
                }
                hir::StatementKind::While {
                    condition_setup,
                    cond,
                    body,
                    ..
                } => {
                    self.release_statements(condition_setup, values, facts);
                    self.release_expression(cond, values, facts);
                    self.release_statements(body, values, facts);
                }
                hir::StatementKind::When(when) => {
                    self.release_expression(&when.subject, values, facts);
                    for arm in &when.arms {
                        if let Some(guard) = &arm.guard {
                            self.release_statements(&guard.setup, values, facts);
                            self.release_expression(&guard.condition, values, facts);
                        }
                        self.release_statements(&arm.body, values, facts);
                    }
                    if let hir::WhenFallback::Else(body) = &when.fallback {
                        self.release_statements(body, values, facts);
                    }
                }
                hir::StatementKind::LocalFunction(_)
                | hir::StatementKind::Break { .. }
                | hir::StatementKind::Continue { .. } => {}
                hir::StatementKind::InitializationEnsure(_)
                | hir::StatementKind::GenericDelegateEnsure(_)
                | hir::StatementKind::Try(_)
                | hir::StatementKind::Throw(_) => facts.requirements = None,
            }
        }
    }

    pub(super) fn release_global(&self, global: hir::GlobalId) -> bool {
        matches!(
            &self.globals[global].storage,
            hir::GlobalStorage::Local {
                thread_local: false,
                ..
            } | hir::GlobalStorage::Extern {
                thread_local: false,
                ..
            } | hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::EncodedStaticValue { .. },
            }
        )
    }
}
