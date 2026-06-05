//! 文件级结构解析：package/import/顶层 items。

use crate::ast;
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
            inferred_fun_return_tys: std::cell::RefCell::new(std::collections::HashMap::new()),
            inferred_performed_effect_tys: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            inferred_handle_arm_effect_tys: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            inferred_handle_arm_op_type_args: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            safe_member_access_resolved: std::cell::RefCell::new(std::collections::HashMap::new()),
            typechecked_member_resolved: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            splice_field_contracts: std::cell::RefCell::new(std::collections::HashMap::new()),
            with_update_contracts: std::cell::RefCell::new(std::collections::HashMap::new()),
            assign_place_contracts: std::cell::RefCell::new(std::collections::HashMap::new()),
            continuation_resume_call_sites: std::cell::RefCell::new(
                std::collections::HashSet::new(),
            ),
            non_pure_continuation_resume_call_sites: std::cell::RefCell::new(
                std::collections::HashSet::new(),
            ),
            zero_arg_unit_call_sugar_sites: std::cell::RefCell::new(
                std::collections::HashSet::new(),
            ),
            top_level_fun_value_refs: std::cell::RefCell::new(std::collections::HashMap::new()),
            top_level_fun_call_bindings: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            typechecked_call_arg_bindings: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            typechecked_effect_op_call_bindings: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            typechecked_ctor_call_bindings: std::cell::RefCell::new(
                std::collections::HashMap::new(),
            ),
            release_hook_bindings: std::cell::RefCell::new(std::collections::HashMap::new()),
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
