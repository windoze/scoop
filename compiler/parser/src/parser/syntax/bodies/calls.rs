use super::*;
use ast::{Expr, RecoveredCallTarget as Target};

impl Parser {
    pub(crate) fn record_recovered_call(
        &mut self,
        expression: &Expr,
        mut arguments: Option<ast::RecoveredCallArguments>,
        target_complete: bool,
        mut trailing: Option<ast::RecoveredCallArgument>,
    ) {
        let (target, type_args) = match expression {
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
            for argument in arguments
                .iter_mut()
                .flat_map(|arguments| &mut arguments.items)
                .chain(&mut trailing)
            {
                if let ast::RecoveredArgumentValue::Complete(expression) = &mut argument.value {
                    crate::expr::name_lambda(expression, label);
                }
            }
        }
        let target = if target_complete {
            target
        } else {
            let end = arguments
                .as_ref()
                .map(|arguments| arguments.opening.start)
                .or_else(|| trailing.as_ref().map(|argument| argument.span.start))
                .expect("a call has an argument list or an actual trailing input");
            Target::Unavailable(Span::new(expression.span().start, end))
        };
        let span = Span::new(
            expression.span().start,
            trailing
                .as_ref()
                .map_or(expression.span().end, |argument| argument.span.end),
        );
        self.syntax
            .events
            .push(Event::Body(BodyEvent::Call(Box::new(ast::RecoveredCall {
                block: None,
                target,
                type_args: type_args.clone(),
                arguments,
                trailing,
                span,
            }))));
    }
}
