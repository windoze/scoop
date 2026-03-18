//! 文件级结构解析：package/import/顶层 items。

use crate::ast;
use crate::syntax::token::{Keyword, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    pub(super) fn parse_file(mut self) -> Result<ast::File, ParseError> {
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
            if self.peek_keyword(Keyword::Val) || self.peek_keyword(Keyword::Var) {
                items.push(ast::Item::Val(self.parse_val_decl()?));
                continue;
            }
            if self.is_type_decl_start() {
                items.push(ast::Item::Type(self.parse_type_decl()?));
                continue;
            }

            // 早期阶段：遇到未知顶层结构直接报错（后续再做错误恢复）
            let tok = *self.peek();
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
}
