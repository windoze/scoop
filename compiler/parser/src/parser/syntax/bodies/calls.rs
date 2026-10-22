use super::*;
use ast::{Expr, RecoveredCallTarget as Target};

impl Parser {
    pub(crate) fn finish_recovered_call(
        &mut self,
        expression: Expr,
        arguments: Option<ast::RecoveredCallArguments>,
        target_complete: bool,
    ) -> Expr {
        let Some(mut arguments) = arguments else {
            return expression;
        };
        let (target, type_args) = match &expression {
            Expr::Call(call) => (Target::Named(call.callee.clone()), &call.type_args),
            Expr::MethodCall {
                receiver,
                name,
                navigation,
                type_args,
                ..
            } => (
                Target::Member {
                    receiver: receiver.clone(),
                    name: name.clone(),
                    navigation: *navigation,
                },
                type_args,
            ),
            Expr::Invoke {
                callee, type_args, ..
            } => (Target::Invoke(callee.clone()), type_args),
            Expr::SuperMethodCall {
                super_span,
                name,
                type_args,
                ..
            } => (
                Target::Super {
                    super_span: *super_span,
                    name: name.clone(),
                },
                type_args,
            ),
            Expr::QualifiedInterfaceSuperMethodCall {
                super_span,
                qualifier,
                name,
                type_args,
                ..
            } => (
                Target::QualifiedSuper {
                    super_span: *super_span,
                    qualifier: Box::new(qualifier.clone()),
                    name: name.clone(),
                },
                type_args,
            ),
            _ => unreachable!("call grammar records only calls"),
        };
        let label = match &target {
            Target::Named(name)
            | Target::Member { name, .. }
            | Target::Super { name, .. }
            | Target::QualifiedSuper { name, .. } => Some(name),
            Target::Invoke(_) | Target::Unavailable(_) => None,
        };
        if let Some(label) = label {
            for argument in &mut arguments.items {
                if let ast::RecoveredArgumentValue::Complete(expression) = &mut argument.value {
                    crate::expr::name_lambda(expression, label);
                }
            }
        }
        let target = if target_complete {
            target
        } else {
            Target::Unavailable(Span::new(expression.span().start, arguments.opening.start))
        };
        self.syntax
            .events
            .push(Event::Body(BodyEvent::Call(Box::new(ast::RecoveredCall {
                block: None,
                target,
                type_args: type_args.clone(),
                arguments,
                trailing: None,
                span: expression.span(),
            }))));
        expression
    }

    pub(crate) fn record_recovered_trailing_argument(
        &mut self,
        call: Span,
        expression: &Result<Expr, ast::Diagnostic>,
        complete: bool,
        span: Span,
    ) {
        let value = match expression {
            Ok(expression) if complete => {
                ast::RecoveredArgumentValue::Complete(Box::new(expression.clone()))
            }
            _ => ast::RecoveredArgumentValue::Unavailable(span),
        };
        self.syntax
            .events
            .push(Event::Body(BodyEvent::TrailingArgument {
                call,
                argument: Box::new(ast::RecoveredCallArgument {
                    name: ast::CallArgumentName::TrailingLambda,
                    spread: ast::SpreadSyntax::Plain,
                    value,
                    span,
                }),
            }));
    }
}
