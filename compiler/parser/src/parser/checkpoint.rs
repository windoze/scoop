use scoop_ast::{DocCommentTarget, Span};

use super::Parser;

/// All mutable state that a speculative grammar production may affect.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Checkpoint {
    pub(crate) position: usize,
    events: usize,
    diagnostics: usize,
    attachments: usize,
    next_lambda: u32,
    next_anonymous_function: u32,
    next_callable_reference: u32,
    arm_body_nesting: Option<usize>,
    supertype_expression_nesting: Option<usize>,
}

impl Parser {
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            position: self.pos,
            events: self.syntax.len(),
            diagnostics: self.diagnostics.len(),
            attachments: self.attachment_undo.len(),
            next_lambda: self.next_lambda_id,
            next_anonymous_function: self.next_anonymous_function_id,
            next_callable_reference: self.next_callable_reference_id,
            arm_body_nesting: self.arm_body_nesting,
            supertype_expression_nesting: self.supertype_expression_nesting,
        }
    }

    pub(crate) fn restore(&mut self, checkpoint: Checkpoint) {
        self.pos = checkpoint.position;
        self.syntax.truncate(checkpoint.events);
        self.diagnostics.truncate(checkpoint.diagnostics);
        for (index, target) in self.attachment_undo.drain(checkpoint.attachments..).rev() {
            self.documentation[index].comment.target = target;
        }
        self.next_lambda_id = checkpoint.next_lambda;
        self.next_anonymous_function_id = checkpoint.next_anonymous_function;
        self.next_callable_reference_id = checkpoint.next_callable_reference;
        self.arm_body_nesting = checkpoint.arm_body_nesting;
        self.supertype_expression_nesting = checkpoint.supertype_expression_nesting;
    }

    pub(crate) fn has_errors_since(&self, checkpoint: Checkpoint) -> bool {
        self.diagnostics.len() != checkpoint.diagnostics
    }

    pub(crate) fn has_lexical_error_in(&self, span: Span) -> bool {
        self.lexical_diagnostics.iter().any(|diagnostic| {
            diagnostic
                .span
                .is_some_and(|error| error.start < span.end && error.end > span.start)
        })
    }

    pub(crate) fn fragment_is_complete(&self, checkpoint: Checkpoint) -> bool {
        let start = self.tokens[checkpoint.position].span.start;
        let end = self
            .pos
            .checked_sub(1)
            .map_or(start, |last| self.tokens[last].span.end);
        !self.has_errors_since(checkpoint) && !self.has_lexical_error_in(Span::new(start, end))
    }

    pub(crate) fn attach_documentation(&mut self, start_token: usize, target: Span) {
        let start = self
            .documentation
            .partition_point(|doc| doc.before_token < start_token);
        for (index, doc) in self
            .documentation
            .iter_mut()
            .enumerate()
            .skip(start)
            .take_while(|(_, doc)| doc.before_token == start_token)
        {
            self.attachment_undo.push((index, doc.comment.target));
            doc.comment.target = DocCommentTarget::Declaration(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_syntax::SyntaxKind;

    #[test]
    fn rollback_discards_recovered_class_headers_and_member_ownership() {
        let source = "class Outer { class Inner { fun editing(value: Boolean) { value. } } }";
        let (mut parser, _) = Parser::from_source(source).unwrap();
        let checkpoint = parser.checkpoint();
        let first = parser.parse_decl().unwrap();
        parser.restore(checkpoint);
        assert_eq!(parser.parse_decl().unwrap(), first);
        let (syntax, diagnostics) = parser.finish_syntax(source, SyntaxKind::SourceFile);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(syntax.nominals.len(), 1);
        let scoop_ast::RecoveredNominal::Class(outer) = &syntax.nominals[0].value else {
            panic!("outer class")
        };
        assert_eq!(outer.recovery.nested.len(), 1);
        let scoop_ast::RecoveredNominal::Class(inner) = &outer.recovery.nested[0] else {
            panic!("inner class")
        };
        assert_eq!(inner.recovery.methods.len(), 1);
        assert_eq!(
            inner.recovery.methods[0]
                .body
                .as_ref()
                .unwrap()
                .selectors
                .len(),
            1
        );
    }

    #[test]
    fn rollback_discards_recovered_call_slots_and_ownership() {
        let source = "fun editing() { call(true,) }";
        let (mut parser, _) = Parser::from_source(source).unwrap();
        let checkpoint = parser.checkpoint();
        let first = parser.parse_decl().unwrap();
        parser.restore(checkpoint);
        assert_eq!(parser.parse_decl().unwrap(), first);
        let (syntax, diagnostics) = parser.finish_syntax(source, SyntaxKind::SourceFile);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(syntax.bodies.len(), 1);
        assert_eq!(syntax.bodies[0].value.calls.len(), 1);
        assert_eq!(
            syntax.bodies[0].value.calls[0]
                .arguments
                .as_ref()
                .unwrap()
                .items
                .len(),
            2
        );
    }

    #[test]
    fn rollback_restores_real_grammar_effects() {
        let source = "{\n/// local\nfun local() = { 1 }\nlocal()\n}";
        let (mut parser, _) = Parser::from_source(source).unwrap();
        let checkpoint = parser.checkpoint();
        let first = parser.parse_expr().unwrap();
        assert_eq!(parser.next_lambda_id, 2);
        assert!(
            parser
                .documentation
                .iter()
                .any(|doc| matches!(doc.comment.target, DocCommentTarget::Declaration(_)))
        );
        parser.restore(checkpoint);
        assert_eq!(parser.next_lambda_id, 0);
        assert!(
            parser
                .documentation
                .iter()
                .all(|doc| doc.comment.target == DocCommentTarget::Unattached)
        );
        assert!(parser.parse_type_ref().is_err());
        parser.restore(checkpoint);
        let expression = parser.parse_expr().unwrap();
        assert_eq!(expression, first);
        let parsed = parser.finish(source, SyntaxKind::FunctionBody, Ok(expression));
        assert!(parsed.missing().is_empty());
        assert_eq!(parsed.syntax().text().to_string(), source);
        assert_eq!(parsed.declaration_data().len(), 1);
        assert_eq!(
            parsed.declaration_data()[0].value.signature.name.text,
            "local"
        );
    }
}
