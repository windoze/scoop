//! Recursive-descent parser for the M10 subset: token vector -> AST.
//!
//! Parsing recovers at declaration, member and statement boundaries and
//! returns every independent spanned diagnostic found in one file. A file
//! with any diagnostic does not produce an AST. Constructs that are
//! lexically recognizable but outside the subset (string interpolation,
//! slices, loop labels, `do-while`, `sealed` classes)
//! get dedicated "not supported" diagnostics rather than generic syntax
//! errors. Declaration parsing lives in `decl.rs`, type parsing in `ty.rs`,
//! statement/control-flow parsing in `stmt.rs`, expression parsing in
//! `expr.rs`, and pattern parsing in `pattern.rs`.

use scoop_ast::{Diagnostic, Ident, SourceFile, Span};

use crate::lexer::{Token, TokenKind, lex};

pub(crate) fn parse_file(source: &str) -> Result<SourceFile, Vec<Diagnostic>> {
    let (tokens, lexical_diagnostics) = lex(source);
    if !lexical_diagnostics.is_empty() {
        return Err(lexical_diagnostics);
    }
    let mut parser = Parser {
        tokens,
        pos: 0,
        diagnostics: Vec::new(),
        next_lambda_id: 0,
        next_anonymous_function_id: 0,
        next_callable_reference_id: 0,
    };
    let mut declarations = Vec::new();
    while !parser.at_eof() {
        let start = parser.pos;
        match parser.parse_decl() {
            Ok(decl) => declarations.push(decl),
            Err(diagnostic) => {
                parser.diagnostics.push(diagnostic);
                parser.synchronize_top_level(start);
            }
        }
    }
    let file = SourceFile {
        declarations,
        span: Span::new(0, source.len() as u32),
    };
    if parser.diagnostics.is_empty() {
        Ok(file)
    } else {
        Err(parser.diagnostics)
    }
}

pub(crate) struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) next_lambda_id: u32,
    pub(crate) next_anonymous_function_id: u32,
    pub(crate) next_callable_reference_id: u32,
}

impl Parser {
    pub(crate) fn alloc_lambda_id(&mut self) -> scoop_ast::LambdaId {
        let id = scoop_ast::LambdaId(self.next_lambda_id);
        self.next_lambda_id += 1;
        id
    }

    pub(crate) fn alloc_anonymous_function_id(&mut self) -> scoop_ast::AnonymousFunctionId {
        let id = scoop_ast::AnonymousFunctionId(self.next_anonymous_function_id);
        self.next_anonymous_function_id += 1;
        id
    }

    pub(crate) fn alloc_callable_reference_id(&mut self) -> scoop_ast::CallableReferenceId {
        let id = scoop_ast::CallableReferenceId(self.next_callable_reference_id);
        self.next_callable_reference_id += 1;
        id
    }

    pub(crate) fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    pub(crate) fn bump(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        token
    }

    pub(crate) fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    /// Number of unmatched `{` tokens before the current position.
    /// Computing it on demand keeps direct token advances in the small
    /// parsing routines from having to maintain duplicate delimiter state.
    pub(crate) fn brace_depth(&self) -> usize {
        self.tokens[..self.pos]
            .iter()
            .fold(0usize, |depth, token| match token.kind {
                TokenKind::LBrace => depth + 1,
                TokenKind::RBrace => depth.saturating_sub(1),
                _ => depth,
            })
    }

    fn is_top_level_start(&self) -> bool {
        match &self.peek().kind {
            TokenKind::Suspend
            | TokenKind::Infix
            | TokenKind::Fun
            | TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Class
            | TokenKind::Interface
            | TokenKind::At => true,
            TokenKind::Ident(text) => {
                matches!(
                    text.as_str(),
                    "public"
                        | "internal"
                        | "private"
                        | "protected"
                        | "const"
                        | "lateinit"
                        | "inner"
                        | "open"
                        | "final"
                        | "abstract"
                        | "override"
                        | "sealed"
                        | "object"
                        | "operator"
                        | "typealias"
                )
            }
            _ => false,
        }
    }

    /// Skip a malformed declaration without consuming the next declaration
    /// starter at brace depth zero. Delimiters inside the malformed item are
    /// crossed before synchronization, so errors in a body do not turn each
    /// remaining statement into a top-level diagnostic.
    fn synchronize_top_level(&mut self, item_start: usize) {
        while !self.at_eof() {
            if self.pos > item_start
                && self.brace_depth() == 0
                && self.peek().newline_before
                && self.is_top_level_start()
            {
                return;
            }
            self.bump();
        }
    }

    /// Recover to the next line/semicolon at `target_depth`, or stop before
    /// the brace closing the current body. `item_start` prevents a token that
    /// itself failed at the beginning of an item from being retried forever.
    pub(crate) fn synchronize_body_item(&mut self, target_depth: usize, item_start: usize) {
        while !self.at_eof() {
            let depth = self.brace_depth();
            if depth < target_depth
                || (depth == target_depth && matches!(self.peek().kind, TokenKind::RBrace))
            {
                return;
            }
            if self.pos > item_start && depth == target_depth && self.peek().newline_before {
                return;
            }
            let token = self.bump();
            if depth == target_depth && matches!(token.kind, TokenKind::Semicolon) {
                return;
            }
        }
    }

    /// Builds "expected <what>, found <token>" at the current token.
    pub(crate) fn unexpected<T>(&self, what: &str) -> Result<T, Diagnostic> {
        let token = self.peek();
        Err(Diagnostic::at(
            token.span,
            format!("expected {what}, found {}", token.describe()),
        ))
    }

    pub(crate) fn expect(
        &mut self,
        what: &str,
        pred: impl Fn(&TokenKind) -> bool,
    ) -> Result<Token, Diagnostic> {
        if pred(&self.peek().kind) {
            Ok(self.bump())
        } else {
            self.unexpected(what)
        }
    }

    pub(crate) fn expect_ident(&mut self, what: &str) -> Result<Ident, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Ident(text) => {
                self.pos += 1;
                Ok(Ident {
                    text,
                    span: token.span,
                })
            }
            _ => self.unexpected(what),
        }
    }
}
