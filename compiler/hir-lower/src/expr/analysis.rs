use super::*;

pub(super) fn block_contains_return(block: &ast::Block) -> bool {
    block.statements.iter().any(statement_contains_return)
}

fn statement_contains_return(statement: &ast::Statement) -> bool {
    match &statement.kind {
        ast::StatementKind::Return { .. } => true,
        ast::StatementKind::LocalFunction(_) => false,
        ast::StatementKind::Expr(expr) | ast::StatementKind::Throw(expr) => {
            expr_contains_return(expr)
        }
        ast::StatementKind::ValDecl(decl) => expr_contains_return(&decl.init),
        ast::StatementKind::LocalDelegatedProperty(decl) => expr_contains_return(&decl.expression),
        ast::StatementKind::Assign(assign) => {
            place_contains_return(&assign.target) || expr_contains_return(&assign.value)
        }
        ast::StatementKind::If(if_) => {
            expr_contains_return(&if_.cond)
                || block_contains_return(&if_.then_block)
                || if_.else_block.as_ref().is_some_and(block_contains_return)
        }
        ast::StatementKind::While(while_) => {
            expr_contains_return(&while_.cond) || block_contains_return(&while_.body)
        }
        ast::StatementKind::For(for_) => {
            expr_contains_return(&for_.iterable) || block_contains_return(&for_.body)
        }
        ast::StatementKind::Break | ast::StatementKind::Continue => false,
        ast::StatementKind::Block(block) | ast::StatementKind::SafetyBlock { block, .. } => {
            block_contains_return(block)
        }
        ast::StatementKind::When(when) => {
            when.subject.initializer().is_some_and(expr_contains_return)
                || when.arms.iter().any(|arm| {
                    (match &arm.condition {
                        ast::WhenArmCondition::Conditions(conditions) => {
                            conditions.iter().any(|condition| match condition {
                                ast::WhenCondition::Expression(expr) => expr_contains_return(expr),
                                ast::WhenCondition::In { collection, .. } => {
                                    expr_contains_return(collection)
                                }
                                ast::WhenCondition::Is { .. } => false,
                            })
                        }
                        ast::WhenArmCondition::Case(_) | ast::WhenArmCondition::Else => false,
                    }) || arm.guard.as_ref().is_some_and(expr_contains_return)
                        || block_contains_return(&arm.body)
                })
                || when.else_body.as_ref().is_some_and(block_contains_return)
        }
        ast::StatementKind::Try(try_) => {
            block_contains_return(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| block_contains_return(&catch.body))
                || try_
                    .finally_body
                    .as_ref()
                    .is_some_and(block_contains_return)
        }
    }
}

