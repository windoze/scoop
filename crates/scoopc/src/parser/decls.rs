//! 顶层声明解析：package/import/fun/type decl。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, TokenKind};

use super::{ParseError, Parser};

impl Parser {
    pub(super) fn parse_package_decl(&mut self) -> Result<ast::PackageDecl, ParseError> {
        let kw = self.expect_keyword(Keyword::Package)?;
        let path = self.parse_dotted_path()?;
        let end = path.last().map(|i| i.span.end).unwrap_or(kw.span.end);
        self.eat_symbol(Symbol::Semicolon);

        Ok(ast::PackageDecl {
            span: Span::new(kw.span.start, end),
            path,
        })
    }

    pub(super) fn parse_import_decl(&mut self) -> Result<ast::ImportDecl, ParseError> {
        let kw = self.expect_keyword(Keyword::Import)?;
        let path = self.parse_dotted_path()?;
        let mut has_star = false;
        let mut end = path.last().map(|i| i.span.end).unwrap_or(kw.span.end);

        if self.peek_symbol(Symbol::Dot) {
            // 可能是 `import a.b.*`
            let dot_tok = self.bump();
            if self.peek_symbol(Symbol::Star) {
                let star_tok = self.bump();
                has_star = true;
                end = star_tok.span.end;
                // dot 不进入 path，但 span 需要覆盖
                let _ = dot_tok;
            } else {
                return Err(ParseError::Expected {
                    expected: "`*`（import star）",
                    found: self.peek().kind,
                    span: self.peek().span.into(),
                });
            }
        }

        self.eat_symbol(Symbol::Semicolon);

        Ok(ast::ImportDecl {
            span: Span::new(kw.span.start, end),
            path,
            has_star,
        })
    }

    pub(super) fn parse_fun_decl(&mut self) -> Result<ast::FunDecl, ParseError> {
        let kw = self.expect_keyword(Keyword::Fun)?;
        let name_tok = self.expect_kind(TokenKind::Ident, "函数名（标识符）")?;
        let name = ast::Ident { span: name_tok.span };

        let (params_span, params) = self.parse_param_list()?;

        let return_ty = if self.eat_symbol(Symbol::Colon) {
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        // TODO: generics / effect rows / where clause（当前先粗暴跳过，避免阻塞后续顶层解析）
        let mut last_end = return_ty
            .as_ref()
            .map(|t| t.span().end)
            .unwrap_or(params_span.end);
        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::LBrace) {
            if self.is_top_level_item_start() {
                break;
            }
            last_end = self.bump().span.end;
        }

        let body = if self.peek_symbol(Symbol::LBrace) {
            let span = self.consume_balanced(Symbol::LBrace, Symbol::RBrace)?;
            last_end = span.end;
            ast::FunBody::Block(ast::Block { span })
        } else {
            ast::FunBody::Missing
        };

        Ok(ast::FunDecl {
            span: Span::new(kw.span.start, last_end),
            name,
            params_span,
            params,
            return_ty,
            body,
        })
    }

    pub(super) fn parse_param_list(&mut self) -> Result<(Span, Vec<ast::Param>), ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        let mut params = Vec::new();
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok((Span::new(start, close.span.end), params));
        }

        loop {
            let name_tok = self.expect_kind(TokenKind::Ident, "参数名（标识符）")?;
            let name = ast::Ident { span: name_tok.span };

            let ty = if self.eat_symbol(Symbol::Colon) {
                Some(self.parse_type_ref()?)
            } else {
                None
            };
            params.push(ast::Param { name, ty });

            if self.eat_symbol(Symbol::Comma) {
                // trailing comma
                if self.peek_symbol(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_symbol(Symbol::RParen)?;
        Ok((Span::new(start, close.span.end), params))
    }

    pub(super) fn parse_type_decl(&mut self) -> Result<ast::TypeDecl, ParseError> {
        let start = self.peek().span.start;

        // modifiers（当前仅消费，不进入 AST）
        while self.peek_keyword(Keyword::Open)
            || self.peek_keyword(Keyword::Abstract)
            || self.peek_keyword(Keyword::Sealed)
        {
            self.bump();
        }

        let (kind_kw, kind) = if self.peek_keyword(Keyword::Class) {
            (self.bump(), ast::TypeKind::Class)
        } else if self.peek_keyword(Keyword::Interface) {
            (self.bump(), ast::TypeKind::Interface)
        } else if self.peek_keyword(Keyword::Struct) {
            (self.bump(), ast::TypeKind::Struct)
        } else if self.peek_keyword(Keyword::Enum) {
            (self.bump(), ast::TypeKind::Enum)
        } else if self.peek_keyword(Keyword::Effect) {
            (self.bump(), ast::TypeKind::Effect)
        } else {
            let tok = self.peek().clone();
            return Err(ParseError::Expected {
                expected: "类型声明关键字（class/interface/struct/enum/effect）",
                found: tok.kind,
                span: tok.span.into(),
            });
        };

        let name_tok = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
        let name = ast::Ident { span: name_tok.span };

        // optional generic params: `<...>`
        if self.peek_symbol(Symbol::Lt) {
            let _ = self.consume_balanced(Symbol::Lt, Symbol::Gt)?;
        }

        // optional primary ctor params: `( ... )`
        if self.peek_symbol(Symbol::LParen) {
            let _ = self.consume_balanced(Symbol::LParen, Symbol::RParen)?;
        }

        // header tail（继承/实现等）：消耗到 `{` 或下一个顶层 item 开始
        let mut last_end = name_tok.span.end.max(kind_kw.span.end);
        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::LBrace) {
            if self.is_top_level_item_start() {
                break;
            }
            last_end = self.bump().span.end;
        }

        let body = if self.peek_symbol(Symbol::LBrace) {
            let span = self.consume_balanced(Symbol::LBrace, Symbol::RBrace)?;
            last_end = span.end;
            Some(ast::Block { span })
        } else {
            None
        };

        Ok(ast::TypeDecl {
            span: Span::new(start, last_end),
            kind,
            name,
            body,
        })
    }

    pub(super) fn parse_dotted_path(&mut self) -> Result<Vec<ast::Ident>, ParseError> {
        let first = self.expect_kind(TokenKind::Ident, "标识符")?;
        let mut parts = vec![ast::Ident { span: first.span }];
        while self.peek_symbol(Symbol::Dot) {
            self.bump(); // '.'
            let ident = self.expect_kind(TokenKind::Ident, "标识符")?;
            parts.push(ast::Ident { span: ident.span });
        }
        Ok(parts)
    }
}

