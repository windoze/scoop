use super::*;
mod expression;
mod pattern;

impl Concretizer<'_> {
    pub(super) fn lower_body(
        &mut self,
        source: &export::Body,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Body, Vec<concrete::LocalId>) {
        let (locals, local_map) = self.lower_locals(&source.locals, substitution);
        let statements = source
            .statements
            .iter()
            .filter_map(|statement| self.lower_statement(statement, substitution, &local_map))
            .collect();
        (concrete::Body { locals, statements }, local_map)
    }

    pub(super) fn lower_locals(
        &mut self,
        source: &Arena<export::Local>,
        substitution: &[concrete::TypeId],
    ) -> (Arena<concrete::Local>, Vec<concrete::LocalId>) {
        let mut locals = Arena::new();
        let mut local_map = Vec::with_capacity(source.len());
        for (source_id, source_local) in source.iter() {
            let id = locals.alloc(concrete::Local {
                binding: concrete::BindingId::from_raw(source_local.binding.into_raw()),
                name: source_local.name.clone(),
                ty: self.lower_type(source_local.ty, substitution),
                mutable: source_local.mutable,
            });
            assert_eq!(id.into_raw(), source_id.into_raw());
            local_map.push(id);
        }
        (locals, local_map)
    }

    pub(super) fn lower_statement(
        &mut self,
        source: &export::Statement,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> Option<concrete::Statement> {
        let kind = match &source.kind {
            export::StatementKind::Expr(expr) => {
                concrete::StatementKind::Expr(self.lower_expr(expr, substitution, locals))
            }
            // This marker has no runtime semantics. Concrete local-function
            // entities are requested by direct calls/references instead.
            export::StatementKind::LocalFunction(_) => return None,
            export::StatementKind::Return { value } => concrete::StatementKind::Return {
                value: value
                    .as_ref()
                    .map(|value| self.lower_expr(value, substitution, locals)),
            },
            export::StatementKind::ValDecl { pattern, init } => {
                let init = self.lower_expr(init, substitution, locals);
                let pattern = self.lower_pattern(pattern, init.ty, substitution, locals);
                concrete::StatementKind::ValDecl { pattern, init }
            }
            export::StatementKind::Assign { target, value } => concrete::StatementKind::Assign {
                target: self.lower_assign_target(target, substitution, locals),
                value: self.lower_expr(value, substitution, locals),
            },
            export::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => concrete::StatementKind::If {
                cond: self.lower_expr(cond, substitution, locals),
                then_body: self.lower_statements(then_body, substitution, locals),
                else_body: else_body
                    .as_ref()
                    .map(|body| self.lower_statements(body, substitution, locals)),
            },
            export::StatementKind::While {
                condition_setup,
                cond,
                body,
            } => concrete::StatementKind::While {
                condition_setup: self.lower_statements(condition_setup, substitution, locals),
                cond: self.lower_expr(cond, substitution, locals),
                body: self.lower_statements(body, substitution, locals),
            },
            export::StatementKind::When(when) => {
                concrete::StatementKind::When(self.lower_when(when, substitution, locals))
            }
            export::StatementKind::Try(try_) => concrete::StatementKind::Try(concrete::Try {
                body: self.lower_statements(&try_.body, substitution, locals),
                catches: try_
                    .catches
                    .iter()
                    .map(|catch| concrete::CatchClause {
                        local: self.lower_local(catch.local, locals),
                        ty: self.lower_type(catch.ty, substitution),
                        body: self.lower_statements(&catch.body, substitution, locals),
                        span: catch.span,
                    })
                    .collect(),
                finally_body: try_
                    .finally_body
                    .as_ref()
                    .map(|body| self.lower_statements(body, substitution, locals)),
            }),
            export::StatementKind::Throw(expr) => {
                concrete::StatementKind::Throw(self.lower_expr(expr, substitution, locals))
            }
        };
        Some(concrete::Statement {
            kind,
            span: source.span,
        })
    }

    pub(super) fn lower_statements(
        &mut self,
        source: &[export::Statement],
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> Vec<concrete::Statement> {
        source
            .iter()
            .filter_map(|statement| self.lower_statement(statement, substitution, locals))
            .collect()
    }

    pub(super) fn lower_assign_target(
        &mut self,
        source: &export::AssignTarget,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::AssignTarget {
        match source {
            export::AssignTarget::Local(local) => {
                concrete::AssignTarget::Local(self.lower_local(*local, locals))
            }
            export::AssignTarget::Global(global) => {
                concrete::AssignTarget::Global(self.global_map[global])
            }
            export::AssignTarget::Index { array, index } => concrete::AssignTarget::Index {
                array: Box::new(self.lower_expr(array, substitution, locals)),
                index: Box::new(self.lower_expr(index, substitution, locals)),
            },
            export::AssignTarget::Field { receiver, field } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                let field = self.lower_field_ref(*field, substitution);
                concrete::AssignTarget::Field {
                    receiver: Box::new(receiver),
                    field,
                }
            }
        }
    }

    pub(super) fn lower_when(
        &mut self,
        source: &export::When,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::When {
        let subject = self.lower_expr(&source.subject, substitution, locals);
        let arms = source
            .arms
            .iter()
            .map(|arm| concrete::WhenArm {
                pattern: self.lower_pattern(&arm.pattern, subject.ty, substitution, locals),
                guard: arm.guard.as_ref().map(|guard| concrete::WhenGuard {
                    setup: self.lower_statements(&guard.setup, substitution, locals),
                    condition: self.lower_expr(&guard.condition, substitution, locals),
                }),
                body: self.lower_statements(&arm.body, substitution, locals),
                span: arm.span,
            })
            .collect();
        concrete::When {
            subject,
            arms,
            else_body: source
                .else_body
                .as_ref()
                .map(|body| self.lower_statements(body, substitution, locals)),
        }
    }
}
