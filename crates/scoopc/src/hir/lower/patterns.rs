//! 模式相关 lowering（TODO T0103e）。
//!
//! 说明：
//! - 当前主要承载 `when` 的模式（`WhenPat`）AST → HIR 转换；
//! - 规则与 span 选择尽量保持既有行为稳定，避免 HIR fixtures 输出漂移。

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::char_literal::parse_char_literal;
use crate::ty::{RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::HirLowering;
use super::ValScope;
use super::types::ExpectedExpr;
use super::util::*;

use super::super::{
    Block, Expr, ExprKind, Item, LiteralKind, MemberAccess, MemberRef, Stmt, StmtKind, ValDecl,
    ValueRef, WhenArm, WhenPat,
};

impl<'a> HirLowering<'a> {
    pub(super) fn lower_top_level_pattern_val_items(
        &mut self,
        pkg_prefix: &str,
        v: &ast::ValDecl,
        out: &mut Vec<Item>,
    ) {
        let ast::ValBinding::Pattern(pattern) = &v.binding else {
            unreachable!("only top-level pattern val declarations should reach this helper");
        };

        let mut subject_decl = self.lower_val_decl(pkg_prefix, v, ValScope::TopLevel);
        let subject_ty = subject_decl.ty;
        let base_name = self.synthetic_top_level_pattern_base_name(v.span);
        let subject_name = format!("{base_name}__subject");
        let subject_fqn = join_prefix(pkg_prefix, &subject_name);
        subject_decl.id = Some(self.symbols.intern_top_level(subject_fqn.clone()));
        subject_decl.name = Some(subject_name);
        subject_decl.mutable = false;
        self.record_top_level_immutable_value(
            subject_fqn.clone(),
            v.span,
            subject_ty,
            subject_decl.init.clone(),
        );
        out.push(Item::Val(subject_decl));

        let subject_ref = self.synth_top_level_ref(pattern.span, subject_ty, subject_fqn);

        let check_fqn: Option<String> = if self.pattern_contains_variant(pattern) {
            let check_name = format!("{base_name}__check");
            let check_fqn = join_prefix(pkg_prefix, &check_name);
            let check_init = self.synth_pattern_runtime_check_expr(subject_ref.clone(), pattern);
            self.record_top_level_immutable_value(
                check_fqn.clone(),
                v.span,
                self.builtins.unit,
                Some(check_init.clone()),
            );
            out.push(Item::Val(ValDecl {
                span: v.span,
                id: Some(self.symbols.intern_top_level(check_fqn.clone())),
                name: Some(check_name),
                mutable: false,
                ty: self.builtins.unit,
                init: Some(check_init),
            }));
            Some(check_fqn)
        } else {
            None
        };

        for binder in v.binding.bound_idents() {
            let binder_name = binder.text(self.source).to_string();
            let binder_fqn = join_prefix(pkg_prefix, &binder_name);
            let binder_ty = self
                .typechecked_binding_ty(binder.span)
                .unwrap_or(self.builtins.any);
            let Some(mut init) = self.synth_pattern_binding_init_expr(
                subject_ref.clone(),
                pattern,
                binder.span,
                binder_ty,
            ) else {
                continue;
            };

            if let Some(check_fqn) = check_fqn.as_ref() {
                let check_ref =
                    self.synth_top_level_ref(pattern.span, self.builtins.unit, check_fqn.clone());
                init = self.sequence_expr(pattern.span, vec![check_ref], init);
            }

            self.record_top_level_immutable_value(
                binder_fqn.clone(),
                binder.span,
                binder_ty,
                Some(init.clone()),
            );
            out.push(Item::Val(ValDecl {
                span: v.span,
                id: Some(self.symbols.intern_top_level(binder_fqn)),
                name: Some(binder_name),
                mutable: false,
                ty: binder_ty,
                init: Some(init),
            }));
        }
    }

    pub(super) fn lower_when_arm(
        &mut self,
        pkg_prefix: &str,
        arm: &ast::WhenArm,
        expected: ExpectedExpr,
    ) -> WhenArm {
        WhenArm {
            span: arm.span,
            pat: self.lower_when_pat(&arm.pat),
            guard: arm.guard.as_ref().map(|e| self.lower_expr(pkg_prefix, e)),
            arrow_span: arm.arrow_span,
            body: self.lower_expr_with_expected(pkg_prefix, &arm.body, expected),
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
            ast::WhenPat::Bind { ident } => {
                let binder_ty = self
                    .typechecked_binding_ty(ident.span)
                    .unwrap_or(self.builtins.any);
                self.record_when_pat_binding_ty(ident.span, binder_ty);
                WhenPat::Bind {
                    span: ident.span,
                    id: self.intern_local_symbol(ident.span, false),
                    name: ident.text(self.source).to_string(),
                }
            }
            ast::WhenPat::Tuple { span, elements } => WhenPat::Tuple {
                span: *span,
                elements: elements.iter().map(|e| self.lower_when_pat(e)).collect(),
            },
            ast::WhenPat::Variant { span, path, args } => {
                let variant_name = path
                    .segments
                    .last()
                    .copied()
                    .expect("when variant pattern path should contain at least one segment");
                WhenPat::Variant {
                    span: *span,
                    name_span: variant_name.span,
                    name: variant_name.text(self.source).to_string(),
                    args: args.iter().map(|a| self.lower_when_pat(a)).collect(),
                }
            }
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

    pub(super) fn lower_local_pattern_val_stmt(
        &mut self,
        pkg_prefix: &str,
        stmt_span: Span,
        v: &ast::ValDecl,
        out: &mut Vec<Stmt>,
    ) {
        let ast::ValBinding::Pattern(pattern) = &v.binding else {
            unreachable!("only pattern val declarations should reach this helper");
        };

        let (subject_decl_span, subject_id, subject_name) =
            self.fresh_synthetic_local(stmt_span, "__destructure_subject", false);
        let mut subject_decl = self.lower_val_decl(pkg_prefix, v, ValScope::Local);
        let subject_ty = subject_decl.ty;
        subject_decl.id = Some(subject_id);
        subject_decl.name = Some(subject_name.clone());
        subject_decl.mutable = false;

        out.push(Stmt {
            span: stmt_span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(subject_decl),
        });

        let subject_ref = self.synth_local_ref(
            pattern.span,
            subject_ty,
            subject_id,
            subject_name,
            subject_decl_span,
        );

        if self.pattern_contains_variant(pattern) {
            out.push(Stmt {
                span: stmt_span,
                ty: self.builtins.unit,
                kind: StmtKind::Expr(
                    self.synth_pattern_runtime_check_expr(subject_ref.clone(), pattern),
                ),
            });
        }

        let mut binders = Vec::new();
        collect_pattern_binders(pattern, &mut binders);
        for binder in binders {
            let binder_ty = self
                .typechecked_binding_ty(binder.span)
                .unwrap_or(self.builtins.any);
            let Some(init) = self.synth_pattern_binding_init_expr(
                subject_ref.clone(),
                pattern,
                binder.span,
                binder_ty,
            ) else {
                continue;
            };

            out.push(Stmt {
                span: stmt_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: stmt_span,
                    id: Some(self.intern_local_symbol(binder.span, false)),
                    name: Some(binder.text(self.source).to_string()),
                    mutable: false,
                    ty: binder_ty,
                    init: Some(init),
                }),
            });
        }
    }

    fn synth_pattern_runtime_check_expr(&mut self, subject: Expr, pattern: &ast::Pattern) -> Expr {
        match &pattern.kind {
            ast::PatternKind::Wildcard
            | ast::PatternKind::Rest
            | ast::PatternKind::Bind(_)
            | ast::PatternKind::Missing => self.unit_expr(pattern.span),
            ast::PatternKind::Tuple(elements) => {
                let checks = elements
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, element)| match element.kind {
                        ast::PatternKind::Rest => None,
                        _ if !self.pattern_contains_variant(element) => None,
                        _ => {
                            let projected =
                                self.synth_tuple_member_access(subject.clone(), element.span, idx);
                            Some(self.synth_pattern_runtime_check_expr(projected, element))
                        }
                    })
                    .collect();
                self.unit_block_expr(pattern.span, checks)
            }
            ast::PatternKind::Struct { path, fields, .. } => {
                let checks = fields
                    .iter()
                    .filter_map(|field| {
                        let nested = field.value.as_deref()?;
                        if !self.pattern_contains_variant(nested) {
                            return None;
                        }
                        let projected =
                            self.synth_struct_field_access(subject.clone(), path, field);
                        Some(self.synth_pattern_runtime_check_expr(projected, nested))
                    })
                    .collect();
                self.unit_block_expr(pattern.span, checks)
            }
            ast::PatternKind::Variant { path, args } => {
                self.synth_variant_runtime_check_expr(subject, pattern.span, path, args)
            }
        }
    }

    fn synth_variant_runtime_check_expr(
        &mut self,
        subject: Expr,
        span: Span,
        path: &ast::TypePath,
        args: &[ast::Pattern],
    ) -> Expr {
        let variant_name = path
            .segments
            .last()
            .map(|segment| segment.text(self.source).to_string())
            .unwrap_or_default();
        let subject_ty = subject.ty;

        let mut nested_checks = Vec::new();
        let mut arm_args = Vec::with_capacity(args.len());
        for (idx, arg) in args.iter().enumerate() {
            match &arg.kind {
                ast::PatternKind::Rest => arm_args.push(WhenPat::Rest { span: arg.span }),
                _ if self.pattern_contains_variant(arg) => {
                    let (bind_span, bind_id, bind_name) =
                        self.fresh_synthetic_local(arg.span, "__destructure_check", false);
                    let bind_ty = self
                        .variant_pattern_synthetic_bind_ty(subject_ty, path, idx)
                        .unwrap_or(self.builtins.any);
                    self.record_when_pat_binding_ty(bind_span, bind_ty);
                    arm_args.push(WhenPat::Bind {
                        span: bind_span,
                        id: bind_id,
                        name: bind_name.clone(),
                    });
                    let nested_subject =
                        self.synth_local_ref(arg.span, bind_ty, bind_id, bind_name, bind_span);
                    nested_checks.push(self.synth_pattern_runtime_check_expr(nested_subject, arg));
                }
                _ => arm_args.push(WhenPat::Wildcard { span: arg.span }),
            }
        }

        Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::When {
                subject: Box::new(subject),
                arms: vec![
                    WhenArm {
                        span,
                        pat: WhenPat::Variant {
                            span,
                            name_span: path
                                .segments
                                .last()
                                .map(|segment| segment.span)
                                .unwrap_or(span),
                            name: variant_name,
                            args: arm_args,
                        },
                        guard: None,
                        arrow_span: span,
                        body: self.unit_block_expr(span, nested_checks),
                    },
                    WhenArm {
                        span,
                        pat: WhenPat::Else { span },
                        guard: None,
                        arrow_span: span,
                        body: self.synth_raise_null_assertion_failed(span),
                    },
                ],
            },
        }
    }

    fn synth_pattern_binding_init_expr(
        &mut self,
        subject: Expr,
        pattern: &ast::Pattern,
        target_span: Span,
        target_ty: TypeId,
    ) -> Option<Expr> {
        match &pattern.kind {
            ast::PatternKind::Wildcard | ast::PatternKind::Rest | ast::PatternKind::Missing => None,
            ast::PatternKind::Bind(ident) => (ident.span == target_span).then_some(subject),
            ast::PatternKind::Tuple(elements) => {
                for (idx, element) in elements.iter().enumerate() {
                    if matches!(element.kind, ast::PatternKind::Rest)
                        || !pattern_contains_binding(element, target_span)
                    {
                        continue;
                    }
                    let projected =
                        self.synth_tuple_member_access(subject.clone(), element.span, idx);
                    return self.synth_pattern_binding_init_expr(
                        projected,
                        element,
                        target_span,
                        target_ty,
                    );
                }
                None
            }
            ast::PatternKind::Struct { path, fields, .. } => {
                for field in fields {
                    match field.value.as_deref() {
                        Some(nested) if pattern_contains_binding(nested, target_span) => {
                            let projected =
                                self.synth_struct_field_access(subject.clone(), path, field);
                            return self.synth_pattern_binding_init_expr(
                                projected,
                                nested,
                                target_span,
                                target_ty,
                            );
                        }
                        None if field.name.span == target_span => {
                            return Some(self.synth_struct_field_access(
                                subject.clone(),
                                path,
                                field,
                            ));
                        }
                        _ => {}
                    }
                }
                None
            }
            ast::PatternKind::Variant { path, args } => {
                let target_index = args.iter().enumerate().find_map(|(idx, arg)| {
                    pattern_contains_binding(arg, target_span).then_some(idx)
                })?;
                let variant_name = path
                    .segments
                    .last()
                    .map(|segment| segment.text(self.source).to_string())
                    .unwrap_or_default();
                let subject_ty = subject.ty;

                let mut arm_args = Vec::with_capacity(args.len());
                let mut arm_body = None;
                for (idx, arg) in args.iter().enumerate() {
                    if matches!(arg.kind, ast::PatternKind::Rest) {
                        arm_args.push(WhenPat::Rest { span: arg.span });
                        continue;
                    }

                    if idx == target_index {
                        let (bind_span, bind_id, bind_name) =
                            self.fresh_synthetic_local(arg.span, "__destructure_extract", false);
                        let bind_ty = self
                            .variant_pattern_synthetic_bind_ty(subject_ty, path, idx)
                            .unwrap_or(self.builtins.any);
                        self.record_when_pat_binding_ty(bind_span, bind_ty);
                        arm_args.push(WhenPat::Bind {
                            span: bind_span,
                            id: bind_id,
                            name: bind_name.clone(),
                        });
                        let nested_subject =
                            self.synth_local_ref(arg.span, bind_ty, bind_id, bind_name, bind_span);
                        arm_body = self.synth_pattern_binding_init_expr(
                            nested_subject,
                            arg,
                            target_span,
                            target_ty,
                        );
                    } else {
                        arm_args.push(WhenPat::Wildcard { span: arg.span });
                    }
                }

                Some(Expr {
                    span: pattern.span,
                    ty: target_ty,
                    kind: ExprKind::When {
                        subject: Box::new(subject),
                        arms: vec![
                            WhenArm {
                                span: pattern.span,
                                pat: WhenPat::Variant {
                                    span: pattern.span,
                                    name_span: path
                                        .segments
                                        .last()
                                        .map(|segment| segment.span)
                                        .unwrap_or(pattern.span),
                                    name: variant_name,
                                    args: arm_args,
                                },
                                guard: None,
                                arrow_span: pattern.span,
                                body: arm_body.unwrap_or_else(|| self.unit_expr(pattern.span)),
                            },
                            WhenArm {
                                span: pattern.span,
                                pat: WhenPat::Else { span: pattern.span },
                                guard: None,
                                arrow_span: pattern.span,
                                body: self.synth_raise_null_assertion_failed(pattern.span),
                            },
                        ],
                    },
                })
            }
        }
    }

    fn pattern_contains_variant(&self, pattern: &ast::Pattern) -> bool {
        match &pattern.kind {
            ast::PatternKind::Variant { .. } => true,
            ast::PatternKind::Tuple(elements) => elements
                .iter()
                .any(|element| self.pattern_contains_variant(element)),
            ast::PatternKind::Struct { fields, .. } => fields.iter().any(|field| {
                field
                    .value
                    .as_deref()
                    .is_some_and(|nested| self.pattern_contains_variant(nested))
            }),
            ast::PatternKind::Wildcard
            | ast::PatternKind::Rest
            | ast::PatternKind::Bind(_)
            | ast::PatternKind::Missing => false,
        }
    }

    fn variant_pattern_synthetic_bind_ty(
        &mut self,
        subject_ty: TypeId,
        path: &ast::TypePath,
        field_index: usize,
    ) -> Option<TypeId> {
        let (enum_fqn, enum_args) = match self.types.kind(subject_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                ("scoop.core.Option".to_string(), vec![*inner])
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                (nominal.fqn.clone(), nominal.args.clone())
            }
            _ => return None,
        };

        let (enum_source, enum_file, enum_decl) =
            self.find_type_decl_in_compilation_unit(&enum_fqn)?;
        if !matches!(enum_decl.kind, ast::TypeKind::Enum) {
            return None;
        }
        if enum_decl.type_params.len() != enum_args.len() {
            return None;
        }

        let variant_name = path.segments.last()?.text(self.source);
        let body = enum_decl.body.as_ref()?;
        let variant = body.members.iter().find_map(|member| match member {
            ast::TypeMember::EnumVariant(variant)
                if variant.name.text(enum_source) == variant_name =>
            {
                Some(variant)
            }
            _ => None,
        })?;
        let field = variant.params.get(field_index)?;
        let param_map: HashMap<String, TypeId> = enum_decl
            .type_params
            .iter()
            .map(|param| param.name.text(enum_source).to_string())
            .zip(enum_args)
            .collect();
        resolve_field_type_id(
            enum_source,
            enum_file,
            self.index,
            field.ty.as_ref(),
            &param_map,
            self.types,
        )
    }

    fn find_type_decl_in_compilation_unit(
        &self,
        target_fqn: &str,
    ) -> Option<(&'a SourceFile, &'a ast::File, &'a ast::TypeDecl)> {
        self.compilation_unit.iter().find_map(|(source, file)| {
            let pkg_prefix = package_prefix(source, file.package.as_ref());
            find_type_decl_in_items(source, file, &pkg_prefix, &file.items, target_fqn)
        })
    }

    fn synth_local_ref(
        &mut self,
        span: Span,
        ty: TypeId,
        id: super::super::SymbolId,
        name: String,
        decl_span: Span,
    ) -> Expr {
        Expr {
            span,
            ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id,
                name,
                decl_span,
            }),
        }
    }

    fn synth_top_level_ref(&mut self, span: Span, ty: TypeId, fqn: String) -> Expr {
        Expr {
            span,
            ty,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn,
            }),
        }
    }

    fn synth_tuple_member_access(&mut self, receiver: Expr, span: Span, index: usize) -> Expr {
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: MemberAccess {
                    span,
                    name: format!("_{index}"),
                    resolved: None,
                },
            },
        }
    }

    fn synth_struct_field_access(
        &mut self,
        receiver: Expr,
        path: &ast::TypePath,
        field: &ast::StructPatternField,
    ) -> Expr {
        let field_name = field.name.text(self.source).to_string();
        Expr {
            span: field.span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: MemberAccess {
                    span: field.name.span,
                    name: field_name.clone(),
                    resolved: self.synth_struct_field_member_ref(
                        path,
                        &field_name,
                        field.name.span,
                    ),
                },
            },
        }
    }

    fn synth_struct_field_member_ref(
        &mut self,
        path: &ast::TypePath,
        field_name: &str,
        field_span: Span,
    ) -> Option<MemberRef> {
        let nominal_ty = self.lower_type_ref(&ast::TypeRef::Path(path.clone()));
        let owner_fqn = match self.types.kind(nominal_ty) {
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
            | TypeKind::Ref(RefTypeKind::Nominal(nominal)) => nominal.fqn.clone(),
            _ => return None,
        };
        let field_fqn = format!("{owner_fqn}.{field_name}");
        Some(MemberRef::Value {
            id: self.symbols.intern_top_level(field_fqn.clone()),
            fqn: field_fqn,
        })
        .filter(|_| !field_span.is_empty() || !owner_fqn.is_empty())
    }

    fn unit_expr(&self, span: Span) -> Expr {
        Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Literal(LiteralKind::Unit),
        }
    }

    fn unit_block_expr(&self, span: Span, steps: Vec<Expr>) -> Expr {
        if steps.is_empty() {
            return self.unit_expr(span);
        }

        let mut stmts: Vec<Stmt> = steps
            .into_iter()
            .map(|expr| Stmt {
                span: expr.span,
                ty: self.builtins.unit,
                kind: StmtKind::Expr(expr),
            })
            .collect();
        stmts.push(Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.unit_expr(span)),
        });

        Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Block(Block {
                span,
                ty: self.builtins.unit,
                stmts,
            }),
        }
    }

    fn sequence_expr(&self, span: Span, steps: Vec<Expr>, tail: Expr) -> Expr {
        if steps.is_empty() {
            return tail;
        }

        let mut stmts: Vec<Stmt> = steps
            .into_iter()
            .map(|expr| Stmt {
                span: expr.span,
                ty: self.builtins.unit,
                kind: StmtKind::Expr(expr),
            })
            .collect();
        let tail_ty = tail.ty;
        stmts.push(Stmt {
            span: tail.span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(tail),
        });

        Expr {
            span,
            ty: tail_ty,
            kind: ExprKind::Block(Block {
                span,
                ty: tail_ty,
                stmts,
            }),
        }
    }

    fn record_top_level_immutable_value(
        &mut self,
        fqn: String,
        span: Span,
        ty: TypeId,
        init: Option<Expr>,
    ) {
        self.top_level_immutable_values.insert(
            fqn.clone(),
            super::super::TopLevelImmutableValue {
                fqn,
                source_path: self.source.path().to_path_buf(),
                span,
                ty,
                init,
            },
        );
    }

    fn synthetic_top_level_pattern_base_name(&self, span: Span) -> String {
        let source_key = self.source.path().display().to_string();
        let digest = Sha256::digest(source_key.as_bytes());
        let bytes: [u8; 8] = digest[0..8]
            .try_into()
            .expect("sha256 output should contain 8 bytes");
        let source_hash = u64::from_le_bytes(bytes);
        format!(
            "__top_level_pattern_{source_hash:016x}_{}_{}",
            span.start, span.end
        )
    }
}

