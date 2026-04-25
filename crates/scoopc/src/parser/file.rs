//! 文件级结构解析：package/import/顶层 items。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    pub(super) fn parse_file(mut self) -> Result<ast::File, ParseError> {
        // spec §15.3：文件级注解（`@file:...`）必须出现在 `package/import` 之前。
        //
        // 注意：这里刻意只消费显式 `@file:` 前缀的注解，避免把普通顶层声明注解
        // （例如文件开头的 `@Unsafe fun f()`）误判为 file annotation。
        let mut file_annotations = Vec::new();
        while self.peek_symbol(Symbol::At)
            && self.peek_n(1).kind == TokenKind::Ident
            && self.peek_n(2).kind == TokenKind::Symbol(Symbol::Colon)
            && self
                .source_text
                .get(self.peek_n(1).span.start..self.peek_n(1).span.end)
                == Some("file")
        {
            match self.parse_annotation_use() {
                Ok(ann) => file_annotations.push(ann),
                Err(e) => {
                    self.record_error(e);
                    self.recover_to_top_level_sync();
                }
            }
        }

        let package = if self.peek_keyword(Keyword::Package) {
            match self.parse_package_decl() {
                Ok(pkg) => Some(pkg),
                Err(e) => {
                    self.record_error(e);
                    self.recover_to_top_level_sync();
                    None
                }
            }
        } else {
            None
        };

        let mut imports = Vec::new();
        while self.peek_keyword(Keyword::Import) {
            match self.parse_import_decl() {
                Ok(decl) => imports.push(decl),
                Err(e) => {
                    self.record_error(e);
                    self.recover_to_top_level_sync();
                }
            }
        }

        let mut items = Vec::new();
        while !self.peek_kind(TokenKind::Eof) {
            // T1220a：package-level `comptime if`（分支块内为顶层 items 列表）。
            if self.peek_keyword(Keyword::Comptime) {
                match self.parse_comptime_if_item() {
                    Ok(ci) => items.push(ast::Item::ComptimeIf(Box::new(ci))),
                    Err(e) => {
                        self.record_error(e);
                        self.recover_to_top_level_sync();
                    }
                }
                continue;
            }

            let head = self.peek_after_modifiers().kind;

            if head == TokenKind::Keyword(Keyword::Typealias) {
                match self.parse_typealias_decl() {
                    Ok(decl) => items.push(ast::Item::TypeAlias(Box::new(decl))),
                    Err(e) => {
                        self.record_error(e);
                        self.recover_to_top_level_sync();
                    }
                }
                continue;
            }
            if head == TokenKind::Keyword(Keyword::Fun) {
                match self.parse_fun_decl() {
                    Ok(decl) => items.push(ast::Item::Fun(Box::new(decl))),
                    Err(e) => {
                        self.record_error(e);
                        self.recover_to_top_level_sync();
                    }
                }
                continue;
            }
            if matches!(head, TokenKind::Keyword(Keyword::Val | Keyword::Var)) {
                if self.is_extension_property_decl_start() {
                    match self.parse_extension_property_decl() {
                        Ok(decl) => items.push(ast::Item::ExtensionProperty(Box::new(decl))),
                        Err(e) => {
                            self.record_error(e);
                            self.recover_to_top_level_sync();
                        }
                    }
                } else {
                    match self.parse_val_decl() {
                        Ok(decl) => items.push(ast::Item::Val(Box::new(decl))),
                        Err(e) => {
                            self.record_error(e);
                            self.recover_to_top_level_sync();
                        }
                    }
                }
                continue;
            }
            if head == TokenKind::Keyword(Keyword::Object) {
                match self.parse_object_decl() {
                    Ok(decl) => items.push(ast::Item::Object(Box::new(decl))),
                    Err(e) => {
                        self.record_error(e);
                        self.recover_to_top_level_sync();
                    }
                }
                continue;
            }
            if self.is_type_decl_start() {
                match self.parse_type_decl() {
                    Ok(decl) => items.push(ast::Item::Type(Box::new(decl))),
                    Err(e) => {
                        self.record_error(e);
                        self.recover_to_top_level_sync();
                    }
                }
                continue;
            }

            // T0220：顶层错误恢复：遇到未知结构不再立即退出，
            // 而是记录错误并跳到下一个同步点（下一个顶层 item 起始）。
            let tok = *self.peek();
            self.record_error(ParseError::Expected {
                expected: "顶层声明（例如 `fun`）",
                found: tok.kind,
                span: tok.span.into(),
            });
            self.recover_to_top_level_sync();
        }

        self.finish(ast::File {
            file_annotations,
            package,
            imports,
            items,
            inferred_expr_tys: std::cell::RefCell::new(std::collections::HashMap::new()),
            inferred_binding_tys: std::cell::RefCell::new(std::collections::HashMap::new()),
            inferred_performed_effect_tys: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            inferred_handle_arm_effect_tys: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            safe_member_access_resolved: std::cell::RefCell::new(std::collections::HashMap::new()),
            typechecked_member_resolved: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            continuation_resume_call_sites: std::cell::RefCell::new(
                std::collections::HashSet::new(),
            ),
            non_pure_continuation_resume_call_sites: std::cell::RefCell::new(
                std::collections::HashSet::new(),
            ),
            top_level_fun_value_refs: std::cell::RefCell::new(std::collections::HashMap::new()),
            top_level_fun_call_bindings: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            typechecked_effect_op_call_bindings: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            typechecked_ctor_call_bindings: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
        })
    }

    fn parse_comptime_if_item(&mut self) -> Result<ast::ComptimeIfItem, ParseError> {
        let comptime_tok = self.expect_keyword(Keyword::Comptime)?;
        let comptime_span = comptime_tok.span;

        if !self.peek_keyword(Keyword::If) {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "`if`（`comptime if`）",
                found: tok.kind,
                span: tok.span.into(),
            });
        }

        self.parse_comptime_if_item_after_comptime(comptime_span)
    }

    fn parse_comptime_if_item_after_comptime(
        &mut self,
        comptime_span: Span,
    ) -> Result<ast::ComptimeIfItem, ParseError> {
        let if_tok = self.expect_keyword(Keyword::If)?;
        let if_span = if_tok.span;

        self.expect_symbol(Symbol::LParen)?;
        let tok = *self.peek();
        let cond = self.try_parse_expr()?.ok_or(ParseError::Expected {
            expected: "表达式（package-level comptime if 条件）",
            found: tok.kind,
            span: tok.span.into(),
        })?;
        self.expect_symbol(Symbol::RParen)?;

        let then_branch = self.parse_item_block_in_comptime_if()?;

        let else_branch = if self.peek_keyword(Keyword::Else) {
            let else_tok = self.bump(); // `else`
            let else_span = else_tok.span;

            // 支持两种 else-if 形态：
            // - `else comptime if (...) { ... }`（既有风格）
            // - `else if (...) { ... }`（语法糖，T1220a）
            if self.peek_keyword(Keyword::Comptime) {
                let else_comptime = self.bump();
                let else_comptime_span = else_comptime.span;

                if !self.peek_keyword(Keyword::If) {
                    let tok = *self.peek();
                    return Err(ParseError::Expected {
                        expected: "`if`（`else comptime if`）",
                        found: tok.kind,
                        span: tok.span.into(),
                    });
                }

                let nested = self.parse_comptime_if_item_after_comptime(else_comptime_span)?;
                Some(Box::new(ast::ComptimeIfItemElse::If(Box::new(nested))))
            } else if self.peek_keyword(Keyword::If) {
                // `else if`：把 `comptime` 视为被省略的关键字。
                let implicit_comptime = Span::new(else_span.end, else_span.end);
                let nested = self.parse_comptime_if_item_after_comptime(implicit_comptime)?;
                Some(Box::new(ast::ComptimeIfItemElse::If(Box::new(nested))))
            } else if self.peek_symbol(Symbol::LBrace) {
                let block = self.parse_item_block_in_comptime_if()?;
                Some(Box::new(ast::ComptimeIfItemElse::Block(block)))
            } else {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "`{` / `if` / `comptime if`（package-level comptime if 的 else 分支）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }
        } else {
            None
        };

        let end = else_branch
            .as_deref()
            .map(|e| match e {
                ast::ComptimeIfItemElse::Block(b) => b.span.end,
                ast::ComptimeIfItemElse::If(i) => i.span.end,
            })
            .unwrap_or(then_branch.span.end);

        Ok(ast::ComptimeIfItem {
            span: Span::new(comptime_span.start, end),
            comptime_span,
            if_span,
            cond,
            then_branch,
            else_branch,
        })
    }

    fn parse_item_block_in_comptime_if(&mut self) -> Result<ast::ItemBlock, ParseError> {
        let open = self.expect_symbol(Symbol::LBrace)?;
        let start = open.span.start;

        let mut items = Vec::new();
        while !self.peek_symbol(Symbol::RBrace) && !self.peek_kind(TokenKind::Eof) {
            items.push(self.parse_one_item_in_comptime_if_block()?);
        }

        let close = self.expect_symbol(Symbol::RBrace)?;
        Ok(ast::ItemBlock {
            span: Span::new(start, close.span.end),
            items,
        })
    }

    fn parse_one_item_in_comptime_if_block(&mut self) -> Result<ast::Item, ParseError> {
        // 分支块内允许嵌套 package-level `comptime if`。
        if self.peek_keyword(Keyword::Comptime) {
            let ci = self.parse_comptime_if_item()?;
            return Ok(ast::Item::ComptimeIf(Box::new(ci)));
        }

        let head = self.peek_after_modifiers().kind;

        if head == TokenKind::Keyword(Keyword::Typealias) {
            return Ok(ast::Item::TypeAlias(Box::new(self.parse_typealias_decl()?)));
        }
        if head == TokenKind::Keyword(Keyword::Fun) {
            return Ok(ast::Item::Fun(Box::new(self.parse_fun_decl()?)));
        }
        if matches!(head, TokenKind::Keyword(Keyword::Val | Keyword::Var)) {
            if self.is_extension_property_decl_start() {
                return Ok(ast::Item::ExtensionProperty(Box::new(
                    self.parse_extension_property_decl()?,
                )));
            }
            return Ok(ast::Item::Val(Box::new(self.parse_val_decl()?)));
        }
        if head == TokenKind::Keyword(Keyword::Object) {
            return Ok(ast::Item::Object(Box::new(self.parse_object_decl()?)));
        }
        if self.is_type_decl_start() {
            return Ok(ast::Item::Type(Box::new(self.parse_type_decl()?)));
        }

        // v0：分支块内只允许顶层 items；语句/表达式应在此处报错。
        let tok = *self.peek();
        Err(ParseError::Expected {
            expected: "顶层声明（package-level comptime if 分支块内，例如 `fun`）",
            found: tok.kind,
            span: tok.span.into(),
        })
    }

    fn recover_to_top_level_sync(&mut self) {
        // 同步点（sync token）：
        // - brace depth 为 0 时的顶层 item 起始（fun/val/var/type decl/package/import）
        // - 或 EOF
        //
        // 说明：
        // - 这里只跟踪 `{}` 深度即可：顶层 item 不应出现在 type/function 的 body 内部；
        //   若发现 brace depth 非 0，则继续跳过直到回到 0。
        // - 该函数必须保证至少消耗一个 token，避免死循环。
        if self.peek_kind(TokenKind::Eof) {
            return;
        }

        let mut depth_brace = 0usize;

        let first = self.bump();
        if let TokenKind::Symbol(sym) = first.kind {
            match sym {
                Symbol::LBrace => depth_brace += 1,
                Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                _ => {}
            }
        }

        while !self.peek_kind(TokenKind::Eof) {
            if depth_brace == 0 && self.is_top_level_item_start() {
                break;
            }

            let tok = self.bump();
            if let TokenKind::Symbol(sym) = tok.kind {
                match sym {
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    _ => {}
                }
            }
        }
    }
}
