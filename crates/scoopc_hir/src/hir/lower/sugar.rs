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

        let receiver = self.lower_expr(pkg_prefix, receiver);
        let this_ref = receiver.clone();
        let value = self.lower_expr(pkg_prefix, rhs);
        let delegate = self.lower_generic_delegated_property_delegate_access_expr(
            member.span,
            receiver.clone(),
            &info,
            value.ty,
        );

        let meta = self.lower_property_meta_ref_expr(member.span, &info.property_meta_fqn);

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

    pub(super) fn lower_generic_delegated_property_delegate_access_expr(
        &mut self,
        span: Span,
        receiver: Expr,
        info: &GenericDelegatedPropertyInfo,
        property_ty: TypeId,
    ) -> Expr {
        let ty = self.specialized_delegated_property_delegate_ty(info, property_ty);
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

    pub(in crate::hir::lower) fn specialized_delegated_property_delegate_ty(
        &mut self,
        info: &GenericDelegatedPropertyInfo,
        property_ty: TypeId,
    ) -> TypeId {
        let base_ty = match (info.delegate_ty, self.typecheck_types) {
            (Some(ty), Some(typecheck_types)) => {
                let ty = self.types.re_intern_from(typecheck_types, ty);
                self.apply_active_type_param_bindings(ty)
            }
            _ => info
                .delegate_class_fqn
                .as_ref()
                .map(|fqn| self.intern_nominal(fqn.clone(), Vec::new(), None))
                .unwrap_or(self.builtins.any),
        };

        if self
            .nominal_type_arg_count(base_ty)
            .is_some_and(|count| count > 0)
        {
            return base_ty;
        }

        let Some(class_fqn) = info
            .delegate_class_fqn
            .as_ref()
            .cloned()
            .or_else(|| self.nominal_fqn_for_ty(base_ty))
        else {
            return base_ty;
        };

        if self.type_param_count_for_nominal_fqn(&class_fqn) == Some(1) {
            return self.intern_nominal(class_fqn, vec![property_ty], None);
        }

        base_ty
    }

    fn nominal_type_arg_count(&self, ty: TypeId) -> Option<usize> {
        match self.types.kind(ty) {
            crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal))
            | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nominal)) => {
                Some(nominal.args.len())
            }
            _ => None,
        }
    }

    pub(in crate::hir::lower) fn type_param_count_for_nominal_fqn(
        &self,
        target_fqn: &str,
    ) -> Option<usize> {
        for (source, file) in self.compilation_unit.iter().copied() {
            let prefix = super::package_prefix(source, file.package.as_ref());
            if let Some(count) = type_param_count_in_items(source, &file.items, &prefix, target_fqn)
            {
                return Some(count);
            }
        }
        None
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

fn join_fqn(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn type_param_count_in_items(
    source: &crate::source::SourceFile,
    items: &[ast::Item],
    prefix: &str,
    target_fqn: &str,
) -> Option<usize> {
    for item in items {
        match item {
            ast::Item::Type(ty) => {
                if let Some(count) = type_param_count_in_type_decl(source, ty, prefix, target_fqn) {
                    return Some(count);
                }
            }
            ast::Item::Object(obj) => {
                if let Some(count) =
                    type_param_count_in_object_decl(source, obj, prefix, target_fqn)
                {
                    return Some(count);
                }
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }
    None
}

fn type_param_count_in_type_decl(
    source: &crate::source::SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    target_fqn: &str,
) -> Option<usize> {
    let name = decl.name.text(source);
    let fqn = join_fqn(prefix, name);
    if fqn == target_fqn {
        return Some(decl.type_params.len());
    }

    let body = decl.body.as_ref()?;
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                if let Some(count) = type_param_count_in_type_decl(source, nested, &fqn, target_fqn)
                {
                    return Some(count);
                }
            }
            ast::TypeMember::Object(obj) => {
                if let Some(count) = type_param_count_in_object_decl(source, obj, &fqn, target_fqn)
                {
                    return Some(count);
                }
            }
            ast::TypeMember::Property(_)
            | ast::TypeMember::Fun(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::EnumVariant(_) => {}
        }
    }
    None
}

fn type_param_count_in_object_decl(
    source: &crate::source::SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    target_fqn: &str,
) -> Option<usize> {
    let obj_name = match &obj.name {
        Some(name) => name.text(source).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => return None,
        },
    };
    let fqn = join_fqn(prefix, &obj_name);
    let body = obj.body.as_ref()?;
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                if let Some(count) = type_param_count_in_type_decl(source, nested, &fqn, target_fqn)
                {
                    return Some(count);
                }
            }
            ast::TypeMember::Object(nested) => {
                if let Some(count) =
                    type_param_count_in_object_decl(source, nested, &fqn, target_fqn)
                {
                    return Some(count);
                }
            }
            ast::TypeMember::Property(_)
            | ast::TypeMember::Fun(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::EnumVariant(_) => {}
        }
    }
    None
}
