//! 文件级结构解析（grammar §2）：file annotations / package / import / 顶层 items。

use scoop2_base::Span;

use crate::ast::decl::*;
use crate::ast::{File, ImportDecl, Item, ItemKind, PackageDecl, TypePath};
use crate::token::{Keyword, Symbol, TokenKind};

use super::decls::{FunContext, fun_decl_span};
use super::{PResult, Parser};

impl<'a> Parser<'a> {
    pub(crate) fn parse_file_root(&mut self) -> File {
        let id = self.nid();
        let file_end = self.source.len();

        // `@file:...` 文件级注解：只在文件开头、literal target `file`（§2）。
        let mut file_annotations = Vec::new();
        while self.at_sym(Symbol::At)
            && self.peek_n(1).kind == TokenKind::Ident
            && self.at_sym_n(2, Symbol::Colon)
            && self.token_text(self.peek_n(1)) == "file"
        {
            match self.parse_annotation_use() {
                Ok(ann) => file_annotations.push(ann),
                Err(_abort) => self.recover_to_top_level_sync(),
            }
        }

        let package = if self.at_kw(Keyword::Package) {
            match self.parse_package_decl() {
                Ok(pkg) => Some(pkg),
                Err(_abort) => {
                    self.recover_to_top_level_sync();
                    None
                }
            }
        } else {
            None
        };

        let mut imports = Vec::new();
        while self.at_kw(Keyword::Import) {
            match self.parse_import_decl() {
                Ok(decl) => imports.push(decl),
                Err(_abort) => self.recover_to_top_level_sync(),
            }
        }

        let mut items = Vec::new();
        while !self.at_eof() {
            // 顶层允许孤立的 `;`（§1.1）。
            if self.eat_sym(Symbol::Semicolon) {
                continue;
            }

            let head = self.peek_after_modifiers().kind;

            let parsed: PResult<ItemKind> = match head {
                TokenKind::Keyword(Keyword::Typealias) => {
                    self.parse_typealias_decl().map(ItemKind::TypeAlias)
                }
                TokenKind::Keyword(Keyword::Fun) => {
                    self.parse_fun_decl(FunContext::TopLevel).map(ItemKind::Fun)
                }
                TokenKind::Keyword(Keyword::Val | Keyword::Var) => {
                    if self.is_extension_property_decl_start() {
                        self.parse_extension_property_decl()
                            .map(ItemKind::ExtensionProperty)
                    } else {
                        self.parse_top_level_val_decl().map(ItemKind::Val)
                    }
                }
                TokenKind::Keyword(Keyword::Object) => {
                    self.parse_object_decl(false).map(ItemKind::Object)
                }
                TokenKind::Keyword(
                    Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect,
                ) => self.parse_type_decl().map(ItemKind::Type),
                _ => {
                    let tok = self.peek();
                    Err(self.err_expected("顶层声明（例如 `fun`）", tok))
                }
            };

            match parsed {
                Ok(kind) => {
                    let span = item_kind_span(&kind);
                    items.push(Item {
                        id: self.nid(),
                        span,
                        kind,
                    });
                }
                Err(_abort) => self.recover_to_top_level_sync(),
            }
        }

        File {
            id,
            span: Span::new(0, file_end),
            file_annotations,
            package,
            imports,
            items,
        }
    }

    fn parse_package_decl(&mut self) -> PResult<PackageDecl> {
        let kw = self.expect_kw(Keyword::Package)?;
        let id = self.nid();
        let path = self.parse_dotted_path()?;
        let end = path.span.end;
        self.eat_sym(Symbol::Semicolon);
        Ok(PackageDecl {
            id,
            span: Span::new(kw.span.start, end),
            path,
        })
    }

    fn parse_import_decl(&mut self) -> PResult<ImportDecl> {
        let kw = self.expect_kw(Keyword::Import)?;
        let id = self.nid();
        let first = self.expect_ident("标识符")?;
        let mut segments = vec![self.ident(first)];
        let mut wildcard: Option<Span> = None;
        let mut alias = None;
        let mut end = first.span.end;

        while self.at_sym(Symbol::Dot) {
            self.bump(); // `.`
            // `import a.b.*`：通配导入。
            if self.at_sym(Symbol::Star) {
                let star = self.bump();
                wildcard = Some(star.span);
                end = star.span.end;
                break;
            }
            let seg = self.expect_ident("标识符")?;
            end = seg.span.end;
            segments.push(self.ident(seg));
        }

        if self.at_kw(Keyword::As) {
            if wildcard.is_some() {
                // 通配导入不支持 alias（§2 dedicated error）。
                let tok = self.peek();
                return Err(self.err_expected("通配 import 不支持 alias", tok));
            }
            self.bump(); // `as`
            let name = self.expect_ident("标识符")?;
            end = name.span.end;
            alias = Some(self.ident(name));
        }

        self.eat_sym(Symbol::Semicolon);

        Ok(ImportDecl {
            id,
            span: Span::new(kw.span.start, end),
            path: TypePath {
                segments,
                span: Span::new(kw.span.start, end),
            },
            wildcard,
            alias,
        })
    }
}

fn item_kind_span(kind: &ItemKind) -> Span {
    match kind {
        ItemKind::TypeAlias(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or(d.name.span.start);
            Span::new(start, d.ty.span.end)
        }
        ItemKind::Fun(d) => fun_decl_span(d),
        ItemKind::Val(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or_else(|| match &d.binding {
                    ValBinding::Name(n) => n.span.start,
                    ValBinding::Pattern(p) => p.span.start,
                });
            let end = d
                .init
                .as_ref()
                .map(|e| e.span.end)
                .or_else(|| d.ty.as_ref().map(|t| t.span.end))
                .unwrap_or(match &d.binding {
                    ValBinding::Name(n) => n.span.end,
                    ValBinding::Pattern(p) => p.span.end,
                });
            Span::new(start, end)
        }
        ItemKind::ExtensionProperty(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or(d.receiver.span.start);
            let end = d
                .accessors
                .last()
                .map(|a| a.span.end)
                .or_else(|| d.init.as_ref().map(|e| e.span.end))
                .unwrap_or(d.ty.span.end);
            Span::new(start, end)
        }
        ItemKind::Object(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .or_else(|| d.name.as_ref().map(|n| n.span.start))
                .unwrap_or(0);
            let end = d
                .body
                .as_ref()
                .map(|b| b.span.end)
                .or_else(|| d.supertypes.last().map(|s| s.span.end))
                .or_else(|| d.name.as_ref().map(|n| n.span.end))
                .unwrap_or(start);
            Span::new(start, end)
        }
        ItemKind::Type(d) => {
            let start = d
                .annotations
                .first()
                .map(|a| a.span.start)
                .unwrap_or(d.name.span.start);
            let end = d
                .body
                .as_ref()
                .map(|b| b.span.end)
                .or_else(|| d.where_clause.as_ref().map(|w| w.span.end))
                .or_else(|| d.supertypes.last().map(|s| s.span.end))
                .or_else(|| d.primary_ctor.as_ref().map(|c| c.span.end))
                .or_else(|| d.type_params.as_ref().map(|t| t.span.end))
                .unwrap_or(d.name.span.end);
            Span::new(start, end)
        }
    }
}
