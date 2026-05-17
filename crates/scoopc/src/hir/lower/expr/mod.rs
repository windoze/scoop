//! 表达式 lowering（TODO T0103c）。
//!
//! 说明：
//! - 该模块只负责 AST → HIR 的表达式部分 lowering；
//! - 规则与 span 选择尽量保持与原先 `lower/mod.rs` 一致，避免 HIR fixtures 输出漂移。

use std::collections::HashMap;
use std::path::PathBuf;

use crate::ast;
use crate::resolve::{ConstructorOverload, ParamSig, Visibility};
use crate::span::Span;
use crate::syntax::char_literal::parse_char_literal;
use crate::syntax::float_literal::{FloatLiteralSuffix, parse_float_literal};
use crate::syntax::string_literal::parse_f_string_text_utf8;
use crate::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::HirLowering;
use super::types::*;
use super::util::*;

use super::super::{
    Block, CallArg, ClassLiteralExpr, ClosureExpr, ClosureId, EffectOpRef, Expr, ExprKind,
    HandleArm, HandleArmKind, HandleBinder, HandleExpr, HandleOp, LiteralKind, MemberAccess,
    MemberRef, Param, Stmt, StmtKind, StructLitField, TypeMetadataLiteralKind, ValDecl, ValueRef,
    WhenArm, WhenPat,
};

#[derive(Clone)]
pub(in crate::hir::lower) struct LoweredSpliceFieldContract {
    field_name: String,
    field_fqn: String,
    field_ty: TypeId,
}

#[derive(Clone)]
pub(in crate::hir::lower) struct WithUpdateGroupedValue {
    rest: Vec<ast::Ident>,
    value: Expr,
}

#[derive(Clone)]
pub(in crate::hir::lower) struct CallableParamPlan {
    decl_file: PathBuf,
    type_param_bindings: Vec<(String, TypeId)>,
    params: Vec<DefaultArgParamInfo>,
}

pub(in crate::hir::lower) struct CanonicalCallLoweringRequest<'b> {
    pkg_prefix: &'b str,
    call_span: Span,
    callee: Expr,
    source_args: &'b [ast::Expr],
    receiver: Option<Expr>,
    binding: crate::ast::CallArgBinding,
    plan: Option<CallableParamPlan>,
    call_ty: TypeId,
}

fn param_infos_from_ast_params(
    source: &crate::source::SourceFile,
    params: &[ast::Param],
) -> Vec<DefaultArgParamInfo> {
    params
        .iter()
        .map(|param| DefaultArgParamInfo {
            decl_span: param.name.span,
            name: param.name.text(source).to_string(),
            is_vararg: param.is_vararg,
            ty_ref: param.ty.clone(),
            default_value: param.default_value.clone(),
        })
        .collect()
}

fn push_type_param_names(
    source: &crate::source::SourceFile,
    stack: &mut Vec<String>,
    params: &[ast::TypeParam],
) -> usize {
    let start = stack.len();
    stack.extend(params.iter().map(|p| p.name.text(source).to_string()));
    start
}

fn find_fun_decl_with_type_params_in_type_member<'b>(
    source: &crate::source::SourceFile,
    member: &'b ast::TypeMember,
    decl_span: Span,
    type_params: &mut Vec<String>,
) -> Option<(&'b ast::FunDecl, Vec<String>)> {
    match member {
        ast::TypeMember::Fun(fun) if fun.name.span == decl_span => {
            let mut names = type_params.clone();
            names.extend(
                fun.type_params
                    .iter()
                    .map(|p| p.name.text(source).to_string()),
            );
            Some((fun, names))
        }
        ast::TypeMember::Type(ty) => {
            let start = push_type_param_names(source, type_params, &ty.type_params);
            let found = ty.body.as_ref().and_then(|body| {
                body.members.iter().find_map(|member| {
                    find_fun_decl_with_type_params_in_type_member(
                        source,
                        member,
                        decl_span,
                        type_params,
                    )
                })
            });
            type_params.truncate(start);
            found
        }
        ast::TypeMember::Object(obj) => obj.body.as_ref().and_then(|body| {
            body.members.iter().find_map(|member| {
                find_fun_decl_with_type_params_in_type_member(
                    source,
                    member,
                    decl_span,
                    type_params,
                )
            })
        }),
        _ => None,
    }
}

