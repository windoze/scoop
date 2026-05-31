//! 语法糖与特殊 case 的 lowering（TODO T0103e）。
//!
//! 说明：该模块集中 delegated properties（spec §10.4）与少量表达式糖的 lowering。

use crate::ast;
use crate::span::Span;
use crate::ty::TypeId;

use super::HirLowering;
use super::types::*;

use super::super::{
    CallArg, CtorCallInfo, Expr, ExprKind, LiteralKind, MemberAccess, MemberRef, StructLitField,
    ValueRef,
};

impl<'a> HirLowering<'a> {
    pub(super) fn try_lower_computed_property_assign(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Option<Expr> {
        let ast::ExprKind::MemberAccess { receiver, member } = &lhs.kind else {
            return None;
        };
        let resolved = self.resolved_member_for_lowering(member);
        let Some(ast::ResolvedMemberRef::Value { fqn }) = resolved.as_ref() else {
            return None;
        };
        if !self.computed_property_setters.contains(fqn) {
            return None;
        }

        let receiver = self.lower_expr(pkg_prefix, receiver);
        let value = self.lower_expr(pkg_prefix, rhs);
        let setter_fqn = super::computed_property_setter_fqn(fqn);
        let callee = Expr {
            span: member.span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(setter_fqn.clone()),
                fqn: setter_fqn,
            }),
        };

