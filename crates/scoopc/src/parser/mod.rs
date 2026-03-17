//! Parser（语法分析）。
//!
//! 本阶段采用“保守增量”策略：
//! - 先支持文件级结构：`package` / `import` / 顶层 `fun`
//! - 函数体暂时只保证 `{ ... }` 的括号平衡，并记录 `Span`
//! - 表达式/语句的完整解析会在后续阶段逐步补齐

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::lexer::{lex, LexError};
use crate::syntax::token::{Keyword, Symbol, Token, TokenKind};

#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Lex(#[from] LexError),

    #[error("语法错误：期望 {expected}，但遇到 {found:?}")]
    #[diagnostic(code(scoop::parse::expected))]
    Expected {
        expected: &'static str,
        found: TokenKind,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("语法错误：未闭合的分组（期望 {close:?}）")]
    #[diagnostic(code(scoop::parse::unterminated_group))]
    UnterminatedGroup {
        close: Symbol,
        #[label("从这里开始")]
        span: miette::SourceSpan,
    },
}

pub fn parse_file(source: &SourceFile) -> Result<ast::File, ParseError> {
    let tokens = lex(source.text())?;
    Parser::new(tokens).parse_file()
}

struct Parser {
    tokens: Vec<Token>,
    i: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, i: 0 }
    }

    fn parse_file(mut self) -> Result<ast::File, ParseError> {
        let package = if self.peek_keyword(Keyword::Package) {
            Some(self.parse_package_decl()?)
        } else {
            None
        };

        let mut imports = Vec::new();
        while self.peek_keyword(Keyword::Import) {
            imports.push(self.parse_import_decl()?);
        }

        let mut items = Vec::new();
        while !self.peek_kind(TokenKind::Eof) {
            if self.peek_keyword(Keyword::Fun) {
                items.push(ast::Item::Fun(self.parse_fun_decl()?));
                continue;
            }
            if self.is_type_decl_start() {
                items.push(ast::Item::Type(self.parse_type_decl()?));
                continue;
            }

            // 早期阶段：遇到未知顶层结构直接报错（后续再做错误恢复）
            let tok = self.peek().clone();
            return Err(ParseError::Expected {
                expected: "顶层声明（例如 `fun`）",
                found: tok.kind,
                span: tok.span.into(),
            });
        }

        Ok(ast::File {
            package,
            imports,
            items,
        })
    }

    fn parse_package_decl(&mut self) -> Result<ast::PackageDecl, ParseError> {
        let kw = self.expect_keyword(Keyword::Package)?;
        let path = self.parse_dotted_path()?;
        let end = path.last().map(|i| i.span.end).unwrap_or(kw.span.end);
        self.eat_symbol(Symbol::Semicolon);

        Ok(ast::PackageDecl {
            span: Span::new(kw.span.start, end),
            path,
        })
    }

    fn parse_import_decl(&mut self) -> Result<ast::ImportDecl, ParseError> {
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

    fn parse_fun_decl(&mut self) -> Result<ast::FunDecl, ParseError> {
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

    fn parse_param_list(&mut self) -> Result<(Span, Vec<ast::Param>), ParseError> {
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

    fn parse_type_ref(&mut self) -> Result<ast::TypeRef, ParseError> {
        let mut ty = if self.peek_symbol(Symbol::LParen) {
            self.parse_tuple_or_group_type()?
        } else {
            self.parse_path_type()?
        };

        if self.peek_symbol(Symbol::Question) {
            let q = self.bump();
            let span = Span::new(ty.span().start, q.span.end);
            ty = ast::TypeRef::Nullable {
                span,
                inner: Box::new(ty),
            };
        }

        Ok(ty)
    }

    fn parse_tuple_or_group_type(&mut self) -> Result<ast::TypeRef, ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok(ast::TypeRef::Tuple(ast::TypeTuple {
                span: Span::new(start, close.span.end),
                elements: Vec::new(),
            }));
        }

        let first = self.parse_type_ref()?;
        if self.eat_symbol(Symbol::Comma) {
            let mut elements = vec![first];
            while !self.peek_symbol(Symbol::RParen) && !self.peek_kind(TokenKind::Eof) {
                elements.push(self.parse_type_ref()?);
                if self.eat_symbol(Symbol::Comma) {
                    // allow trailing comma
                    if self.peek_symbol(Symbol::RParen) {
                        break;
                    }
                    continue;
                }
                break;
            }
            let close = self.expect_symbol(Symbol::RParen)?;
            return Ok(ast::TypeRef::Tuple(ast::TypeTuple {
                span: Span::new(start, close.span.end),
                elements,
            }));
        }

        // grouping type: `(T)` → `T`
        let _close = self.expect_symbol(Symbol::RParen)?;
        Ok(first)
    }

    fn parse_path_type(&mut self) -> Result<ast::TypeRef, ParseError> {
        let first = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
        let start = first.span.start;
        let mut segments = vec![ast::Ident { span: first.span }];

        while self.peek_symbol(Symbol::Dot) {
            self.bump();
            let seg = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
            segments.push(ast::Ident { span: seg.span });
        }

        let mut args = Vec::new();
        let mut end = segments.last().unwrap().span.end;
        if self.peek_symbol(Symbol::Lt) {
            let (a, gt_end) = self.parse_type_args()?;
            args = a;
            end = gt_end;
        }

        Ok(ast::TypeRef::Path(ast::TypePath {
            span: Span::new(start, end),
            segments,
            args,
        }))
    }

    fn parse_type_args(&mut self) -> Result<(Vec<ast::TypeRef>, usize), ParseError> {
        let _lt = self.expect_symbol(Symbol::Lt)?;
        let mut args = Vec::new();

        if self.peek_symbol(Symbol::Gt) {
            let gt = self.bump();
            return Ok((args, gt.span.end));
        }

        loop {
            args.push(self.parse_type_ref()?);
            if self.eat_symbol(Symbol::Comma) {
                if self.peek_symbol(Symbol::Gt) {
                    break;
                }
                continue;
            }
            break;
        }
        let gt = self.expect_symbol(Symbol::Gt)?;
        Ok((args, gt.span.end))
    }

    fn parse_type_decl(&mut self) -> Result<ast::TypeDecl, ParseError> {
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

    fn parse_dotted_path(&mut self) -> Result<Vec<ast::Ident>, ParseError> {
        let first = self.expect_kind(TokenKind::Ident, "标识符")?;
        let mut parts = vec![ast::Ident { span: first.span }];
        while self.peek_symbol(Symbol::Dot) {
            self.bump(); // '.'
            let ident = self.expect_kind(TokenKind::Ident, "标识符")?;
            parts.push(ast::Ident { span: ident.span });
        }
        Ok(parts)
    }

    fn consume_balanced(&mut self, open: Symbol, close: Symbol) -> Result<Span, ParseError> {
        let open_tok = self.expect_symbol(open)?;
        let start = open_tok.span.start;

        let mut depth = 1usize;
        while !self.peek_kind(TokenKind::Eof) {
            let tok = self.bump();
            if let TokenKind::Symbol(sym) = tok.kind {
                if sym == open {
                    depth += 1;
                } else if sym == close {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Span::new(start, tok.span.end));
                    }
                }
            }
        }

        Err(ParseError::UnterminatedGroup {
            close,
            span: Span::new(start, self.peek().span.end).into(),
        })
    }

    fn expect_keyword(&mut self, kw: Keyword) -> Result<Token, ParseError> {
        if self.peek_keyword(kw) {
            Ok(self.bump())
        } else {
            let tok = self.peek().clone();
            Err(ParseError::Expected {
                expected: kw_name(kw),
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    fn expect_symbol(&mut self, sym: Symbol) -> Result<Token, ParseError> {
        if self.peek_symbol(sym) {
            Ok(self.bump())
        } else {
            let tok = self.peek().clone();
            Err(ParseError::Expected {
                expected: sym_name(sym),
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    fn expect_kind(&mut self, kind: TokenKind, expected: &'static str) -> Result<Token, ParseError> {
        if self.peek_kind(kind) {
            Ok(self.bump())
        } else {
            let tok = self.peek().clone();
            Err(ParseError::Expected {
                expected,
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    fn eat_symbol(&mut self, sym: Symbol) -> bool {
        if self.peek_symbol(sym) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.i).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("lexer must produce at least EOF token")
        })
    }

    fn bump(&mut self) -> Token {
        let tok = self.peek().clone();
        self.i = (self.i + 1).min(self.tokens.len());
        tok
    }

    fn peek_kind(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn peek_keyword(&self, kw: Keyword) -> bool {
        self.peek().kind == TokenKind::Keyword(kw)
    }

    fn peek_symbol(&self, sym: Symbol) -> bool {
        self.peek().kind == TokenKind::Symbol(sym)
    }

    fn is_type_decl_start(&self) -> bool {
        self.peek_keyword(Keyword::Open)
            || self.peek_keyword(Keyword::Abstract)
            || self.peek_keyword(Keyword::Sealed)
            || self.peek_keyword(Keyword::Class)
            || self.peek_keyword(Keyword::Interface)
            || self.peek_keyword(Keyword::Struct)
            || self.peek_keyword(Keyword::Enum)
            || self.peek_keyword(Keyword::Effect)
    }

    fn is_top_level_item_start(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Keyword(
                Keyword::Package
                    | Keyword::Import
                    | Keyword::Fun
                    | Keyword::Val
                    | Keyword::Var
                    | Keyword::Open
                    | Keyword::Abstract
                    | Keyword::Sealed
                    | Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect
            )
        )
    }
}

fn kw_name(kw: Keyword) -> &'static str {
    match kw {
        Keyword::Open => "`open`",
        Keyword::Abstract => "`abstract`",
        Keyword::Sealed => "`sealed`",
        Keyword::Package => "`package`",
        Keyword::Import => "`import`",
        Keyword::Fun => "`fun`",
        Keyword::Val => "`val`",
        Keyword::Var => "`var`",
        Keyword::Class => "`class`",
        Keyword::Interface => "`interface`",
        Keyword::Struct => "`struct`",
        Keyword::Enum => "`enum`",
        Keyword::Effect => "`effect`",
        Keyword::Handle => "`handle`",
        Keyword::With => "`with`",
        Keyword::Perform => "`perform`",
        Keyword::Try => "`try`",
        Keyword::Catch => "`catch`",
        Keyword::Finally => "`finally`",
        Keyword::Async => "`async`",
        Keyword::Await => "`await`",
        Keyword::Return => "`return`",
        Keyword::If => "`if`",
        Keyword::Else => "`else`",
        Keyword::When => "`when`",
        Keyword::Is => "`is`",
        Keyword::As => "`as`",
        Keyword::AsQ => "`as?`",
    }
}

fn sym_name(sym: Symbol) -> &'static str {
    match sym {
        Symbol::At => "`@`",
        Symbol::LParen => "`(`",
        Symbol::RParen => "`)`",
        Symbol::LBrace => "`{`",
        Symbol::RBrace => "`}`",
        Symbol::LBracket => "`[`",
        Symbol::RBracket => "`]`",
        Symbol::Comma => "`,`",
        Symbol::Colon => "`:`",
        Symbol::Semicolon => "`;`",
        Symbol::Dot => "`.`",
        Symbol::Plus => "`+`",
        Symbol::Minus => "`-`",
        Symbol::Star => "`*`",
        Symbol::Slash => "`/`",
        Symbol::Eq => "`=`",
        Symbol::Lt => "`<`",
        Symbol::Gt => "`>`",
        Symbol::Bang => "`!`",
        Symbol::Question => "`?`",
        Symbol::Arrow => "`->`",
        Symbol::EqEq => "`==`",
        Symbol::BangEq => "`!=`",
        Symbol::LtEq => "`<=`",
        Symbol::GtEq => "`>=`",
        Symbol::AndAnd => "`&&`",
        Symbol::OrOr => "`||`",
        Symbol::BangBang => "`!!`",
        Symbol::QuestionDot => "`?.`",
        Symbol::Elvis => "`?:`",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_file() {
        let src = SourceFile::new_virtual("<mem>", "package a.b\n\nfun main() { val x = 1 }");
        let ast = parse_file(&src).unwrap();
        assert!(ast.package.is_some());
        assert_eq!(ast.imports.len(), 0);
        assert_eq!(ast.items.len(), 1);
    }

    #[test]
    fn parse_type_decls() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "open class A(x: Int) : B(x)\nstruct P(val x: Int)\nenum E { A, B }\nfun main() {}",
        );
        let ast = parse_file(&src).unwrap();
        assert_eq!(ast.items.len(), 4);
    }
}
