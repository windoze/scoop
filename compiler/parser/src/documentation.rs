//! Attach only at successful grammar productions, never by source proximity.

use crate::parser::Parser;
use scoop_ast::{DocCommentTarget, Span};

impl Parser {
    pub(crate) fn attach_documentation(&mut self, start_token: usize, target: Span) {
        let start = self
            .documentation
            .partition_point(|doc| doc.before_token < start_token);
        for doc in self.documentation[start..]
            .iter_mut()
            .take_while(|doc| doc.before_token == start_token)
        {
            doc.comment.target = DocCommentTarget::Declaration(target);
        }
    }
}