        Some(Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: vec![CallArg::Positional(receiver), CallArg::Positional(value)],
            },
        })
    }

    pub(super) fn try_lower_delegated_property_assign(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Option<Expr> {
        let ast::ExprKind::MemberAccess { receiver, member } = &lhs.kind else {
            return None;
        };
        let resolved = self.resolved_member_for_lowering(member);
        let Some(ast::ResolvedMemberRef::Value { fqn }) = resolved.as_ref() else {
            return None;
        };
        let info = self.delegated_properties.get(fqn.as_str()).cloned()?;

        match info {
            DelegatedPropertyInfo::Generic(info) => {
                let receiver = self.lower_expr(pkg_prefix, receiver);
                let this_ref = receiver.clone();
                let delegate = self.lower_generic_delegated_property_delegate_access_expr(
                    member.span,
                    receiver.clone(),
                    &info,
                );

                let meta = self.lower_property_meta_ref_expr(member.span, &info.property_meta_fqn);
                let value = self.lower_expr(pkg_prefix, rhs);

                if let Some(class_fqn) = info.delegate_class_fqn.as_ref() {
                    let setter_fqn = format!("{class_fqn}.setValue");
                    let receiver_ty = delegate.ty;
                    return Some(self.lower_synthetic_member_call_with_receiver_ty(
                        span,
                        delegate,
                        receiver_ty,
                        &setter_fqn,
                        vec![this_ref, meta, value],
                        self.builtins.unit,
                    ));
                }

                let callee = Expr {
                    span: member.span,
                    ty: self.builtins.any,
                    kind: ExprKind::MemberAccess {
                        receiver: Box::new(delegate),
                        member: MemberAccess {
                            span: member.span,
                            name: "setValue".to_string(),
                            resolved: None,
                        },
                    },
                };

                Some(Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Call {
                        callee: Box::new(callee),
                        args: vec![
                            CallArg::Positional(this_ref),
                            CallArg::Positional(meta),
                            CallArg::Positional(value),
                        ],
                    },
                })
            }
            DelegatedPropertyInfo::MapBacked => None,
        }
    }

    pub(super) fn lower_generic_delegated_property_delegate_access_expr(
        &mut self,
        span: Span,
        receiver: Expr,
        info: &GenericDelegatedPropertyInfo,
    ) -> Expr {
        let ty = info
            .delegate_class_fqn
            .as_ref()
            .map(|fqn| self.intern_nominal(fqn.clone(), Vec::new(), None))
            .unwrap_or(self.builtins.any);
        Expr {
            span,
            ty,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: MemberAccess {
                    span,
                    name: format!("{}$delegate", info.name),
                    resolved: Some(MemberRef::Value {
                        id: self
                            .symbols
                            .intern_top_level(info.delegate_field_fqn.clone()),
                        fqn: info.delegate_field_fqn.clone(),
                    }),
                },
            },
        }
    }

    pub(super) fn lower_property_meta_ref_expr(&mut self, span: Span, fqn: &str) -> Expr {
        let property_meta_ty =
            self.intern_nominal(Self::PROPERTY_META_FQN.to_string(), Vec::new(), None);
        let (owner_name, property_name) = fqn.split_once(".$PropertyMeta$").unwrap_or(("", fqn));

        Expr {
            span,
            ty: property_meta_ty,
            kind: ExprKind::StructLit {
                ty: property_meta_ty,
                fields: vec![
                    StructLitField {
                        span,
                        name: "name".to_string(),
                        name_span: span,
                        colon_span: span,
                        value: self.synth_string_expr(span, property_name),
                    },
                    StructLitField {
                        span,
                        name: "type".to_string(),
                        name_span: span,
                        colon_span: span,
                        value: self.type_meta_expr(span, "", "Primitive"),
                    },
                    StructLitField {
                        span,
                        name: "owner".to_string(),
                        name_span: span,
                        colon_span: span,
                        value: self.type_meta_expr(span, owner_name, "Class"),
                    },
                ],
            },
        }
    }

    fn synth_string_expr(&self, span: Span, value: &str) -> Expr {
        Expr {
            span,
            ty: self.builtins.string,
            kind: ExprKind::Literal(LiteralKind::SynthString(value.to_string())),
        }
    }

    fn type_meta_expr(&mut self, span: Span, name: &str, kind_name: &str) -> Expr {
        let type_meta_ty = self.intern_nominal("scoop.core.TypeMeta".to_string(), Vec::new(), None);
        let type_kind_ty = self.intern_nominal("scoop.core.TypeKind".to_string(), Vec::new(), None);

        Expr {
            span,
            ty: type_meta_ty,
            kind: ExprKind::StructLit {
                ty: type_meta_ty,
                fields: vec![
                    StructLitField {
                        span,
                        name: "name".to_string(),
                        name_span: span,
                        colon_span: span,
                        value: self.synth_string_expr(span, name),
                    },
                    StructLitField {
                        span,
                        name: "kind".to_string(),
                        name_span: span,
                        colon_span: span,
                        value: Expr {
                            span,
                            ty: type_kind_ty,
                            kind: ExprKind::UnresolvedIdent {
                                name: kind_name.to_string(),
                            },
                        },
                    },
                    StructLitField {
                        span,
                        name: "annotations".to_string(),
                        name_span: span,
                        colon_span: span,
                        value: self.empty_meta_list_expr(span),
                    },
                ],
            },
        }
    }

    fn empty_meta_list_expr(&mut self, span: Span) -> Expr {
        let annotation_meta_ty =
            self.intern_nominal("scoop.core.AnnotationMeta".to_string(), Vec::new(), None);
        let meta_list_ty = self.intern_nominal(
            "scoop.core.MetaList".to_string(),
            vec![annotation_meta_ty],
            None,
        );
        self.synthetic_class_ctor_call_expr(span, "scoop.core.MetaList", "MetaList", meta_list_ty)
    }

    fn synthetic_class_ctor_call_expr(
        &mut self,
        span: Span,
        class_fqn: &str,
        short_name: &str,
        ty: TypeId,
    ) -> Expr {
        let call_span = self.fresh_synthetic_call_site_span(span);
        let ctor_span = self
            .index
            .constructors
            .get(class_fqn)
            .and_then(|ctors| ctors.iter().find(|ctor| ctor.params.is_empty()))
            .map(|ctor| ctor.span);
        self.ctor_call_sites.insert(
            self.call_site(call_span),
            CtorCallInfo {
                class_fqn: class_fqn.to_string(),
                ctor_span,
                arg_mapping: Vec::new(),
            },
        );

        Expr {
            span: call_span,
            ty,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span: call_span,
                    ty: self.builtins.any,
                    kind: ExprKind::UnresolvedIdent {
                        name: short_name.to_string(),
                    },
                }),
                args: Vec::new(),
            },
        }
    }
}
