//! Recursive-descent parser: one token vector becomes a spanned AST.
//!
//! Parsing recovers at declaration, member and statement boundaries and
//! returns every independent spanned diagnostic found in one file. A file
//! with any diagnostic does not produce an AST. Unsupported declaration
//! or control syntax receives a dedicated diagnostic where it is recognized.
//! Declaration parsing lives in `decl.rs`, type parsing in `ty.rs`,
//! statement/control-flow parsing in `stmt.rs`, expression parsing in
//! `expr.rs`, and pattern parsing in `pattern.rs`.

use scoop_ast::{Diagnostic, DiagnosticNote, Ident, PackageSyntax, SourceFile, Span};

use crate::lexer::{Token, TokenKind, lex};

pub(crate) fn parse_file(source: &str) -> Result<SourceFile, Vec<Diagnostic>> {
    let (tokens, lexical_diagnostics) = lex(source);
    let mut parser = Parser {
        expression_nesting: Parser::expression_nesting(&tokens),
        arm_body_nesting: None,
        tokens,
        pos: 0,
        diagnostics: Vec::new(),
        next_lambda_id: 0,
        next_anonymous_function_id: 0,
        next_callable_reference_id: 0,
    };
    let mut package = PackageSyntax::RootPackage;
    let mut imports = Vec::new();
    let mut declarations = Vec::new();
    let mut header_state = HeaderState::Start;
    while !parser.at_eof() {
        let start = parser.pos;
        match parser.top_level_item_kind() {
            TopLevelItemKind::Package => {
                let keyword_span = parser.peek().span;
                if let PackageSyntax::QualifiedPackage {
                    package_keyword_span: first_keyword_span,
                    ..
                } = &package
                {
                    parser.diagnostics.push(
                        Diagnostic::at(
                            keyword_span,
                            "a source file may contain only one `package` header",
                        )
                        .with_note(DiagnosticNote::at(
                            0,
                            *first_keyword_span,
                            "first `package` header is here",
                        )),
                    );
                } else if header_state >= HeaderState::AfterImport {
                    parser.diagnostics.push(Diagnostic::at(
                        keyword_span,
                        "a `package` header must appear before imports and declarations",
                    ));
                }
                let accept = matches!(package, PackageSyntax::RootPackage)
                    && header_state < HeaderState::AfterImport;
                header_state.advance_to(HeaderState::AfterPackage);
                match parser.parse_package_header() {
                    Ok(parsed) if accept => {
                        package = parsed;
                    }
                    Ok(_) => {}
                    Err(diagnostic) => {
                        parser.diagnostics.push(diagnostic);
                        parser.synchronize_header_item(start);
                    }
                }
            }
            TopLevelItemKind::Import => {
                let header_span = parser.import_prefix_span();
                let accept = header_state < HeaderState::InDeclarations;
                if !accept {
                    parser.diagnostics.push(Diagnostic::at(
                        header_span,
                        "`import` headers must appear before declarations",
                    ));
                }
                header_state.advance_to(HeaderState::AfterImport);
                match parser.parse_import_header() {
                    Ok(import) if accept => imports.push(import),
                    Ok(_) => {}
                    Err(diagnostic) => {
                        parser.diagnostics.push(diagnostic);
                        parser.synchronize_header_item(start);
                    }
                }
            }
            TopLevelItemKind::UnsupportedImportModifier(modifier) => {
                let modifier_token = parser.bump();
                if header_state >= HeaderState::InDeclarations {
                    parser.diagnostics.push(Diagnostic::at(
                        modifier_token.span,
                        "`import` headers must appear before declarations",
                    ));
                }
                parser.diagnostics.push(Diagnostic::at(
                    modifier_token.span,
                    format!(
                        "`{modifier} import` is not supported; imports may be ordinary or `public`"
                    ),
                ));
                header_state.advance_to(HeaderState::AfterImport);
                match parser.parse_import_header() {
                    Ok(_) => {}
                    Err(diagnostic) => {
                        parser.diagnostics.push(diagnostic);
                        parser.synchronize_header_item(start);
                    }
                }
            }
            TopLevelItemKind::Declaration => {
                header_state.advance_to(HeaderState::InDeclarations);
                match parser.parse_decl() {
                    Ok(decl) => declarations.push(decl),
                    Err(diagnostic) => {
                        parser.diagnostics.push(diagnostic);
                        parser.synchronize_top_level(start);
                    }
                }
            }
        }
    }
    let file = SourceFile {
        package,
        imports,
        declarations,
        span: Span::new(0, source.len() as u32),
    };
    let mut diagnostics = lexical_diagnostics;
    let syntax_diagnostics = parser
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            !parser.syntax_diagnostic_is_shadowed_by_lexical_error(diagnostic, &diagnostics)
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics.extend(syntax_diagnostics);
    diagnostics.sort_by_key(|diagnostic| {
        diagnostic
            .span
            .map_or((u32::MAX, u32::MAX), |span| (span.start, span.end))
    });
    if diagnostics.is_empty() {
        Ok(file)
    } else {
        Err(diagnostics)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HeaderState {
    Start,
    AfterPackage,
    AfterImport,
    InDeclarations,
}

impl HeaderState {
    fn advance_to(&mut self, next: Self) {
        *self = (*self).max(next);
    }
}

enum TopLevelItemKind {
    Package,
    Import,
    UnsupportedImportModifier(&'static str),
    Declaration,
}

#[derive(Default)]
struct DelimiterBalance {
    parentheses: usize,
    brackets: usize,
    braces: usize,
}

impl DelimiterBalance {
    fn observe(&mut self, kind: &TokenKind) {
        match kind {
            TokenKind::LParen => self.parentheses += 1,
            TokenKind::RParen => self.parentheses = self.parentheses.saturating_sub(1),
            TokenKind::LBracket => self.brackets += 1,
            TokenKind::RBracket => self.brackets = self.brackets.saturating_sub(1),
            TokenKind::LBrace => self.braces += 1,
            TokenKind::RBrace => self.braces = self.braces.saturating_sub(1),
            _ => {}
        }
    }

    fn is_balanced(&self) -> bool {
        self.parentheses == 0 && self.brackets == 0 && self.braces == 0
    }
}

pub(crate) struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) expression_nesting: Vec<usize>,
    pub(crate) arm_body_nesting: Option<usize>,
    pub(crate) pos: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) next_lambda_id: u32,
    pub(crate) next_anonymous_function_id: u32,
    pub(crate) next_callable_reference_id: u32,
}