fn collect_pattern_binders(pattern: &ast::Pattern, out: &mut Vec<ast::Ident>) {
    match &pattern.kind {
        ast::PatternKind::Bind(ident) => out.push(*ident),
        ast::PatternKind::Tuple(elements) => {
            for element in elements {
                collect_pattern_binders(element, out);
            }
        }
        ast::PatternKind::Struct { fields, .. } => {
            for field in fields {
                match field.value.as_deref() {
                    Some(nested) => collect_pattern_binders(nested, out),
                    None => out.push(field.name),
                }
            }
        }
        ast::PatternKind::Variant { args, .. } => {
            for arg in args {
                collect_pattern_binders(arg, out);
            }
        }
        ast::PatternKind::Wildcard | ast::PatternKind::Rest | ast::PatternKind::Missing => {}
    }
}

fn pattern_contains_binding(pattern: &ast::Pattern, target_span: Span) -> bool {
    match &pattern.kind {
        ast::PatternKind::Bind(ident) => ident.span == target_span,
        ast::PatternKind::Tuple(elements) => elements
            .iter()
            .any(|element| pattern_contains_binding(element, target_span)),
        ast::PatternKind::Struct { fields, .. } => {
            fields.iter().any(|field| match field.value.as_deref() {
                Some(nested) => pattern_contains_binding(nested, target_span),
                None => field.name.span == target_span,
            })
        }
        ast::PatternKind::Variant { args, .. } => args
            .iter()
            .any(|arg| pattern_contains_binding(arg, target_span)),
        ast::PatternKind::Wildcard | ast::PatternKind::Rest | ast::PatternKind::Missing => false,
    }
}

