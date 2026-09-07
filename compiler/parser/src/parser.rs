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

use scoop_ast::{Diagnostic, Ident, ImportSyntax, PackageSyntax, QualifiedPath, SourceFile, Span};

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
    // File header: one optional `package`, then zero or more imports.
    // Each header item is an independent recovery boundary.
    let package = parser.parse_package_header();
    let mut imports = Vec::new();
    while let Some(import) = parser.parse_import_header() {
        if let Some(import) = import {
            imports.push(import);
        }
    }
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
        package,
        imports,
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

    /// Span from token `start` to just before the current token.
    fn span_from(&self, start: usize) -> Span {
        let start_span = self.tokens[start].span;
        let end = if self.pos > start {
            self.tokens[self.pos - 1].span.end
        } else {
            start_span.start
        };
        Span::new(start_span.start, end)
    }

    /// Whether the current token is the contextual keyword `package`.
    fn at_contextual(&self, keyword: &str) -> bool {
        matches!(&self.tokens.get(self.pos).map(|t| &t.kind), Some(TokenKind::Ident(name)) if name == keyword)
    }

    /// `package QualifiedPath` — at most once and only before any import
    /// or declaration; later occurrences surface as ordinary declaration
    /// errors through `parse_decl`.
    fn parse_package_header(&mut self) -> PackageSyntax {
        if !self.at_contextual("package") {
            return PackageSyntax::RootPackage;
        }
        let start = self.pos;
        self.pos += 1;
        match self.parse_qualified_path() {
            Ok(path) => PackageSyntax::QualifiedPackage(path),
            Err(diagnostic) => {
                self.diagnostics.push(diagnostic);
                self.skip_to_header_boundary(start);
                PackageSyntax::RootPackage
            }
        }
    }

    /// `public? import ImportSelector (as Ident)?` / `public? import
    /// QualifiedPath . *`. Returns `None` when the header ends; `Some(None)`
    /// records that a malformed import was diagnosed and skipped.
    fn parse_import_header(&mut self) -> Option<Option<ImportSyntax>> {
        let start = self.pos;
        let public = if self.at_contextual("public") {
            // Only consume `public` when an `import` actually follows;
            // otherwise it belongs to a declaration.
            if matches!(&self.tokens.get(self.pos + 1).map(|t| &t.kind), Some(TokenKind::Ident(name)) if name == "import")
            {
                self.pos += 2;
                true
            } else {
                return None;
            }
        } else if self.at_contextual("import") {
            self.pos += 1;
            false
        } else {
            return None;
        };
        match self.parse_import_selector(public, start) {
            Ok(import) => Some(Some(import)),
            Err(diagnostic) => {
                self.diagnostics.push(diagnostic);
                self.skip_to_header_boundary(start);
                Some(None)
            }
        }
    }

    fn parse_import_selector(
        &mut self,
        public: bool,
        start: usize,
    ) -> Result<ImportSyntax, Diagnostic> {
        let path = self.parse_qualified_path()?;
        // Star suffix: `path . *` — no alias allowed.
        if self.eat_dot_star() {
            let span = self.span_from(start);
            if let Some(alias) = self.try_alias() {
                return Err(Diagnostic::at(alias.span, "a star import cannot use `as`"));
            }
            if !self.header_item_or_declaration_starts() {
                return Err(Diagnostic::at(
                    self.span_from(self.pos),
                    "unexpected tokens after the star import",
                ));
            }
            return Ok(ImportSyntax::Star { public, path, span });
        }
        let alias = self.try_alias();
        let span = self.span_from(start);
        if !self.header_item_or_declaration_starts() {
            return Err(Diagnostic::at(
                self.span_from(self.pos),
                "unexpected tokens after the import selector; expected a qualified path, `as`, or a declaration",
            ));
        }
        Ok(ImportSyntax::Exact {
            public,
            path,
            alias,
            span,
        })
    }

    /// Dot-separated identifier sequence (at least one segment).
    fn parse_qualified_path(&mut self) -> Result<QualifiedPath, Diagnostic> {
        let start = self.pos;
        let mut segments = Vec::new();
        while let Some(TokenKind::Ident(name)) = self.tokens.get(self.pos).map(|t| &t.kind) {
            let ident = Ident {
                text: name.clone(),
                span: self.span_from(self.pos),
            };
            segments.push(ident);
            self.pos += 1;
            let dot_followed_by_ident = matches!(
                (
                    &self.tokens.get(self.pos).map(|t| &t.kind),
                    self.tokens.get(self.pos + 1).map(|t| &t.kind)
                ),
                (Some(TokenKind::Dot), Some(TokenKind::Ident(_)))
            );
            if !dot_followed_by_ident {
                break;
            }
            self.pos += 1;
        }
        if segments.is_empty() {
            return Err(Diagnostic::at(
                self.span_from(start),
                "expected a qualified path of `.`-separated identifiers",
            ));
        }
        let span = self.span_from(start);
        Ok(QualifiedPath { segments, span })
    }

    /// Whether the current token can start another header item or a
    /// declaration (the only legal successors of an import).
    fn header_item_or_declaration_starts(&self) -> bool {
        self.at_contextual("import")
            || self.at_contextual("public")
            || self.at_eof()
            || self.is_top_level_start()
    }

    /// Consumes `.` `*` if present.
    fn eat_dot_star(&mut self) -> bool {
        let dot_star = matches!(
            (
                &self.tokens.get(self.pos).map(|t| &t.kind),
                self.tokens.get(self.pos + 1).map(|t| &t.kind)
            ),
            (Some(TokenKind::Dot), Some(TokenKind::Star))
        );
        if dot_star {
            self.pos += 2;
        }
        dot_star
    }

    /// `as Identifier`, if present.
    fn try_alias(&mut self) -> Option<Ident> {
        if !matches!(
            &self.tokens.get(self.pos).map(|t| &t.kind),
            Some(TokenKind::As)
        ) {
            return None;
        }
        self.pos += 1;
        match self.tokens.get(self.pos).map(|t| &t.kind) {
            Some(TokenKind::Ident(name)) => {
                let ident = Ident {
                    text: name.clone(),
                    span: self.span_from(self.pos),
                };
                self.pos += 1;
                Some(ident)
            }
            _ => {
                self.diagnostics.push(Diagnostic::at(
                    self.span_from(self.pos),
                    "expected an identifier after `as`",
                ));
                None
            }
        }
    }

    /// Header-level recovery: advance to the next token that can begin a
    /// header item or a declaration.
    fn skip_to_header_boundary(&mut self, item_start: usize) {
        let _ = item_start;
        while !self.at_eof() {
            if self.is_top_level_start() {
                return;
            }
            self.pos += 1;
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