impl Parser {
    fn syntax_diagnostic_is_shadowed_by_lexical_error(
        &self,
        syntax: &Diagnostic,
        lexical_diagnostics: &[Diagnostic],
    ) -> bool {
        let Some(syntax_span) = syntax.span else {
            return false;
        };
        lexical_diagnostics.iter().any(|lexical| {
            let Some(lexical_span) = lexical.span else {
                return false;
            };
            let overlaps_recovery_token =
                lexical_span.start < syntax_span.end && syntax_span.start < lexical_span.end;
            overlaps_recovery_token
                || (lexical_span.start <= syntax_span.start
                    && !self.has_top_level_start_between(lexical_span.end, syntax_span.start))
        })
    }

    fn has_top_level_start_between(&self, start: u32, end: u32) -> bool {
        let mut braces = 0usize;
        for (index, token) in self.tokens.iter().enumerate() {
            if token.span.start >= end {
                break;
            }
            if token.span.start >= start
                && braces == 0
                && Self::is_top_level_start_at(&self.tokens, index)
            {
                return true;
            }
            match token.kind {
                TokenKind::LBrace => braces += 1,
                TokenKind::RBrace => braces = braces.saturating_sub(1),
                _ => {}
            }
        }
        false
    }

    fn is_top_level_start_at(tokens: &[Token], index: usize) -> bool {
        match &tokens[index].kind {
            TokenKind::Package
            | TokenKind::Import
            | TokenKind::Suspend
            | TokenKind::Infix
            | TokenKind::Fun
            | TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Class
            | TokenKind::Interface
            | TokenKind::Val
            | TokenKind::Var
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

    fn top_level_item_kind(&self) -> TopLevelItemKind {
        match &self.peek().kind {
            TokenKind::Package => TopLevelItemKind::Package,
            TokenKind::Import => TopLevelItemKind::Import,
            TokenKind::Ident(text) if text == "public" && self.next_is_import() => {
                TopLevelItemKind::Import
            }
            TokenKind::Ident(text) if text == "internal" && self.next_is_import() => {
                TopLevelItemKind::UnsupportedImportModifier("internal")
            }
            TokenKind::Ident(text) if text == "private" && self.next_is_import() => {
                TopLevelItemKind::UnsupportedImportModifier("private")
            }
            _ => TopLevelItemKind::Declaration,
        }
    }

    pub(crate) fn at_public_import(&self) -> bool {
        matches!(&self.peek().kind, TokenKind::Ident(text) if text == "public")
            && self.next_is_import()
    }

    fn next_is_import(&self) -> bool {
        self.tokens
            .get(self.pos + 1)
            .is_some_and(|token| matches!(token.kind, TokenKind::Import))
    }

    fn import_prefix_span(&self) -> Span {
        if self.at_public_import() {
            Span::new(self.peek().span.start, self.tokens[self.pos + 1].span.end)
        } else {
            self.peek().span
        }
    }

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
        Self::is_top_level_start_at(&self.tokens, self.pos)
    }

    fn is_header_recovery_start(&self) -> bool {
        match &self.peek().kind {
            TokenKind::Package
            | TokenKind::Import
            | TokenKind::Suspend
            | TokenKind::Infix
            | TokenKind::Fun
            | TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Class
            | TokenKind::Interface
            | TokenKind::Val
            | TokenKind::Var
            | TokenKind::At => true,
            TokenKind::Ident(text)
                if matches!(text.as_str(), "public" | "internal" | "private")
                    && self.next_is_import() =>
            {
                true
            }
            TokenKind::Ident(text) => {
                self.peek().newline_before
                    && matches!(
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

    /// Skip one malformed package/import without consuming the next complete
    /// top-level item. Unlike declaration recovery, header grammar does not
    /// require a newline boundary, so any balanced top-level starter is safe.
    fn synchronize_header_item(&mut self, item_start: usize) {
        let mut balance = DelimiterBalance::default();
        for token in &self.tokens[item_start..self.pos] {
            balance.observe(&token.kind);
        }
        while !self.at_eof() {
            if self.pos > item_start && balance.is_balanced() && self.is_header_recovery_start() {
                return;
            }
            let token = self.bump();
            balance.observe(&token.kind);
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