fn find_type_decl_in_items<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
    owner_prefix: &str,
    items: &'a [ast::Item],
    target_fqn: &str,
) -> Option<(&'a SourceFile, &'a ast::File, &'a ast::TypeDecl)> {
    for item in items {
        match item {
            ast::Item::Type(ty) => {
                let fqn = join_prefix(owner_prefix, ty.name.text(source));
                if fqn == target_fqn {
                    return Some((source, file, ty));
                }
                if let Some(body) = &ty.body
                    && let Some(found) = find_type_decl_in_type_members(
                        source,
                        file,
                        &fqn,
                        &body.members,
                        target_fqn,
                    )
                {
                    return Some(found);
                }
            }
            ast::Item::Object(obj) => {
                let Some(name) = obj.name.as_ref() else {
                    continue;
                };
                let obj_fqn = join_prefix(owner_prefix, name.text(source));
                if let Some(body) = &obj.body
                    && let Some(found) = find_type_decl_in_type_members(
                        source,
                        file,
                        &obj_fqn,
                        &body.members,
                        target_fqn,
                    )
                {
                    return Some(found);
                }
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }

    None
}

fn find_type_decl_in_type_members<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
    owner_prefix: &str,
    members: &'a [ast::TypeMember],
    target_fqn: &str,
) -> Option<(&'a SourceFile, &'a ast::File, &'a ast::TypeDecl)> {
    for member in members {
        match member {
            ast::TypeMember::Type(nested) => {
                let fqn = join_prefix(owner_prefix, nested.name.text(source));
                if fqn == target_fqn {
                    return Some((source, file, nested));
                }
                if let Some(body) = &nested.body
                    && let Some(found) = find_type_decl_in_type_members(
                        source,
                        file,
                        &fqn,
                        &body.members,
                        target_fqn,
                    )
                {
                    return Some(found);
                }
            }
            ast::TypeMember::Object(obj) => {
                let Some(name) = obj.name.as_ref() else {
                    continue;
                };
                let obj_fqn = join_prefix(owner_prefix, name.text(source));
                if let Some(body) = &obj.body
                    && let Some(found) = find_type_decl_in_type_members(
                        source,
                        file,
                        &obj_fqn,
                        &body.members,
                        target_fqn,
                    )
                {
                    return Some(found);
                }
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }

    None
}
