use super::*;

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
