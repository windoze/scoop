use scoop_ast::{Block, Diagnostic, Span, TypeRef};
use scoop_source::SourceLengthError;
use scoop_syntax::{Parse, SyntaxKind};

use crate::lexer::{Lexed, lex};

use super::Parser;

pub(crate) fn parse_function_body(source: &str) -> Result<Parse<Block>, SourceLengthError> {
    let (mut parser, _) = Parser::from_source(source)?;
    let body = parser.parse_block().and_then(|body| {
        if parser.at_eof() {
            Ok(body)
        } else {
            parser.unexpected("end of function body")
        }
    });
    Ok(parser.finish(source, SyntaxKind::FunctionBody, body))
}

pub(crate) fn parse_type(source: &str) -> Result<Parse<TypeRef>, SourceLengthError> {
    let (mut parser, _) = Parser::from_source(source)?;
    let ty = parser.parse_type_ref().and_then(|ty| {
        if parser.at_eof() {
            Ok(ty)
        } else {
            parser.unexpected("end of type")
        }
    });
    Ok(parser.finish(source, SyntaxKind::TypeFragment, ty))
}

impl Parser {
    pub(super) fn from_source(source: &str) -> Result<(Self, Span), SourceLengthError> {
        let span = scoop_source::source_span(source.len())?;
        let Lexed {
            tokens,
            diagnostics: lexical_diagnostics,
            documentation,
            lexemes,
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
                lexemes,
                syntax: super::syntax::SyntaxEvents::default(),
                attachment_undo: Vec::new(),
            },
            span,
        ))
    }

    pub(super) fn finish<T>(
        mut self,
        source: &str,
        root: SyntaxKind,
        result: Result<T, Diagnostic>,
    ) -> Parse<T> {
        if let Err(diagnostic) = &result {
            self.diagnostics.push(diagnostic.clone());
        }
        if !self.at_eof() {
            self.with_node(SyntaxKind::Error, |parser| {
                while !parser.at_eof() {
                    parser.bump();
                }
            });
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
        let (green, missing) = self.syntax.build(source, &self.lexemes, root);
        Parse::new(green, diagnostics, missing, result.ok())
    }
}
