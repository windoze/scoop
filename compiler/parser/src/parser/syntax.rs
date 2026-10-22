use scoop_ast::{Diagnostic, Ident, Span};
use scoop_syntax::{
    DeclarationData, DeclarationIndex, DeclarationName, DeclarationSyntax, ExpectedSyntax,
    GreenNode, GreenNodeBuilder, MissingSyntax, SyntaxKind,
};

use crate::lexer::{Lexeme, TokenKind};

use super::Parser;

pub(super) fn expected_token(predicate: &impl Fn(&TokenKind) -> bool) -> Option<SyntaxKind> {
    const PUNCTUATION: &[TokenKind] = &[
        TokenKind::LParen,
        TokenKind::RParen,
        TokenKind::LBrace,
        TokenKind::RBrace,
        TokenKind::LBracket,
        TokenKind::RBracket,
        TokenKind::Comma,
        TokenKind::Semicolon,
        TokenKind::Colon,
        TokenKind::DoubleColon,
        TokenKind::At,
        TokenKind::Dot,
        TokenKind::Arrow,
        TokenKind::Equal,
        TokenKind::Less,
        TokenKind::Greater,
    ];
    let mut matches = PUNCTUATION.iter().filter(|kind| predicate(kind));
    let first = matches.next()?.syntax_kind();
    matches.next().is_none().then_some(first)
}

#[derive(Debug)]
enum Event {
    Start { offset: u32, kind: SyntaxKind },
    Finish { offset: u32 },
    Missing(MissingSyntax),
    DeclarationName(DeclarationName),
    FunctionHeader(Box<scoop_ast::FunctionHeader>),
    TopLevelDeclaration,
}

pub(super) struct SyntaxBuild {
    pub(super) green: GreenNode,
    pub(super) missing: Vec<MissingSyntax>,
    pub(super) declarations: Vec<DeclarationSyntax>,
    pub(super) function_headers: crate::FunctionHeaders,
    pub(super) top_level_declarations: Vec<DeclarationIndex>,
}

#[derive(Debug, Default)]
pub(super) struct SyntaxEvents {
    events: Vec<Event>,
}

impl SyntaxEvents {
    pub(super) fn len(&self) -> usize {
        self.events.len()
    }

    pub(super) fn truncate(&mut self, length: usize) {
        self.events.truncate(length);
    }

    pub(super) fn build(self, source: &str, lexemes: &[Lexeme], root: SyntaxKind) -> SyntaxBuild {
        let mut builder = GreenNodeBuilder::new();
        let mut cursor = 0;
        let mut missing = Vec::new();
        let mut declarations: Vec<DeclarationSyntax> = Vec::new();
        let mut owners: Vec<Option<DeclarationIndex>> = Vec::new();
        let mut headers = Vec::new();
        let mut last_root = None;
        let mut top_level_declarations = Vec::new();
        builder.start_node(root.into());
        for event in self.events {
            if let Event::TopLevelDeclaration = event {
                top_level_declarations.push(
                    last_root.take().expect(
                        "a successful top-level production has just finished its declaration",
                    ),
                );
                continue;
            }
            if let Event::DeclarationName(name) = event {
                if let Some(index) = owners.iter().rev().flatten().next() {
                    declarations[index.0].name = Some(name);
                }
                continue;
            }
            if let Event::FunctionHeader(header) = event {
                let declaration = *owners
                    .iter()
                    .rev()
                    .flatten()
                    .next()
                    .expect("function grammar records a header within its declaration");
                headers.push(DeclarationData {
                    declaration,
                    value: *header,
                });
                continue;
            }
            let offset = match &event {
                Event::Start { offset, .. } | Event::Finish { offset } => *offset,
                Event::Missing(item) => item.offset,
                Event::DeclarationName(_)
                | Event::FunctionHeader(_)
                | Event::TopLevelDeclaration => {
                    unreachable!("handled above")
                }
            };
            emit_before(&mut builder, source, lexemes, &mut cursor, offset);
            match event {
                Event::Start { kind, offset } => {
                    let index = kind.is_declaration().then(|| {
                        let index = DeclarationIndex(declarations.len());
                        declarations.push(DeclarationSyntax {
                            parent: owners.iter().rev().flatten().next().copied(),
                            kind,
                            span: Span::new(offset, offset),
                            name: None,
                        });
                        index
                    });
                    owners.push(index);
                    builder.start_node(kind.into());
                }
                Event::Finish { offset } => {
                    if let Some(index) = owners.pop().flatten() {
                        let declaration = &mut declarations[index.0];
                        declaration.span.end = offset.max(declaration.span.start);
                        if declaration.parent.is_none() {
                            last_root = Some(index);
                        }
                    }
                    builder.finish_node();
                }
                Event::Missing(item) => {
                    builder.start_node(SyntaxKind::Missing.into());
                    builder.finish_node();
                    missing.push(item);
                }
                Event::DeclarationName(_)
                | Event::FunctionHeader(_)
                | Event::TopLevelDeclaration => {
                    unreachable!("handled above")
                }
            }
        }
        emit_before(&mut builder, source, lexemes, &mut cursor, u32::MAX);
        builder.finish_node();
        SyntaxBuild {
            green: builder.finish(),
            missing,
            declarations,
            function_headers: headers,
            top_level_declarations,
        }
    }
}

