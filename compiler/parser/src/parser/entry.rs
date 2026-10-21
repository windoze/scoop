use scoop_ast::{Block, Diagnostic, DiagnosticSeverity, Span, TypeRef};

use crate::lexer::{Lexed, lex};

use super::Parser;

pub(crate) fn parse_function_body(source: &str) -> Result<Block, Vec<Diagnostic>> {
    let (mut parser, _) = Parser::from_source(source)?;
    let body = parser.parse_block().and_then(|body| {
        if parser.at_eof() {
            Ok(body)
        } else {
            parser.unexpected("end of function body")
        }
    });
    parser.finish(body)
}

pub(crate) fn parse_type(source: &str) -> Result<TypeRef, Vec<Diagnostic>> {
    let (mut parser, _) = Parser::from_source(source)?;
    let ty = parser.parse_type_ref().and_then(|ty| {
        if parser.at_eof() {
            Ok(ty)
        } else {
            parser.unexpected("end of type")
        }
    });
    parser.finish(ty)
}

impl Parser {
    pub(super) fn from_source(source: &str) -> Result<(Self, Span), Vec<Diagnostic>> {
        let span = scoop_source::source_span(source.len()).map_err(|error| {
            vec![Diagnostic::without_span(
                DiagnosticSeverity::Error,
                0,
                error.to_string(),
            )]
        })?;
        let Lexed {
            tokens,
            diagnostics: lexical_diagnostics,
            documentation,
        } = lex(source);
        Ok((
            Self {
                expression_nesting: Self::expression_nesting(&tokens),
                arm_body_nesting: None,
                supertype_expression_nesting: None,
                tokens,
                pos: 0,
                diagnostics: Vec::new(),
                lexical_diagnostics,
                next_lambda_id: 0,
                next_anonymous_function_id: 0,
                next_callable_reference_id: 0,
                documentation,
            },
            span,
        ))
    }

    pub(super) fn finish<T>(mut self, result: Result<T, Diagnostic>) -> Result<T, Vec<Diagnostic>> {
        if let Err(diagnostic) = &result {
            self.diagnostics.push(diagnostic.clone());
        }
        let mut diagnostics = std::mem::take(&mut self.lexical_diagnostics);
        let syntax_diagnostics = self
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                !self.syntax_diagnostic_is_shadowed_by_lexical_error(diagnostic, &diagnostics)
            })
            .cloned()
            .collect::<Vec<_>>();
        diagnostics.extend(syntax_diagnostics);
        diagnostics.sort_by_key(|diagnostic| {
            diagnostic
                .span
                .map_or((u32::MAX, u32::MAX), |span| (span.start, span.end))
        });
        match result {
            Ok(value) if diagnostics.is_empty() => Ok(value),
            _ => Err(diagnostics),
        }
    }
}