pub(crate) fn expr_contains_return(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::TypeQualifier(_) => false,
        ast::Expr::ContextScope { value, body, .. } => {
            expr_contains_return(value) || block_contains_return(body)
        }
        ast::Expr::InterpolatedString { parts, .. } => parts.iter().any(|part| match part {
            ast::StringPart::Text { .. } => false,
            ast::StringPart::Expression { value, .. } => expr_contains_return(value),
        }),
        // A nested callable owns its own return target.
        ast::Expr::Lambda { .. } | ast::Expr::AnonymousFunction { .. } => false,
        ast::Expr::CallableReference { receiver, .. } => {
            receiver.as_deref().is_some_and(expr_contains_return)
        }
        ast::Expr::Return { .. } => true,
        ast::Expr::Throw { value, .. } => expr_contains_return(value),
        ast::Expr::TupleLiteral { elements, .. } | ast::Expr::ArrayLiteral { elements, .. } => {
            elements.iter().any(expr_contains_return)
        }
        ast::Expr::StructInit { args, .. } => args
            .iter()
            .any(|argument| expr_contains_return(&argument.expression)),
        ast::Expr::FieldAccess(access) => expr_contains_return(&access.receiver),
        ast::Expr::CopyUpdate { base, fields, .. } => {
            expr_contains_return(base)
                || fields
                    .iter()
                    .any(|field| expr_contains_return(&field.value))
        }
        ast::Expr::Call(call) => call
            .args
            .iter()
            .any(|argument| expr_contains_return(&argument.expression)),
        ast::Expr::Invoke { callee, args, .. } => {
            expr_contains_return(callee)
                || args
                    .iter()
                    .any(|argument| expr_contains_return(&argument.expression))
        }
        ast::Expr::InfixCall { lhs, rhs, .. }
        | ast::Expr::Binary { lhs, rhs, .. }
        | ast::Expr::Elvis { lhs, rhs, .. } => {
            expr_contains_return(lhs) || expr_contains_return(rhs)
        }
        ast::Expr::Unary { operand, .. }
        | ast::Expr::NullAssert { operand, .. }
        | ast::Expr::Is { operand, .. }
        | ast::Expr::Cast { operand, .. } => expr_contains_return(operand),
        ast::Expr::MethodCall { receiver, args, .. } => {
            expr_contains_return(receiver)
                || args
                    .iter()
                    .any(|argument| expr_contains_return(&argument.expression))
        }
        ast::Expr::SuperMethodCall { args, .. } => args
            .iter()
            .any(|argument| expr_contains_return(&argument.expression)),
        ast::Expr::QualifiedInterfaceSuperMethodCall { args, .. } => args
            .iter()
            .any(|argument| expr_contains_return(&argument.expression)),
        ast::Expr::Index {
            receiver, indices, ..
        } => expr_contains_return(receiver) || indices.iter().any(expr_contains_return),
        ast::Expr::Update { place, .. } => place_contains_return(place),
        ast::Expr::If(if_) => {
            expr_contains_return(&if_.cond)
                || block_contains_return(&if_.then_block)
                || if_.else_block.as_ref().is_some_and(block_contains_return)
        }
        ast::Expr::When(when) => {
            when.subject.initializer().is_some_and(expr_contains_return)
                || when.arms.iter().any(|arm| {
                    (match &arm.condition {
                        ast::WhenArmCondition::Conditions(conditions) => {
                            conditions.iter().any(|condition| match condition {
                                ast::WhenCondition::Expression(expr) => expr_contains_return(expr),
                                ast::WhenCondition::In { collection, .. } => {
                                    expr_contains_return(collection)
                                }
                                ast::WhenCondition::Is { .. } => false,
                            })
                        }
                        ast::WhenArmCondition::Case(_) | ast::WhenArmCondition::Else => false,
                    }) || arm.guard.as_ref().is_some_and(expr_contains_return)
                        || block_contains_return(&arm.body)
                })
                || when.else_body.as_ref().is_some_and(block_contains_return)
        }
        ast::Expr::Try(try_) => {
            block_contains_return(&try_.body)
                || try_
                    .catches
                    .iter()
                    .any(|catch| block_contains_return(&catch.body))
                || try_
                    .finally_body
                    .as_ref()
                    .is_some_and(block_contains_return)
        }
        ast::Expr::StringLiteral { .. }
        | ast::Expr::IntLiteral(_)
        | ast::Expr::FloatLiteral(_)
        | ast::Expr::CharLiteral { .. }
        | ast::Expr::BoolLiteral { .. }
        | ast::Expr::UnitLiteral { .. }
        | ast::Expr::Var(_)
        | ast::Expr::This { .. }
        | ast::Expr::QualifiedInterfaceSuperAccess { .. } => false,
    }
}

fn place_contains_return(place: &ast::PlaceExpr) -> bool {
    match place {
        ast::PlaceExpr::Name(_) => false,
        ast::PlaceExpr::Field { receiver, .. } => expr_contains_return(receiver),
        ast::PlaceExpr::Index {
            receiver, indices, ..
        } => expr_contains_return(receiver) || indices.iter().any(expr_contains_return),
        ast::PlaceExpr::QualifiedInterfaceSuperProperty { .. } => false,
    }
}
