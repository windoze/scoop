use super::*;

pub(crate) fn walk(
    visitor: &mut (impl BodyTransform + ?Sized),
    lowerer: &mut Lowerer,
    statement: &mut hir::Statement,
) {
    match &mut statement.kind {
        hir::StatementKind::MemberRequirementCall(call) => {
            for argument in &mut call.arguments {
                visitor.expression(lowerer, argument);
            }
        }
        hir::StatementKind::ContextScope { value, body } => {
            visitor.expression(lowerer, value);
            visitor.statements(lowerer, body);
        }
        hir::StatementKind::InitializationEnsure(_)
        | hir::StatementKind::GenericDelegateEnsure(_) => {}
        hir::StatementKind::Expr(expr) | hir::StatementKind::Throw(expr) => {
            visitor.expression(lowerer, expr)
        }
        hir::StatementKind::Return { value } => {
            visitor.expression(lowerer, value);
        }
        hir::StatementKind::LocalFunction(_)
        | hir::StatementKind::Break { .. }
        | hir::StatementKind::Continue { .. } => {}
        hir::StatementKind::ValDecl { pattern, init } => {
            visitor.pattern(lowerer, pattern);
            visitor.expression(lowerer, init);
        }
        hir::StatementKind::Assign {
            target: place,
            value,
        } => {
            match place {
                hir::AssignTarget::Local(_)
                | hir::AssignTarget::Global(_)
                | hir::AssignTarget::GenericDelegateStorage(_)
                | hir::AssignTarget::SingletonPublishedRoot(_) => {}
                hir::AssignTarget::Index { array, index } => {
                    visitor.expression(lowerer, array);
                    visitor.expression(lowerer, index);
                }
                hir::AssignTarget::Field { receiver, .. } => {
                    visitor.expression(lowerer, receiver);
                }
                hir::AssignTarget::InitializingClassField { .. } => {}
            }
            visitor.expression(lowerer, value);
        }
        hir::StatementKind::If {
            cond,
            then_body,
            else_body,
        } => {
            visitor.expression(lowerer, cond);
            visitor.statements(lowerer, then_body);
            if let Some(else_body) = else_body {
                visitor.statements(lowerer, else_body);
            }
        }
        hir::StatementKind::While {
            loop_kind: _,
            target: _,
            condition_setup,
            cond,
            body,
        } => {
            visitor.statements(lowerer, condition_setup);
            visitor.expression(lowerer, cond);
            visitor.statements(lowerer, body);
        }
        hir::StatementKind::When(when) => {
            if let Some(subject) = when.subject_mut() {
                visitor.expression(lowerer, subject);
            }
            for arm in when.arms_mut() {
                let predicate = match arm.condition {
                    hir::WhenConditionMut::Case(pattern) => {
                        visitor.pattern(lowerer, pattern);
                        None
                    }
                    hir::WhenConditionMut::Predicate(condition) => Some(condition),
                    hir::WhenConditionMut::Always => None,
                };
                for guard in predicate.into_iter().chain(arm.guard) {
                    visitor.statements(lowerer, &mut guard.setup);
                    visitor.expression(lowerer, &mut guard.condition);
                }
                visitor.statements(lowerer, arm.body);
            }
            if let Some(body) = when.else_body_mut() {
                visitor.statements(lowerer, body);
            }
        }
        hir::StatementKind::Try(try_) => {
            visitor.statements(lowerer, &mut try_.body);
            for catch in &mut try_.catches {
                visitor.statements(lowerer, &mut catch.body);
            }
            if let Some(finally_body) = &mut try_.finally_body {
                visitor.statements(lowerer, finally_body);
            }
        }
    }
}

pub(super) fn pattern(
    visitor: &mut (impl BodyTransform + ?Sized),
    lowerer: &mut Lowerer,
    pattern: &mut hir::Pattern,
) {
    match pattern {
        hir::Pattern::Literal { value, .. } => visitor.expression(lowerer, value),
        hir::Pattern::Variant { fields, .. } | hir::Pattern::Struct { fields, .. } => {
            for (_, field) in fields {
                visitor.pattern(lowerer, field);
            }
        }
        hir::Pattern::Tuple(elements) => {
            for element in elements {
                visitor.pattern(lowerer, element);
            }
        }
        hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
    }
}
