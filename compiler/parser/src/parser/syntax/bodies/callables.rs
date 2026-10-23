use super::*;

impl BodyParts {
    pub(super) fn link_call_literals(&mut self, call: &mut ast::RecoveredCall) {
        let label = match &call.target {
            ast::RecoveredCallTarget::Named(name)
            | ast::RecoveredCallTarget::Member { name, .. }
            | ast::RecoveredCallTarget::Super { name, .. }
            | ast::RecoveredCallTarget::QualifiedSuper { name, .. } => Some(name),
            ast::RecoveredCallTarget::Invoke(_) | ast::RecoveredCallTarget::Unavailable(_) => None,
        };
        for argument in call
            .arguments
            .iter_mut()
            .flat_map(|arguments| &mut arguments.items)
            .chain(&mut call.trailing)
        {
            let ast::RecoveredArgumentValue::Unavailable(span) = argument.value else {
                continue;
            };
            let Some(literal) = self
                .callable_literals
                .iter_mut()
                .find(|literal| literal.span.start == span.start && literal.block == call.block)
            else {
                continue;
            };
            argument.value = ast::RecoveredArgumentValue::CallableLiteral(literal.body);
            if literal.label.is_none()
                && matches!(literal.header, ast::RecoveredCallableHeader::Lambda { .. })
            {
                literal.label = label.cloned();
            }
        }
    }
}

impl Parser {
    pub(crate) fn record_recovered_callable(&mut self, literal: ast::RecoveredCallableLiteral) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::CallableLiteral(Box::new(literal))));
    }

    pub(crate) fn label_recovered_callable(&mut self, start: u32, label: ast::Ident) {
        self.syntax
            .events
            .push(Event::Body(BodyEvent::CallableLabel { start, label }));
    }
}