fn emit_before(
    builder: &mut GreenNodeBuilder<'_>,
    source: &str,
    lexemes: &[Lexeme],
    cursor: &mut usize,
    offset: u32,
) {
    while let Some(lexeme) = lexemes.get(*cursor) {
        if lexeme.span.start >= offset {
            break;
        }
        let error = lexeme.kind == SyntaxKind::InvalidToken;
        if error {
            builder.start_node(SyntaxKind::Error.into());
        }
        builder.token(
            lexeme.kind.into(),
            &source[lexeme.span.start as usize..lexeme.span.end as usize],
        );
        if error {
            builder.finish_node();
        }
        *cursor += 1;
    }
}

impl Parser {
    pub(crate) fn record_top_level_declaration(&mut self) {
        self.syntax.events.push(Event::TopLevelDeclaration);
    }

    pub(crate) fn record_function_header(
        &mut self,
        signature: &scoop_ast::FunctionSignature,
        body_form: scoop_ast::FunctionBodyForm,
        checkpoint: super::checkpoint::Checkpoint,
    ) {
        if !self.has_errors_since(checkpoint) && !self.has_lexical_error_in(signature.span) {
            self.syntax
                .events
                .push(Event::FunctionHeader(Box::new(scoop_ast::FunctionHeader {
                    signature: signature.clone(),
                    body_form,
                })));
        }
    }

    pub(crate) fn record_declaration_name(&mut self, name: &Ident) {
        self.record_syntax_name(&name.text, name.span);
    }

    pub(crate) fn record_syntax_name(&mut self, text: &str, span: Span) {
        self.syntax
            .events
            .push(Event::DeclarationName(DeclarationName {
                text: text.to_owned(),
                span,
            }));
    }

    pub(crate) fn with_node<T>(
        &mut self,
        kind: SyntaxKind,
        production: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let start_position = self.pos;
        let start_event = self.syntax.events.len();
        self.syntax.events.push(Event::Start {
            offset: self.peek().span.start,
            kind,
        });
        let result = production(self);
        if self.pos == start_position && self.syntax.events.len() == start_event + 1 {
            self.syntax.events.truncate(start_event);
            return result;
        }
        let offset = self
            .pos
            .checked_sub(1)
            .map_or(0, |index| self.tokens[index].span.end);
        self.syntax.events.push(Event::Finish { offset });
        result
    }

    pub(super) fn missing<T>(
        &mut self,
        expected: ExpectedSyntax,
        what: &str,
    ) -> Result<T, Diagnostic> {
        Err(self.missing_diagnostic(expected, what))
    }

    pub(super) fn missing_diagnostic(
        &mut self,
        expected: ExpectedSyntax,
        what: &str,
    ) -> Diagnostic {
        let token = self.peek();
        let diagnostic = Diagnostic::at(
            token.span,
            format!("expected {what}, found {}", token.describe()),
        );
        self.syntax.events.push(Event::Missing(MissingSyntax {
            offset: token.span.start,
            expected,
        }));
        diagnostic
    }
}
