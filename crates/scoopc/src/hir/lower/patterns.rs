//! 模式相关 lowering（TODO T0103e）。
//!
//! 说明：
//! - 当前主要承载 `when` 的模式（`WhenPat`）AST → HIR 转换；
//! - 规则与 span 选择尽量保持既有行为稳定，避免 HIR fixtures 输出漂移。

use crate::ast;
use crate::span::Span;
use crate::syntax::char_literal::parse_char_literal;

use super::HirLowering;

use super::super::{WhenArm, WhenPat};

impl<'a> HirLowering<'a> {
    pub(super) fn lower_when_arm(&mut self, pkg_prefix: &str, arm: &ast::WhenArm) -> WhenArm {
        WhenArm {
            span: arm.span,
            pat: self.lower_when_pat(&arm.pat),
            guard: arm.guard.as_ref().map(|e| self.lower_expr(pkg_prefix, e)),
            arrow_span: arm.arrow_span,
            body: self.lower_expr(pkg_prefix, &arm.body),
        }
    }

    pub(super) fn lower_when_pat(&mut self, pat: &ast::WhenPat) -> WhenPat {
        match pat {
            ast::WhenPat::Else { span } => WhenPat::Else { span: *span },
            ast::WhenPat::Or { span, pats } => WhenPat::Or {
                span: *span,
                pats: pats.iter().map(|p| self.lower_when_pat(p)).collect(),
            },
            ast::WhenPat::Wildcard { span } => WhenPat::Wildcard { span: *span },
            ast::WhenPat::Rest { span } => WhenPat::Rest { span: *span },
            ast::WhenPat::Is { is_span, ty } => WhenPat::Is {
                span: Span::new(is_span.start, ty.span().end),
                ty: self.lower_type_ref(ty),
            },
            ast::WhenPat::Bind { ident } => WhenPat::Bind {
                span: ident.span,
                id: self.intern_local_symbol(ident.span, false),
                name: ident.text(self.source).to_string(),
            },
            ast::WhenPat::Tuple { span, elements } => WhenPat::Tuple {
                span: *span,
                elements: elements.iter().map(|e| self.lower_when_pat(e)).collect(),
            },
            ast::WhenPat::Variant { span, name, args } => WhenPat::Variant {
                span: *span,
                name_span: name.span,
                name: name.text(self.source).to_string(),
                args: args.iter().map(|a| self.lower_when_pat(a)).collect(),
            },
            ast::WhenPat::IntLit { span } => WhenPat::IntLit { span: *span },
            ast::WhenPat::CharLit { span } => WhenPat::CharLit {
                span: *span,
                value: parse_char_literal(self.source.slice(*span))
                    .expect("lexer validated Char literal before HIR lowering"),
            },
            ast::WhenPat::StringLit { span } => WhenPat::StringLit { span: *span },
            ast::WhenPat::BoolLit { span } => {
                let value = self.source.slice(*span) == "true";
                WhenPat::BoolLit { span: *span, value }
            }
        }
    }
}