fn find_fun_decl_with_type_params<'b>(
    source: &crate::source::SourceFile,
    file: &'b ast::File,
    decl_span: Span,
) -> Option<(&'b ast::FunDecl, Vec<String>)> {
    let mut type_params = Vec::new();
    for item in &file.items {
        match item {
            ast::Item::Fun(fun) if fun.name.span == decl_span => {
                let names = fun
                    .type_params
                    .iter()
                    .map(|p| p.name.text(source).to_string())
                    .collect();
                return Some((fun, names));
            }
            ast::Item::Type(ty) => {
                let start = push_type_param_names(source, &mut type_params, &ty.type_params);
                let found = ty.body.as_ref().and_then(|body| {
                    body.members.iter().find_map(|member| {
                        find_fun_decl_with_type_params_in_type_member(
                            source,
                            member,
                            decl_span,
                            &mut type_params,
                        )
                    })
                });
                type_params.truncate(start);
                if found.is_some() {
                    return found;
                }
            }
            ast::Item::Object(obj) => {
                let found = obj.body.as_ref().and_then(|body| {
                    body.members.iter().find_map(|member| {
                        find_fun_decl_with_type_params_in_type_member(
                            source,
                            member,
                            decl_span,
                            &mut type_params,
                        )
                    })
                });
                if found.is_some() {
                    return found;
                }
            }
            _ => {}
        }
    }
    None
}

fn find_ctor_params_with_type_params_in_type_decl(
    source: &crate::source::SourceFile,
    decl: &ast::TypeDecl,
    ctor_span: Span,
    type_params: &mut Vec<String>,
) -> Option<(Vec<DefaultArgParamInfo>, Vec<String>)> {
    let start = push_type_param_names(source, type_params, &decl.type_params);
    if let Some(primary) = &decl.primary_ctor
        && primary.params_span == ctor_span
    {
        let names = type_params.clone();
        let params = param_infos_from_ast_params(source, &primary.params);
        type_params.truncate(start);
        return Some((params, names));
    }
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::SecondaryCtor(ctor) if ctor.span == ctor_span => {
                    let names = type_params.clone();
                    let params = param_infos_from_ast_params(source, &ctor.params);
                    type_params.truncate(start);
                    return Some((params, names));
                }
                ast::TypeMember::Type(nested) => {
                    if let Some(found) = find_ctor_params_with_type_params_in_type_decl(
                        source,
                        nested,
                        ctor_span,
                        type_params,
                    ) {
                        type_params.truncate(start);
                        return Some(found);
                    }
                }
                ast::TypeMember::Object(obj) => {
                    if let Some(obj_body) = &obj.body {
                        for nested in &obj_body.members {
                            if let ast::TypeMember::Type(nested_ty) = nested
                                && let Some(found) = find_ctor_params_with_type_params_in_type_decl(
                                    source,
                                    nested_ty,
                                    ctor_span,
                                    type_params,
                                )
                            {
                                type_params.truncate(start);
                                return Some(found);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    type_params.truncate(start);
    None
}

fn find_ctor_params_with_type_params(
    source: &crate::source::SourceFile,
    file: &ast::File,
    ctor_span: Span,
) -> Option<(Vec<DefaultArgParamInfo>, Vec<String>)> {
    let mut type_params = Vec::new();
    for item in &file.items {
        let ast::Item::Type(ty) = item else {
            continue;
        };
        if let Some(found) =
            find_ctor_params_with_type_params_in_type_decl(source, ty, ctor_span, &mut type_params)
        {
            return Some(found);
        }
    }
    None
}

fn const_value_splice_field_name(value: &crate::comptime::ConstValue) -> Option<String> {
    match value {
        crate::comptime::ConstValue::String(name) => Some(name.clone()),
        crate::comptime::ConstValue::Struct(value) => match value.fields.get("name") {
            Some(crate::comptime::ConstValue::String(name)) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

mod canonical_call;
mod main_lower;
mod members;
mod typechecked;
