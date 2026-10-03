use scoop_hir as hir;

use super::ReferenceCollector;

impl ReferenceCollector<'_> {
    pub(super) fn statement(&mut self, statement: &hir::Statement) {
        let origin = self.at(statement.span);
        match &statement.kind {
            hir::StatementKind::GenericDelegateEnsure(_) => self.direct_delegate_storage(origin),
            hir::StatementKind::InitializationEnsure(_)
            | hir::StatementKind::Break { .. }
            | hir::StatementKind::Continue { .. } => {}
            hir::StatementKind::Expr(value) | hir::StatementKind::Throw(value) => {
                self.expression(value);
            }
            hir::StatementKind::LocalFunction(function) => {
                self.local_function_descriptor(*function);
            }
            hir::StatementKind::Return { value } => {
                if let Some(value) = value {
                    self.expression(value);
                }
            }
            hir::StatementKind::ValDecl { pattern, init } => {
                self.pattern(pattern, origin);
                self.expression(init);
            }
            hir::StatementKind::Assign { target, value } => {
                self.assign_target(target, origin);
                self.expression(value);
            }
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                self.expression(cond);
                self.statements(then_body);
                if let Some(else_body) = else_body {
                    self.statements(else_body);
                }
            }
            hir::StatementKind::While {
                condition_setup,
                cond,
                body,
                ..
            } => {
                self.statements(condition_setup);
                self.expression(cond);
                self.statements(body);
            }
            hir::StatementKind::When(value) => self.when(value, origin),
            hir::StatementKind::Try(value) => self.try_(value),
        }
    }

    pub(super) fn statements(&mut self, statements: &[hir::Statement]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    fn assign_target(&mut self, target: &hir::AssignTarget, origin: hir::DefinitionOrigin) {
        match target {
            hir::AssignTarget::GenericDelegateStorage(_) => self.direct_delegate_storage(origin),
            hir::AssignTarget::Local(_) => {}
            hir::AssignTarget::Global(global) => self.global(*global, origin),
            hir::AssignTarget::SingletonPublishedRoot(_) => {}
            hir::AssignTarget::Index { array, index } => {
                self.expression(array);
                self.expression(index);
            }
            hir::AssignTarget::Field { receiver, field } => {
                self.expression(receiver);
                self.field_use(*field, origin);
            }
            hir::AssignTarget::InitializingClassField { .. } => {}
        }
    }

    fn pattern(&mut self, pattern: &hir::Pattern, origin: hir::DefinitionOrigin) {
        match pattern {
            hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
            hir::Pattern::Literal {
                equality,
                subject_ty,
                value,
            } => {
                let origin = value.origin.definition();
                if let hir::LiteralPatternEquality::Ordinary { equals } = equality {
                    self.callable_target(*equals, origin);
                }
                self.type_reference(*subject_ty, origin);
                self.expression(value);
            }
            hir::Pattern::Variant {
                application,
                fields,
                ..
            } => {
                let owner = application.owner;
                self.type_reference(owner, origin);
                for (_, field) in fields {
                    self.pattern(field, origin);
                }
            }
            hir::Pattern::Tuple(elements) => {
                for element in elements {
                    self.pattern(element, origin);
                }
            }
            hir::Pattern::Struct { owner, fields } => {
                self.type_reference(*owner, origin);
                for (_, field) in fields {
                    self.pattern(field, origin);
                }
            }
        }
    }

    fn when(&mut self, value: &hir::When, origin: hir::DefinitionOrigin) {
        self.expression(&value.subject);
        for arm in &value.arms {
            let arm_origin = self.at(arm.span);
            self.pattern(&arm.pattern, arm_origin);
            if let Some(guard) = &arm.guard {
                self.statements(&guard.setup);
                self.expression(&guard.condition);
            }
            self.statements(&arm.body);
        }
        match &value.fallback {
            hir::WhenFallback::Else(body) => self.statements(body),
            hir::WhenFallback::Impossible(hir::ExhaustivenessProof::IrrefutableArm {
                subject_ty,
            })
            | hir::WhenFallback::Impossible(hir::ExhaustivenessProof::PatternMatrix {
                subject_ty,
            })
            | hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix {
                subject_ty,
            }) => self.type_reference(*subject_ty, origin),
        }
    }

    fn try_(&mut self, value: &hir::Try) {
        self.statements(&value.body);
        for catch in &value.catches {
            let origin = self.at(catch.span);
            self.type_reference(catch.ty, origin);
            self.statements(&catch.body);
        }
        if let Some(finally_body) = &value.finally_body {
            self.statements(finally_body);
        }
    }
}
