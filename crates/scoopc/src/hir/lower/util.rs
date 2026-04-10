//! HIR lowering 的通用 helper（TODO T0103b）。
//!
//! 说明：
//! - 该模块集中放置跨 lowering 分支复用的 helper（例如 FQN 拼接、closure capture 计算、注解参数解析）；
//! - 同时把 early-stage 的“临时特判/兼容逻辑”（delegated properties、@Extern、struct/enum layout 收集等）
//!   收拢到少数入口函数中，降低后续拆分 `expr.rs`/`stmt.rs`/`block.rs` 的重复与循环依赖风险。

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::resolve::Index;
use crate::source::SourceFile;
use crate::syntax::string_literal::parse_string_literal_utf8;
use crate::ty::{BuiltinTypes, RefTypeKind, TypeKind, TypeStore, ValueTypeKind};

use super::HirLowering;
use super::types::*;

use super::super::{
    Block, CallArg, Capture, ClassCtor, ClassCtorDelegation, ClassCtorKind, ClassCtorParam,
    ClassField, ClassInit, ClassInitIndex, ClassInitStep, CtorCallSiteIndex, EnumLayout,
    EnumLayoutIndex, EnumRepr, EnumVariantFieldLayout, EnumVariantLayout, ExternAbi, ExternFun,
    ExternFunIndex, InterpolatedStringPart, LiteralKind, MemberAccess, MemberRef,
    ObjectInit, ObjectInitIndex, ObjectInitStep, ObjectProperty, Param, StmtKind, StructCLayout,
    StructFieldLayout, StructLayout, StructLayoutIndex, SymbolId, ValueRef, WhenPat,
};

/// 计算 closure（lambda）的 capture set（自由变量集合）。
///
/// 规则（最小实现，供 T0711）：
/// - 只统计 `VarRef(Local)`；
/// - 以该 lambda 的 params 与其 body 内引入的局部声明（`val/var`、`when` binder、`handle` binder）为“本地声明”；
/// - 只把“在 body 中被引用但不属于本地声明”的 local 视为 capture；
/// - 遇到嵌套 closure 时不深入（由内层 closure 自己计算 captures）。
pub(super) fn compute_closure_captures(
    params: &[Param],
    body: &super::super::Expr,
    local_mutability: &HashMap<SymbolId, bool>,
) -> Vec<Capture> {
    let mut declared: HashSet<SymbolId> = params.iter().map(|p| p.id).collect();
    collect_declared_locals_in_expr(body, &mut declared);

    let mut used: HashMap<SymbolId, Capture> = HashMap::new();
    collect_used_locals_in_expr(body, &mut used);

    let mut captures: Vec<Capture> = used
        .into_values()
        .filter(|c| !declared.contains(&c.id))
        .collect();

    for c in &mut captures {
        c.mutable = local_mutability.get(&c.id).copied().unwrap_or(false);
    }

    // 稳定排序：按声明位置排序（同位置用 SymbolId 兜底）。
    captures.sort_by(|a, b| {
        a.decl_span
            .start
            .cmp(&b.decl_span.start)
            .then_with(|| a.decl_span.end.cmp(&b.decl_span.end))
            .then_with(|| a.id.as_u32().cmp(&b.id.as_u32()))
    });

    captures
}

fn collect_declared_locals_in_expr(expr: &super::super::Expr, declared: &mut HashSet<SymbolId>) {
    match &expr.kind {
        super::super::ExprKind::Missing
        | super::super::ExprKind::Literal(_)
        | super::super::ExprKind::VarRef(_)
        | super::super::ExprKind::UnresolvedIdent { .. }
        | super::super::ExprKind::Todo(_) => {}
        super::super::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_declared_locals_in_expr(&f.value, declared);
            }
        }
        super::super::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_declared_locals_in_expr(e, declared);
            }
        }
        super::super::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let InterpolatedStringPart::Expr { expr } = p {
                    collect_declared_locals_in_expr(expr, declared);
                }
            }
        }
        super::super::ExprKind::Unary { expr, .. } => {
            collect_declared_locals_in_expr(expr.as_ref(), declared)
        }
        super::super::ExprKind::Binary { lhs, rhs, .. } => {
            collect_declared_locals_in_expr(lhs.as_ref(), declared);
            collect_declared_locals_in_expr(rhs.as_ref(), declared);
        }
        super::super::ExprKind::TypeCheck { expr, .. }
        | super::super::ExprKind::Cast { expr, .. } => {
            collect_declared_locals_in_expr(expr.as_ref(), declared);
        }
        super::super::ExprKind::Block(block) => collect_declared_locals_in_block(block, declared),
        super::super::ExprKind::Closure(_) => {
            // 嵌套 closure：由其自身计算 capture set。
        }
        super::super::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_declared_locals_in_expr(cond, declared);
            collect_declared_locals_in_expr(then_branch, declared);
            if let Some(e) = else_branch.as_deref() {
                collect_declared_locals_in_expr(e, declared);
            }
        }
        super::super::ExprKind::When { subject, arms } => {
            collect_declared_locals_in_expr(subject, declared);
            for arm in arms {
                collect_declared_locals_in_when_pat(&arm.pat, declared);
                if let Some(g) = &arm.guard {
                    collect_declared_locals_in_expr(g, declared);
                }
                collect_declared_locals_in_expr(&arm.body, declared);
            }
        }
        super::super::ExprKind::MemberAccess { receiver, .. } => {
            collect_declared_locals_in_expr(receiver, declared)
        }
        super::super::ExprKind::Call { callee, args } => {
            collect_declared_locals_in_expr(callee, declared);
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_declared_locals_in_expr(e, declared),
                    CallArg::Named { value, .. } => {
                        collect_declared_locals_in_expr(value, declared)
                    }
                }
            }
        }
        super::super::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_declared_locals_in_expr(e, declared),
                    CallArg::Named { value, .. } => {
                        collect_declared_locals_in_expr(value, declared)
                    }
                }
            }
        }
        super::super::ExprKind::Handle(handle) => {
            collect_declared_locals_in_block(&handle.body, declared);
            for arm in &handle.arms {
                for b in &arm.op.binders {
                    declared.insert(b.id);
                }
                collect_declared_locals_in_expr(&arm.body, declared);
            }
            if let Some(finally) = &handle.finally {
                collect_declared_locals_in_block(finally, declared);
            }
        }
    }
}

fn collect_declared_locals_in_block(block: &Block, declared: &mut HashSet<SymbolId>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Val(v) => {
                if let Some(id) = v.id {
                    declared.insert(id);
                }
                if let Some(init) = &v.init {
                    collect_declared_locals_in_expr(init, declared);
                }
            }
            StmtKind::Expr(e) => collect_declared_locals_in_expr(e, declared),
            StmtKind::Assign { lhs, rhs, .. } => {
                collect_declared_locals_in_expr(lhs, declared);
                collect_declared_locals_in_expr(rhs, declared);
            }
            StmtKind::While { cond, body } => {
                collect_declared_locals_in_expr(cond, declared);
                collect_declared_locals_in_block(body, declared);
            }
            StmtKind::Return { value } => {
                if let Some(v) = value {
                    collect_declared_locals_in_expr(v, declared);
                }
            }
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
        }
    }
}

fn collect_declared_locals_in_when_pat(pat: &WhenPat, declared: &mut HashSet<SymbolId>) {
    match pat {
        WhenPat::Or { pats, .. } => {
            for p in pats {
                collect_declared_locals_in_when_pat(p, declared);
            }
        }
        WhenPat::Bind { id, .. } => {
            declared.insert(*id);
        }
        WhenPat::Tuple { elements, .. } => {
            for e in elements {
                collect_declared_locals_in_when_pat(e, declared);
            }
        }
        WhenPat::Variant { args, .. } => {
            for a in args {
                collect_declared_locals_in_when_pat(a, declared);
            }
        }
        WhenPat::Else { .. }
        | WhenPat::Wildcard { .. }
        | WhenPat::Rest { .. }
        | WhenPat::Is { .. }
        | WhenPat::IntLit { .. }
        | WhenPat::StringLit { .. }
        | WhenPat::BoolLit { .. } => {}
    }
}

fn collect_used_locals_in_expr(expr: &super::super::Expr, used: &mut HashMap<SymbolId, Capture>) {
    match &expr.kind {
        super::super::ExprKind::Missing
        | super::super::ExprKind::Literal(_)
        | super::super::ExprKind::UnresolvedIdent { .. }
        | super::super::ExprKind::Todo(_) => {}
        super::super::ExprKind::VarRef(v) => {
            let ValueRef::Local {
                id,
                name,
                decl_span,
            } = v
            else {
                return;
            };
            used.entry(*id).or_insert_with(|| Capture {
                id: *id,
                name: name.clone(),
                decl_span: *decl_span,
                mutable: false,
            });
        }
        super::super::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_used_locals_in_expr(&f.value, used);
            }
        }
        super::super::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_used_locals_in_expr(e, used);
            }
        }
        super::super::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let InterpolatedStringPart::Expr { expr } = p {
                    collect_used_locals_in_expr(expr, used);
                }
            }
        }
        super::super::ExprKind::Unary { expr, .. } => {
            collect_used_locals_in_expr(expr.as_ref(), used)
        }
        super::super::ExprKind::Binary { lhs, rhs, .. } => {
            collect_used_locals_in_expr(lhs.as_ref(), used);
            collect_used_locals_in_expr(rhs.as_ref(), used);
        }
        super::super::ExprKind::TypeCheck { expr, .. }
        | super::super::ExprKind::Cast { expr, .. } => {
            collect_used_locals_in_expr(expr.as_ref(), used);
        }
        super::super::ExprKind::Block(block) => {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StmtKind::Expr(e) => collect_used_locals_in_expr(e, used),
                    StmtKind::Val(v) => {
                        if let Some(init) = &v.init {
                            collect_used_locals_in_expr(init, used);
                        }
                    }
                    StmtKind::Assign { lhs, rhs, .. } => {
                        collect_used_locals_in_expr(lhs, used);
                        collect_used_locals_in_expr(rhs, used);
                    }
                    StmtKind::While { cond, body } => {
                        collect_used_locals_in_expr(cond, used);
                        // while body 是一个 block；其内部的局部声明不影响“使用”收集。
                        collect_used_locals_in_expr(
                            &super::super::Expr {
                                span: body.span,
                                ty: body.ty,
                                kind: super::super::ExprKind::Block(body.clone()),
                            },
                            used,
                        );
                    }
                    StmtKind::Return { value } => {
                        if let Some(v) = value {
                            collect_used_locals_in_expr(v, used);
                        }
                    }
                    StmtKind::Empty
                    | StmtKind::Break { .. }
                    | StmtKind::Continue { .. }
                    | StmtKind::Todo(_) => {}
                }
            }
        }
        super::super::ExprKind::Closure(_) => {
            // 嵌套 closure：由其自身计算 capture set。
        }
        super::super::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_used_locals_in_expr(cond, used);
            collect_used_locals_in_expr(then_branch, used);
            if let Some(e) = else_branch.as_deref() {
                collect_used_locals_in_expr(e, used);
            }
        }
        super::super::ExprKind::When { subject, arms } => {
            collect_used_locals_in_expr(subject, used);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_used_locals_in_expr(g, used);
                }
                collect_used_locals_in_expr(&arm.body, used);
            }
        }
        super::super::ExprKind::MemberAccess { receiver, .. } => {
            collect_used_locals_in_expr(receiver, used)
        }
        super::super::ExprKind::Call { callee, args } => {
            collect_used_locals_in_expr(callee, used);
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_used_locals_in_expr(e, used),
                    CallArg::Named { value, .. } => collect_used_locals_in_expr(value, used),
                }
            }
        }
        super::super::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_used_locals_in_expr(e, used),
                    CallArg::Named { value, .. } => collect_used_locals_in_expr(value, used),
                }
            }
        }
        super::super::ExprKind::Handle(handle) => {
            // handle body / arm body 里的 var refs 都算“使用”；binder 是否 capture 由 declared 集合处理。
            collect_used_locals_in_expr(
                &super::super::Expr {
                    span: handle.body.span,
                    ty: handle.body.ty,
                    kind: super::super::ExprKind::Block(handle.body.clone()),
                },
                used,
            );
            for arm in &handle.arms {
                collect_used_locals_in_expr(&arm.body, used);
            }
            if let Some(finally) = &handle.finally {
                collect_used_locals_in_expr(
                    &super::super::Expr {
                        span: finally.span,
                        ty: finally.ty,
                        kind: super::super::ExprKind::Block(finally.clone()),
                    },
                    used,
                );
            }
        }
    }
}

pub(super) fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(p) = package else {
        return String::new();
    };

    let mut out = String::new();
    for (idx, seg) in p.path.iter().enumerate() {
        if idx != 0 {
            out.push('.');
        }
        out.push_str(seg.text(source));
    }
    out
}

pub(super) fn collect_type_decl_kinds(
    pairs: &[(&SourceFile, &ast::File)],
) -> HashMap<String, ast::TypeKind> {
    let mut out: HashMap<String, ast::TypeKind> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };
            out.insert(fqn, ty.kind);
        }
    }
    out
}

pub(super) fn collect_object_inits(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> (ObjectInitIndex, CtorCallSiteIndex) {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties = DelegatedPropertyIndex::new();
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        type_kinds,
        &delegated_properties,
        types,
        builtins,
    );

    let mut out: ObjectInitIndex = HashMap::new();
    for item in &file.items {
        match item {
            ast::Item::Object(obj) => {
                collect_object_decl_inits(&mut ctx, &pkg_prefix, &pkg_prefix, obj, &mut out);
            }
            ast::Item::Type(ty) => {
                collect_objects_in_type_decl(&mut ctx, &pkg_prefix, &pkg_prefix, ty, &mut out);
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }

    let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
    (out, ctor_call_sites)
}

fn collect_objects_in_type_decl(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    out: &mut ObjectInitIndex,
) {
    let name = decl.name.text(ctx.source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);
    let Some(body) = &decl.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Object(obj) => {
                collect_object_decl_inits(ctx, pkg_prefix, &type_fqn, obj, out);
            }
            ast::TypeMember::Type(nested) => {
                collect_objects_in_type_decl(ctx, pkg_prefix, &type_fqn, nested, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_object_decl_inits(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    out: &mut ObjectInitIndex,
) {
    let Some(name) = object_decl_name(ctx.source, obj) else {
        return;
    };

    let fqn = join_prefix(owner_prefix, &name);
    let mut init = ObjectInit {
        fqn: fqn.clone(),
        properties: HashMap::new(),
        steps: Vec::new(),
    };

    if let Some(body) = &obj.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) => {
                    let name = p.name.text(ctx.source).to_string();
                    let mutable = matches!(p.kind, ast::ValKind::Var);
                    let ty =
                        p.ty.as_ref()
                            .map(|t| ctx.lower_type_ref(t))
                            .unwrap_or(ctx.builtins.any);
                    let has_init = p.init.is_some();
                    init.properties.insert(
                        name.clone(),
                        ObjectProperty {
                            name: name.clone(),
                            mutable,
                            ty,
                            has_init,
                        },
                    );

                    if let Some(expr) = p.init.as_ref() {
                        let lowered = ctx.lower_expr(pkg_prefix, expr);
                        init.steps.push(ObjectInitStep::PropertyInit {
                            name,
                            init: lowered,
                        });
                    }
                }
                ast::TypeMember::InitBlock(b) => {
                    let block = ctx.lower_block(pkg_prefix, &b.body);
                    init.steps.push(ObjectInitStep::InitBlock { block });
                }
                ast::TypeMember::Object(nested) => {
                    collect_object_decl_inits(ctx, pkg_prefix, &fqn, nested, out);
                }
                ast::TypeMember::Type(_)
                | ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }

    out.entry(fqn).or_insert(init);
}

pub(super) fn collect_class_inits(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> (ClassInitIndex, CtorCallSiteIndex) {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties = DelegatedPropertyIndex::new();
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        type_kinds,
        &delegated_properties,
        types,
        builtins,
    );

    let mut out: ClassInitIndex = HashMap::new();
    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_classes_in_type_decl(&mut ctx, &pkg_prefix, &pkg_prefix, ty, &mut out);
            }
            ast::Item::Object(obj) => {
                collect_classes_in_object_decl(&mut ctx, &pkg_prefix, &pkg_prefix, obj, &mut out);
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }
    let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
    (out, ctor_call_sites)
}

fn collect_classes_in_type_decl(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    out: &mut ClassInitIndex,
) {
    let name = decl.name.text(ctx.source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);

    if matches!(decl.kind, ast::TypeKind::Class) {
        collect_class_decl_init(ctx, pkg_prefix, &type_fqn, decl, out);
    }

    let Some(body) = &decl.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(ctx, pkg_prefix, &type_fqn, nested, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_classes_in_object_decl(ctx, pkg_prefix, &type_fqn, obj, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_classes_in_object_decl(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    out: &mut ClassInitIndex,
) {
    let Some(name) = object_decl_name(ctx.source, obj) else {
        return;
    };
    let obj_fqn = join_prefix(owner_prefix, &name);

    let Some(body) = &obj.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(ctx, pkg_prefix, &obj_fqn, nested, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_classes_in_object_decl(ctx, pkg_prefix, &obj_fqn, nested, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_class_decl_init(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    class_fqn: &str,
    decl: &ast::TypeDecl,
    out: &mut ClassInitIndex,
) {
    // resolver 使用 class name 的 span 作为 `this` 的 decl_span（T0313），因此这里提前 intern，
    // 以便后续 lowering 的 init blocks/ctor bodies 与 codegen 使用同一个 `SymbolId`。
    let this_id = ctx.intern_local_symbol(decl.name.span, false);

    // 仅记录“直接 superclass”的 FQN（class 单继承；interface 实现列表不计入）。
    // - typecheck（T0439）已保证“最多一个 class supertype”；
    // - 当前阶段若无法解析（例如缺失 import 或语法未覆盖），保持为 None 以便后端走最小行为。
    let super_class_fqn = decl
        .supertypes
        .iter()
        .filter_map(|s| {
            ctx.index
                .type_ref_to_fqn_in_file(ctx.source, ctx.file, &s.ty)
        })
        .find(|fqn| matches!(ctx.type_kinds.get(fqn), Some(ast::TypeKind::Class)));

    // class header 的 `: Base(args...)`：记录 super ctor args（若存在）。
    let (super_ctor_args_span, super_ctor_args) = decl
        .supertypes
        .iter()
        .find(|st| st.ctor_args_span.is_some())
        .map(|st| {
            let span = st.ctor_args_span;
            let args = st
                .ctor_args
                .iter()
                .map(|arg| ctx.lower_expr(pkg_prefix, arg))
                .collect::<Vec<_>>();
            (span, args)
        })
        .unwrap_or((None, Vec::new()));

    let mut init = ClassInit {
        fqn: class_fqn.to_string(),
        super_class_fqn,
        super_ctor_args_span,
        super_ctor_args,
        this_id,
        fields: Vec::new(),
        field_indices: HashMap::new(),
        steps: Vec::new(),
        ctors: Vec::new(),
    };

    let insert_field = |init: &mut ClassInit, field: ClassField| {
        if init.field_indices.contains_key(&field.fqn) {
            return;
        }
        let idx = init.fields.len() as u32;
        init.field_indices.insert(field.fqn.clone(), idx);
        init.fields.push(field);
    };

    // T0125：泛型 class 的 ctor 参数类型可能引用 type params（如 `T`），
    // 需要在 lowering 之前推入 type param 作用域，使 `lower_type_ref` 能够解析为 `TypeKind::Param`。
    ctx.push_type_params(&decl.type_params);

    // primary ctor（若存在）。注意：resolver 当前只会把”显式 primary ctor”加入 constructors overload set，
    // 因此这里也只收集显式 primary ctor。
    if let Some(primary) = &decl.primary_ctor {
        let mut params: Vec<ClassCtorParam> = Vec::with_capacity(primary.params.len());
        for p in &primary.params {
            let name = p.name.text(ctx.source).to_string();
            let id = ctx.intern_local_symbol(p.name.span, false);
            let ty =
                p.ty.as_ref()
                    .map(|t| ctx.lower_type_ref(t))
                    .unwrap_or(ctx.builtins.any);
            let is_property = p.kind.is_some();
            let property_field_fqn = is_property.then(|| format!("{class_fqn}.{name}"));

            params.push(ClassCtorParam {
                id,
                name: name.clone(),
                decl_span: p.name.span,
                ty,
                has_default: p.default_value.is_some(),
                is_property,
                property_field_fqn: property_field_fqn.clone(),
            });

            // `class C(val x: T)`：`x` 同时声明字段/属性，因此需要参与实例 layout，
            // 并在 ctor 执行时先从实参写入该字段（顺序由 codegen 决定）。
            if let Some(field_fqn) = property_field_fqn {
                let mutable = matches!(p.kind, Some(ast::ValKind::Var));
                insert_field(
                    &mut init,
                    ClassField {
                        fqn: field_fqn.clone(),
                        name,
                        mutable,
                        ty,
                    },
                );
            }
        }

        init.ctors.push(ClassCtor {
            kind: ClassCtorKind::Primary,
            span: primary.params_span,
            params,
            delegation: None,
            body: None,
        });
    }

    // type body：property initializer / init blocks / secondary ctors
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) => {
                    // v0：仍跳过显式 getter/setter（computed/accessor codegen 需要 function-level CFG）。
                    if p.getter.is_some() || p.setter.is_some() {
                        continue;
                    }

                    let name = p.name.text(ctx.source).to_string();
                    let ty =
                        p.ty.as_ref()
                            .map(|t| ctx.lower_type_ref(t))
                            .unwrap_or(ctx.builtins.any);

                    // delegated property（spec §10.4）：标准 delegates（lazy/observable/vetoable）与 map-backed。
                    if let Some(delegate_expr) = p.delegate.as_ref() {
                        match parse_std_delegate_expr(ctx.source, delegate_expr) {
                            Some(ParsedStdDelegateExpr::Lazy { mode, .. }) => {
                                // lazy：为属性生成两个隐藏字段：
                                // - `<name>$lazy_inited: Bool`
                                // - `<name>$lazy_value: T`
                                // - （可选）`<name>$lazy_mutex: Mutex`（当 mode 需要互斥锁时）
                                //
                                // getter 会在首次访问时写入 `<name>$lazy_value` 并把 `<name>$lazy_inited` 置 true。
                                let inited_fqn = format!("{class_fqn}.{name}$lazy_inited");
                                let value_fqn = format!("{class_fqn}.{name}$lazy_value");
                                let mutex_fqn = format!("{class_fqn}.{name}$lazy_mutex");

                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: inited_fqn.clone(),
                                        name: format!("{name}$lazy_inited"),
                                        mutable: true,
                                        ty: ctx.builtins.bool_,
                                    },
                                );
                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: value_fqn,
                                        name: format!("{name}$lazy_value"),
                                        mutable: true,
                                        ty,
                                    },
                                );

                                if mode.requires_mutex() {
                                    let mutex_ty = ctx.intern_nominal(
                                        HirLowering::SYNC_MUTEX_TYPE_FQN.to_string(),
                                        Vec::new(),
                                        None,
                                    );
                                    insert_field(
                                        &mut init,
                                        ClassField {
                                            fqn: mutex_fqn.clone(),
                                            name: format!("{name}$lazy_mutex"),
                                            mutable: false,
                                            ty: mutex_ty,
                                        },
                                    );
                                    init.steps.push(ClassInitStep::PropertyInit {
                                        field_fqn: mutex_fqn,
                                        init: ctx.call_top_level_fun(
                                            p.name.span,
                                            HirLowering::SYNC_MUTEX_CREATE_FQN,
                                            Vec::new(),
                                            mutex_ty,
                                        ),
                                    });
                                }

                                init.steps.push(ClassInitStep::PropertyInit {
                                    field_fqn: inited_fqn,
                                    init: super::super::Expr {
                                        span: p.name.span,
                                        ty: ctx.builtins.bool_,
                                        kind: super::super::ExprKind::Literal(LiteralKind::Bool(
                                            false,
                                        )),
                                    },
                                });
                            }
                            Some(ParsedStdDelegateExpr::Observable { initial, .. })
                            | Some(ParsedStdDelegateExpr::Vetoable { initial, .. }) => {
                                // observable/vetoable：在 early stage 采用“编译器内建 delegate”策略：
                                // - 把当前值落到真实字段 `<name>`；
                                // - 注入一个内部互斥锁字段 `<name>$delegate_mutex: Mutex`；
                                // - 在 getter/setter lowering 时通过该 mutex 保障并发可见性（T1326b）。
                                let mutex_fqn = format!("{class_fqn}.{name}$delegate_mutex");
                                let mutex_ty = ctx.intern_nominal(
                                    HirLowering::SYNC_MUTEX_TYPE_FQN.to_string(),
                                    Vec::new(),
                                    None,
                                );
                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: mutex_fqn.clone(),
                                        name: format!("{name}$delegate_mutex"),
                                        mutable: false,
                                        ty: mutex_ty,
                                    },
                                );
                                init.steps.push(ClassInitStep::PropertyInit {
                                    field_fqn: mutex_fqn,
                                    init: ctx.call_top_level_fun(
                                        p.name.span,
                                        HirLowering::SYNC_MUTEX_CREATE_FQN,
                                        Vec::new(),
                                        mutex_ty,
                                    ),
                                });

                                // 把当前值落到真实字段 `<name>`，并在初始化时写入 `initial`。
                                let field_fqn = format!("{class_fqn}.{name}");
                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: field_fqn.clone(),
                                        name,
                                        mutable: true,
                                        ty,
                                    },
                                );
                                let lowered = ctx.lower_expr(pkg_prefix, &initial);
                                init.steps.push(ClassInitStep::PropertyInit {
                                    field_fqn,
                                    init: lowered,
                                });
                            }
                            Some(ParsedStdDelegateExpr::MapBacked { delegate }) => {
                                // map-backed：早期阶段在初始化时把 `by data` 的值写入真实字段 `<name>`。
                                //
                                // 约束：目前只支持 delegate 为 `this.data` 这类“class 字段访问”，
                                // 并要求 delegate 类型存在同名字段（`data.<name>`）。
                                let field_fqn = format!("{class_fqn}.{name}");
                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: field_fqn.clone(),
                                        name: name.clone(),
                                        mutable: false,
                                        ty,
                                    },
                                );

                                let delegate_field_fqn = match &delegate.kind {
                                    ast::ExprKind::MemberAccess { member, .. } => {
                                        let Some(ast::ResolvedMemberRef::Value { fqn }) =
                                            member.resolved.as_ref()
                                        else {
                                            continue;
                                        };
                                        fqn.clone()
                                    }
                                    _ => continue,
                                };

                                let Some(idx) =
                                    init.field_indices.get(&delegate_field_fqn).copied()
                                else {
                                    continue;
                                };
                                let Some(delegate_field) = init.fields.get(idx as usize) else {
                                    continue;
                                };

                                let delegate_ty_fqn = match ctx.types.kind(delegate_field.ty) {
                                    TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                                        nominal.fqn.clone()
                                    }
                                    TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                                        nominal.fqn.clone()
                                    }
                                    _ => continue,
                                };

                                let delegate_member_fqn = format!("{delegate_ty_fqn}.{name}");
                                let delegate_recv = ctx.lower_expr(pkg_prefix, &delegate);
                                let init_expr = super::super::Expr {
                                    span: p.name.span,
                                    ty: ctx.builtins.any,
                                    kind: super::super::ExprKind::MemberAccess {
                                        receiver: Box::new(delegate_recv),
                                        member: MemberAccess {
                                            span: p.name.span,
                                            name: name.clone(),
                                            resolved: Some(MemberRef::Value {
                                                id: ctx
                                                    .symbols
                                                    .intern_top_level(delegate_member_fqn.clone()),
                                                fqn: delegate_member_fqn,
                                            }),
                                        },
                                    },
                                };
                                init.steps.push(ClassInitStep::PropertyInit {
                                    field_fqn,
                                    init: init_expr,
                                });
                            }
                            None => {
                                // 非标准 delegated property：当前阶段不纳入 class init side table。
                            }
                        }
                        continue;
                    }

                    // v0：只收集“具备 backing field” 的属性；delegate/getter/setter 的完整语义留到后续任务。
                    let field_fqn = format!("{class_fqn}.{name}");
                    let mutable = matches!(p.kind, ast::ValKind::Var);

                    insert_field(
                        &mut init,
                        ClassField {
                            fqn: field_fqn.clone(),
                            name,
                            mutable,
                            ty,
                        },
                    );

                    if let Some(expr) = p.init.as_ref() {
                        let lowered = ctx.lower_expr(pkg_prefix, expr);
                        init.steps.push(ClassInitStep::PropertyInit {
                            field_fqn,
                            init: lowered,
                        });
                    }
                }
                ast::TypeMember::InitBlock(b) => {
                    let block = ctx.lower_block(pkg_prefix, &b.body);
                    init.steps.push(ClassInitStep::InitBlock { block });
                }
                ast::TypeMember::SecondaryCtor(ctor) => {
                    let mut params: Vec<ClassCtorParam> = Vec::with_capacity(ctor.params.len());
                    for p in &ctor.params {
                        let name = p.name.text(ctx.source).to_string();
                        let id = ctx.intern_local_symbol(p.name.span, false);
                        let ty =
                            p.ty.as_ref()
                                .map(|t| ctx.lower_type_ref(t))
                                .unwrap_or(ctx.builtins.any);
                        params.push(ClassCtorParam {
                            id,
                            name,
                            decl_span: p.name.span,
                            ty,
                            has_default: p.default_value.is_some(),
                            is_property: false,
                            property_field_fqn: None,
                        });
                    }

                    let delegation = ctor.delegation_call.as_ref().map(|d| ClassCtorDelegation {
                        kind: d.kind,
                        span: d.span,
                        args: d
                            .args
                            .iter()
                            .map(|arg| ctx.lower_expr(pkg_prefix, arg))
                            .collect::<Vec<_>>(),
                    });
                    let body = ctx.lower_block(pkg_prefix, &ctor.body);
                    init.ctors.push(ClassCtor {
                        kind: ClassCtorKind::Secondary,
                        span: ctor.span,
                        params,
                        delegation,
                        body: Some(body),
                    });
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Fun(_)
                | ast::TypeMember::Type(_)
                | ast::TypeMember::Object(_) => {}
            }
        }
    }

    ctx.pop_type_params();
    out.entry(class_fqn.to_string()).or_insert(init);
}

pub(super) fn object_decl_name(source: &SourceFile, obj: &ast::ObjectDecl) -> Option<String> {
    match obj.name.as_ref() {
        Some(name) => Some(name.text(source).to_string()),
        None => match obj.kind {
            ast::ObjectKind::Companion => Some("Companion".to_string()),
            ast::ObjectKind::Object => None,
        },
    }
}

pub(super) fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

#[derive(Debug, Clone)]
enum ParsedStdDelegateExpr {
    Lazy {
        mode: StdLazyThreadSafetyMode,
        initializer_body: ast::Expr,
    },
    Observable {
        initial: ast::Expr,
        on_change: ast::LambdaExpr,
    },
    Vetoable {
        initial: ast::Expr,
        on_change: ast::LambdaExpr,
    },
    MapBacked {
        delegate: ast::Expr,
    },
}

fn unique_top_level_fun_fqn_from_callee(callee: &ast::Expr) -> Option<String> {
    let ast::ExprKind::Ident(id) = &callee.kind else {
        return None;
    };

    // 优先使用 resolver 写回的 call candidates（比 `resolved` 更稳健；可覆盖 overload set）。
    if let Some(call) = id.call.as_ref() {
        let mut funs: Vec<String> = call
            .candidates
            .iter()
            .filter_map(|c| match c {
                ast::CallCandidate::Fun { fqn } => Some(fqn.clone()),
                ast::CallCandidate::Constructor { .. } => None,
            })
            .collect();
        funs.sort();
        funs.dedup();
        if funs.len() == 1 {
            return Some(funs[0].clone());
        }
    }

    // fallback：若 resolver 已把 callee 绑定为唯一顶层函数，同样可用。
    match id.resolved.as_ref() {
        Some(ast::ResolvedValueRef::TopLevel { fqn }) => Some(fqn.clone()),
        _ => None,
    }
}

fn parse_lazy_thread_safety_mode(
    source: &SourceFile,
    expr: &ast::Expr,
) -> Option<StdLazyThreadSafetyMode> {
    // 目前仅支持最常见的枚举常量写法（用于 delegated property 的 early lowering）：
    // - `LazyThreadSafetyMode.None`
    // - `LazyThreadSafetyMode.Publication`
    // - `LazyThreadSafetyMode.Synchronized`
    //
    // 备注：这里优先从源文本切片解析，避免依赖 enum variant 的 resolver/typecheck 语义细节。
    let raw = source.slice(expr.span).trim();

    // 支持命名参数：`mode = LazyThreadSafetyMode.None`。
    let raw = raw
        .split_once('=')
        .map(|(_, rhs)| rhs.trim())
        .unwrap_or(raw);

    let raw = raw.strip_prefix("scoop.delegates.").unwrap_or(raw);
    match raw {
        "LazyThreadSafetyMode.None" => Some(StdLazyThreadSafetyMode::None),
        "LazyThreadSafetyMode.Publication" => Some(StdLazyThreadSafetyMode::Publication),
        "LazyThreadSafetyMode.Synchronized" => Some(StdLazyThreadSafetyMode::Synchronized),
        _ => None,
    }
}

fn parse_std_delegate_expr(
    source: &SourceFile,
    delegate_expr: &ast::Expr,
) -> Option<ParsedStdDelegateExpr> {
    match &delegate_expr.kind {
        ast::ExprKind::Call { callee, args } => {
            let fqn = unique_top_level_fun_fqn_from_callee(callee)?;

            // lazy：`lazy { ... }` / `lazy(mode) { ... }`
            if fqn == "scoop.delegates.lazy" {
                let last = args.last()?;
                let ast::ExprKind::Lambda(lam) = &last.kind else {
                    return None;
                };

                let mode = if args.len() >= 2 {
                    parse_lazy_thread_safety_mode(source, &args[0])
                        .unwrap_or_else(StdLazyThreadSafetyMode::default_for_lazy_call)
                } else {
                    StdLazyThreadSafetyMode::default_for_lazy_call()
                };
                return Some(ParsedStdDelegateExpr::Lazy {
                    mode,
                    initializer_body: (*lam.body).clone(),
                });
            }

            // observable/vetoable：`observable(init) { old, new -> ... }`
            if fqn == "scoop.delegates.observable" || fqn == "scoop.delegates.vetoable" {
                if args.len() < 2 {
                    return None;
                }
                let initial = args.first()?.clone();
                let last = args.last()?;
                let ast::ExprKind::Lambda(lam) = &last.kind else {
                    return None;
                };

                return if fqn == "scoop.delegates.observable" {
                    Some(ParsedStdDelegateExpr::Observable {
                        initial,
                        on_change: lam.clone(),
                    })
                } else {
                    Some(ParsedStdDelegateExpr::Vetoable {
                        initial,
                        on_change: lam.clone(),
                    })
                };
            }

            None
        }

        // map-backed：`val x: T by data`
        ast::ExprKind::Ident(_) | ast::ExprKind::MemberAccess { .. } => {
            Some(ParsedStdDelegateExpr::MapBacked {
                delegate: delegate_expr.clone(),
            })
        }

        _ => None,
    }
}

pub(super) fn collect_delegated_properties(
    pairs: &[(&SourceFile, &ast::File)],
) -> DelegatedPropertyIndex {
    let mut out: DelegatedPropertyIndex = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_delegated_properties_in_type_decl(source, ty, &pkg_prefix, &mut out);
                }
                ast::Item::Object(obj) => {
                    collect_delegated_properties_in_object_decl(source, obj, &pkg_prefix, &mut out);
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }

    out
}

fn collect_delegated_properties_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    out: &mut DelegatedPropertyIndex,
) {
    let local_name = source.slice(decl.name.span);
    let owner_fqn = join_prefix(prefix, local_name);

    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) if p.delegate.is_some() => {
                    let name = source.slice(p.name.span).to_string();
                    let prop_fqn = format!("{owner_fqn}.{name}");
                    let Some(delegate_expr) = p.delegate.as_ref() else {
                        continue;
                    };

                    let info = match parse_std_delegate_expr(source, delegate_expr) {
                        Some(ParsedStdDelegateExpr::Lazy {
                            mode,
                            initializer_body,
                        }) => {
                            let mutex_field_fqn = mode
                                .requires_mutex()
                                .then(|| format!("{owner_fqn}.{name}$lazy_mutex"));
                            DelegatedPropertyInfo::Lazy(LazyDelegatedPropertyInfo {
                                name: name.clone(),
                                ty: p.ty.clone(),
                                mode,
                                value_field_fqn: format!("{owner_fqn}.{name}$lazy_value"),
                                inited_field_fqn: format!("{owner_fqn}.{name}$lazy_inited"),
                                mutex_field_fqn,
                                initializer_body,
                            })
                        }
                        Some(ParsedStdDelegateExpr::Observable { on_change, .. }) => {
                            let mutex_field_fqn =
                                Some(format!("{owner_fqn}.{name}$delegate_mutex"));
                            DelegatedPropertyInfo::Observable(ObservableDelegatedPropertyInfo {
                                name: name.clone(),
                                ty: p.ty.clone(),
                                on_change,
                                mutex_field_fqn,
                            })
                        }
                        Some(ParsedStdDelegateExpr::Vetoable { on_change, .. }) => {
                            let mutex_field_fqn =
                                Some(format!("{owner_fqn}.{name}$delegate_mutex"));
                            DelegatedPropertyInfo::Vetoable(VetoableDelegatedPropertyInfo {
                                name: name.clone(),
                                ty: p.ty.clone(),
                                on_change,
                                mutex_field_fqn,
                            })
                        }
                        Some(ParsedStdDelegateExpr::MapBacked { .. }) => {
                            DelegatedPropertyInfo::MapBacked
                        }
                        None => {
                            let delegate_field_fqn = format!("{owner_fqn}.{name}$delegate");
                            let property_meta_fqn = format!("{owner_fqn}.$PropertyMeta${name}");
                            DelegatedPropertyInfo::Generic(GenericDelegatedPropertyInfo {
                                name: name.clone(),
                                delegate_field_fqn,
                                property_meta_fqn,
                            })
                        }
                    };

                    out.entry(prop_fqn).or_insert(info);
                }
                ast::TypeMember::Type(nested) => {
                    collect_delegated_properties_in_type_decl(source, nested, &owner_fqn, out);
                }
                ast::TypeMember::Object(obj) => {
                    collect_delegated_properties_in_object_decl(source, obj, &owner_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }
}

fn collect_delegated_properties_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    out: &mut DelegatedPropertyIndex,
) {
    let obj_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => return,
        },
    };

    let owner_fqn = join_prefix(prefix, &obj_name);
    let Some(body) = &obj.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Property(p) if p.delegate.is_some() => {
                let name = source.slice(p.name.span).to_string();
                let prop_fqn = format!("{owner_fqn}.{name}");
                let Some(delegate_expr) = p.delegate.as_ref() else {
                    continue;
                };

                let info = match parse_std_delegate_expr(source, delegate_expr) {
                    Some(ParsedStdDelegateExpr::Lazy {
                        mode,
                        initializer_body,
                    }) => {
                        let mutex_field_fqn = mode
                            .requires_mutex()
                            .then(|| format!("{owner_fqn}.{name}$lazy_mutex"));
                        DelegatedPropertyInfo::Lazy(LazyDelegatedPropertyInfo {
                            name: name.clone(),
                            ty: p.ty.clone(),
                            mode,
                            value_field_fqn: format!("{owner_fqn}.{name}$lazy_value"),
                            inited_field_fqn: format!("{owner_fqn}.{name}$lazy_inited"),
                            mutex_field_fqn,
                            initializer_body,
                        })
                    }
                    Some(ParsedStdDelegateExpr::Observable { on_change, .. }) => {
                        let mutex_field_fqn = Some(format!("{owner_fqn}.{name}$delegate_mutex"));
                        DelegatedPropertyInfo::Observable(ObservableDelegatedPropertyInfo {
                            name: name.clone(),
                            ty: p.ty.clone(),
                            on_change,
                            mutex_field_fqn,
                        })
                    }
                    Some(ParsedStdDelegateExpr::Vetoable { on_change, .. }) => {
                        let mutex_field_fqn = Some(format!("{owner_fqn}.{name}$delegate_mutex"));
                        DelegatedPropertyInfo::Vetoable(VetoableDelegatedPropertyInfo {
                            name: name.clone(),
                            ty: p.ty.clone(),
                            on_change,
                            mutex_field_fqn,
                        })
                    }
                    Some(ParsedStdDelegateExpr::MapBacked { .. }) => {
                        DelegatedPropertyInfo::MapBacked
                    }
                    None => {
                        let delegate_field_fqn = format!("{owner_fqn}.{name}$delegate");
                        let property_meta_fqn = format!("{owner_fqn}.$PropertyMeta${name}");
                        DelegatedPropertyInfo::Generic(GenericDelegatedPropertyInfo {
                            name: name.clone(),
                            delegate_field_fqn,
                            property_meta_fqn,
                        })
                    }
                };

                out.entry(prop_fqn).or_insert(info);
            }
            ast::TypeMember::Type(nested) => {
                collect_delegated_properties_in_type_decl(source, nested, &owner_fqn, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_delegated_properties_in_object_decl(source, nested, &owner_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

pub(super) fn collect_extern_funs(source: &SourceFile, file: &ast::File) -> ExternFunIndex {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut out: ExternFunIndex = HashMap::new();

    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };

        let Some(extern_fun) = extern_fun_of_decl(source, fun) else {
            continue;
        };

        let name = fun.name.text(source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name
        } else {
            format!("{pkg_prefix}.{name}")
        };

        out.insert(fqn, extern_fun);
    }

    out
}

#[derive(Debug, Default, Clone)]
struct ExternAnnotationArgs {
    name: Option<String>,
    lib: Option<String>,
}

fn parse_extern_annotation_args(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> ExternAnnotationArgs {
    let mut out = ExternAnnotationArgs::default();
    let mut seen_named = false;

    for arg in &ann.args {
        // 兼容两种“命名参数”形态：
        // - `name: "..."`（AnnotationArg.name）
        // - `name = "..."`（赋值表达式；更贴近 Kotlin 风格）
        let (key, value) = match &arg.name {
            Some(name_id) => (Some(name_id.text(source)), Some(&arg.value)),
            None => match &arg.value.kind {
                ast::ExprKind::Assign { lhs, rhs, .. } => match &lhs.kind {
                    ast::ExprKind::Ident(id) => (Some(source.slice(id.span)), Some(rhs.as_ref())),
                    _ => (None, None),
                },
                _ => (None, None),
            },
        };

        if let (Some(key), Some(value)) = (key, value) {
            seen_named = true;
            if !matches!(value.kind, ast::ExprKind::StringLit) {
                continue;
            }
            let text = source.slice(value.span);
            match key {
                "name" => out.name = parse_string_literal_utf8(text).ok(),
                "lib" => out.lib = parse_string_literal_utf8(text).ok(),
                _ => {}
            }
            continue;
        }

        // 位置参数：`@Extern("symbol")`（仅在未出现命名参数前生效）。
        if seen_named {
            continue;
        }
        if out.name.is_some() {
            continue;
        }
        if !matches!(arg.value.kind, ast::ExprKind::StringLit) {
            continue;
        }
        let text = source.slice(arg.value.span);
        out.name = parse_string_literal_utf8(text).ok();
    }

    out
}

fn extern_fun_of_decl(source: &SourceFile, fun: &ast::FunDecl) -> Option<ExternFun> {
    // 说明：
    // - `@Extern` 在语义上由 typecheck 校验（参数个数/类型等）；
    // - HIR lowering 只做“提取已校验信息”的 best-effort，避免把错误传播面扩到 HIR/LLVM 层。
    let name = fun.name.text(source);
    let calling_convention = fun.annotations.iter().find_map(|ann| {
        if !is_builtin_calling_convention_annotation(source, ann) {
            return None;
        }
        parse_calling_convention_annotation_arg(source, ann)
    });

    for ann in &fun.annotations {
        if !is_builtin_extern_annotation(source, ann) {
            continue;
        }

        // `@Extern`：缺省用函数名作为链接符号名；若显式提供 `name = "..."`（或位置参数），则覆写。
        let args = parse_extern_annotation_args(source, ann);
        let symbol = args.name.unwrap_or_else(|| name.to_string());

        return Some(ExternFun {
            abi: ExternAbi::C,
            symbol,
            calling_convention: calling_convention.clone(),
            lib: args.lib,
        });
    }

    None
}

pub(super) fn collect_extern_libs(pairs: &[(&SourceFile, &ast::File)]) -> Vec<String> {
    let mut libs: HashSet<String> = HashSet::new();

    for (source, file) in pairs {
        collect_extern_libs_in_file(source, file, &mut libs);
    }

    let mut out = libs.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

fn collect_extern_libs_in_file(source: &SourceFile, file: &ast::File, out: &mut HashSet<String>) {
    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                collect_extern_libs_in_annotations(source, &ta.annotations, out);
            }
            ast::Item::Fun(fun) => {
                collect_extern_libs_in_annotations(source, &fun.annotations, out);
            }
            ast::Item::ExtensionProperty(p) => {
                collect_extern_libs_in_annotations(source, &p.annotations, out);
            }
            ast::Item::Val(v) => {
                collect_extern_libs_in_annotations(source, &v.annotations, out);
            }
            ast::Item::Type(ty) => {
                collect_extern_libs_in_type_decl(source, ty, out);
            }
            ast::Item::Object(obj) => {
                collect_extern_libs_in_object_decl(source, obj, out);
            }
            // T1220a：package-level comptime if 在进入后续阶段前应被裁剪（TODO T1220b）。
            ast::Item::ComptimeIf(_ci) => {}
        }
    }
}

fn collect_extern_libs_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    out: &mut HashSet<String>,
) {
    collect_extern_libs_in_annotations(source, &decl.annotations, out);

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                collect_extern_libs_in_annotations(source, &v.annotations, out)
            }
            ast::TypeMember::Property(p) => {
                collect_extern_libs_in_annotations(source, &p.annotations, out)
            }
            ast::TypeMember::InitBlock(_b) => {}
            ast::TypeMember::SecondaryCtor(ctor) => {
                collect_extern_libs_in_annotations(source, &ctor.annotations, out);
                for p in &ctor.params {
                    collect_extern_libs_in_annotations(source, &p.annotations, out);
                }
            }
            ast::TypeMember::Fun(fun) => {
                collect_extern_libs_in_annotations(source, &fun.annotations, out)
            }
            ast::TypeMember::Type(nested) => collect_extern_libs_in_type_decl(source, nested, out),
            ast::TypeMember::Object(obj) => collect_extern_libs_in_object_decl(source, obj, out),
        }
    }
}

fn collect_extern_libs_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    out: &mut HashSet<String>,
) {
    collect_extern_libs_in_annotations(source, &obj.annotations, out);

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                collect_extern_libs_in_annotations(source, &v.annotations, out)
            }
            ast::TypeMember::Property(p) => {
                collect_extern_libs_in_annotations(source, &p.annotations, out)
            }
            ast::TypeMember::InitBlock(_b) => {}
            ast::TypeMember::SecondaryCtor(ctor) => {
                collect_extern_libs_in_annotations(source, &ctor.annotations, out);
                for p in &ctor.params {
                    collect_extern_libs_in_annotations(source, &p.annotations, out);
                }
            }
            ast::TypeMember::Fun(fun) => {
                collect_extern_libs_in_annotations(source, &fun.annotations, out)
            }
            ast::TypeMember::Type(nested) => collect_extern_libs_in_type_decl(source, nested, out),
            ast::TypeMember::Object(nested) => {
                collect_extern_libs_in_object_decl(source, nested, out)
            }
        }
    }
}

fn collect_extern_libs_in_annotations(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
    out: &mut HashSet<String>,
) {
    for ann in annotations {
        if !is_builtin_extern_annotation(source, ann) {
            continue;
        }
        let args = parse_extern_annotation_args(source, ann);
        if let Some(lib) = args.lib {
            if !lib.is_empty() {
                out.insert(lib);
            }
        }
    }
}

fn is_builtin_extern_annotation(source: &SourceFile, ann: &ast::AnnotationUse) -> bool {
    let segs = ann
        .path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>();
    matches!(segs.as_slice(), ["Extern"] | ["scoop", "core", "Extern"])
}

fn is_builtin_calling_convention_annotation(source: &SourceFile, ann: &ast::AnnotationUse) -> bool {
    let segs = ann
        .path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>();
    matches!(
        segs.as_slice(),
        ["CallingConvention"] | ["scoop", "core", "CallingConvention"]
    )
}

fn parse_calling_convention_annotation_arg(
    source: &SourceFile,
    ann: &ast::AnnotationUse,
) -> Option<String> {
    let arg = ann.args.first()?;

    // 兼容两种“命名参数”形态：
    // - `name: "..."`（AnnotationArg.name）
    // - `name = "..."`（赋值表达式；更贴近 Kotlin 风格）
    let (key, value) = match &arg.name {
        Some(name_id) => (Some(name_id.text(source)), Some(&arg.value)),
        None => match &arg.value.kind {
            ast::ExprKind::Assign { lhs, rhs, .. } => match &lhs.kind {
                ast::ExprKind::Ident(id) => (Some(source.slice(id.span)), Some(rhs.as_ref())),
                _ => (None, None),
            },
            _ => (None, Some(&arg.value)),
        },
    };

    if let Some(key) = key {
        if key != "name" {
            return None;
        }
    }

    if !matches!(value?.kind, ast::ExprKind::StringLit) {
        return None;
    }

    let text = source.slice(value?.span);
    parse_string_literal_utf8(text).ok()
}

fn annotation_use_resolves_to_fqn_in_file(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ann: &ast::AnnotationUse,
    expected_fqn: &str,
) -> bool {
    // 与 typecheck 阶段一致：复用 Index 的“按 package/import 规则解析类型名”的逻辑，
    // 避免仅按未限定名匹配导致的误判（同名但不同包的注解类）。
    let ty = ast::TypeRef::Path(ast::TypePath {
        span: ann.span,
        segments: ann.path.clone(),
        args: Vec::new(),
    });
    matches!(
        index.type_ref_to_fqn_in_file(source, file, &ty),
        Some(fqn) if fqn == expected_fqn
    )
}

fn extract_struct_clayout(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    annotations: &[ast::AnnotationUse],
) -> Option<StructCLayout> {
    const CLAYOUT_FQN: &str = "scoop.core.CLayout";
    let ann = annotations.iter().find(|ann| {
        annotation_use_resolves_to_fqn_in_file(source, file, index, ann, CLAYOUT_FQN)
    })?;

    Some(parse_clayout_annotation_args(source, ann))
}

fn parse_clayout_annotation_args(source: &SourceFile, ann: &ast::AnnotationUse) -> StructCLayout {
    // 说明：
    // - HIR lowering（dump/fixtures）不运行完整 typecheck，因此这里按 best-effort 解析；
    // - 实参的合法性与 GC-free 约束由 typecheck 阶段负责；
    // - 这里仅做“形态提取”，供后端在 typecheck 成功后消费。
    let mut aligned: Option<u32> = None;
    let mut packed: Option<u32> = None;

    for (pos, arg) in ann.args.iter().enumerate() {
        // 兼容三种参数形态（与 `@Extern` 一致）：
        // - `aligned: 16`（AnnotationArg.name）
        // - `aligned = 16`（赋值表达式；更贴近 Kotlin 风格）
        // - 位置参数：`@CLayout(16, 1)`（按顺序映射到 aligned/packed）
        let (key, value) = match &arg.name {
            Some(name_id) => (Some(name_id.text(source)), Some(&arg.value)),
            None => match &arg.value.kind {
                ast::ExprKind::Assign { lhs, rhs, .. } => match &lhs.kind {
                    ast::ExprKind::Ident(id) => (Some(source.slice(id.span)), Some(rhs.as_ref())),
                    _ => (None, None),
                },
                _ => (None, Some(&arg.value)),
            },
        };

        let key = match key {
            Some(key) => key,
            None => match pos {
                0 => "aligned",
                1 => "packed",
                _ => continue,
            },
        };
        let Some(value) = value else { continue };

        let ast::ExprKind::IntLit = value.kind else {
            continue;
        };
        let raw = source.slice(value.span);
        let Some(v) = parse_int_literal_decimal_u32(raw) else {
            continue;
        };
        let v = if v == 0 { None } else { Some(v) };

        match key {
            "aligned" => aligned = v,
            "packed" => packed = v,
            _ => {}
        }
    }

    StructCLayout { aligned, packed }
}

fn parse_int_literal_decimal_u32(text: &str) -> Option<u32> {
    let mut out: u128 = 0;
    for ch in text.chars() {
        if ch == '_' {
            continue;
        }
        let d = ch.to_digit(10)?;
        out = out.saturating_mul(10).saturating_add(u128::from(d));
    }
    u32::try_from(out).ok()
}

/// 收集当前编译单元（sysroot + 当前文件）里出现的 struct 字段布局信息。
///
/// 说明（早期阶段约束）：
/// - 仅收集**顶层 struct**；
/// - 仅使用 struct 的 primary ctor params 作为字段（与 resolver 对齐：`p.x` 来自 ctor param）；
/// - 暂不支持泛型 struct / `eff` 参数化 struct：这类布局需要单态化后再确定（留到后续任务）。
pub(super) fn collect_struct_layouts(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
) -> StructLayoutIndex {
    let mut out: StructLayoutIndex = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            if !matches!(ty.kind, ast::TypeKind::Struct) {
                continue;
            }

            // 泛型/eff 参数化 struct 的布局需要在 monomorphization 后才能稳定确定：
            // - field 的 type args 可能包含未绑定的 type params；
            // - ABI/layout 可能依赖实例化参数。
            if !ty.type_params.is_empty() || ty.eff_param.is_some() {
                continue;
            }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name.clone()
            } else {
                format!("{pkg_prefix}.{name}")
            };

            // 避免重复写入（例如 sysroot 与用户文件存在同名 type 时，resolver 会先报错）。
            if out.contains_key(&fqn) {
                continue;
            }

            let c_layout = extract_struct_clayout(source, file, index, &ty.annotations);

            let mut fields: Vec<StructFieldLayout> = Vec::new();
            if let Some(primary_ctor) = &ty.primary_ctor {
                for p in &primary_ctor.params {
                    let field_name = p.name.text(source).to_string();
                    let field_fqn = format!("{fqn}.{field_name}");
                    let ty_fqn =
                        p.ty.as_ref()
                            .and_then(|t| index.type_ref_to_fqn_in_file(source, file, t));

                    fields.push(StructFieldLayout {
                        span: p.name.span,
                        name: field_name,
                        fqn: field_fqn,
                        ty_fqn,
                    });
                }
            }

            out.insert(
                fqn.clone(),
                StructLayout {
                    fqn,
                    fields,
                    c_layout,
                },
            );
        }
    }

    out
}

/// 收集当前编译单元（sysroot + 当前文件）里出现的 enum variant 布局信息。
///
/// 说明（早期阶段约束）：
/// - 仅收集**顶层 enum**；
/// - 暂不支持泛型 enum / `eff` 参数化 enum（这类布局需要单态化后再确定，留到后续任务）；
/// - variant tag 按声明顺序分配，从 0 开始（与 typecheck/type env 的最小规则对齐）。
pub(super) fn collect_enum_layouts(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
) -> EnumLayoutIndex {
    let mut out: EnumLayoutIndex = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            if !matches!(ty.kind, ast::TypeKind::Enum) {
                continue;
            }

            // 泛型/eff 参数化 enum 的布局需要在 monomorphization 后才能稳定确定：
            // - payload 字段类型可能包含未绑定的 type params；
            // - 后端布局/boxing 策略可能依赖实例化参数。
            if !ty.type_params.is_empty() || ty.eff_param.is_some() {
                continue;
            }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name.clone()
            } else {
                format!("{pkg_prefix}.{name}")
            };

            if out.contains_key(&fqn) {
                continue;
            }

            let mut variants: Vec<EnumVariantLayout> = Vec::new();
            let mut repr: EnumRepr = EnumRepr::TaggedUnion;

            let Some(body) = &ty.body else {
                out.insert(
                    fqn.clone(),
                    EnumLayout {
                        fqn,
                        repr,
                        variants,
                    },
                );
                continue;
            };

            // spec §2.3.2.1：value-only enum。
            //
            // 当前阶段的判定策略（避免与 “enum implements interfaces” 的 `:` 语法冲突）：
            // - 只有当 enum body 内出现了显式判别值（`A = 0`）时，才把第一个 supertype 视为底层整型表示。
            if !ty.supertypes.is_empty()
                && body.members.iter().any(
                    |m| matches!(m, ast::TypeMember::EnumVariant(v) if v.discriminant.is_some()),
                )
            {
                let underlying_ty_fqn = ty
                    .supertypes
                    .first()
                    .and_then(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty));
                repr = EnumRepr::ValueOnly { underlying_ty_fqn };
            }

            let mut next_tag: u64 = 0;
            for member in &body.members {
                let ast::TypeMember::EnumVariant(v) = member else {
                    continue;
                };

                let variant_name = v.name.text(source).to_string();
                let tag = match repr {
                    EnumRepr::TaggedUnion => {
                        let tag = next_tag;
                        next_tag = next_tag.saturating_add(1);
                        tag
                    }
                    EnumRepr::ValueOnly { .. } => v
                        .discriminant
                        .as_ref()
                        .and_then(|e| eval_value_only_enum_discriminant(source, e))
                        .map(|v| v as u64)
                        .unwrap_or_else(|| {
                            let tag = next_tag;
                            next_tag = next_tag.saturating_add(1);
                            tag
                        }),
                };

                let mut fields: Vec<EnumVariantFieldLayout> = Vec::new();
                for p in &v.params {
                    let field_name = p.name.text(source).to_string();
                    let ty_fqn =
                        p.ty.as_ref()
                            .and_then(|t| index.type_ref_to_fqn_in_file(source, file, t));
                    fields.push(EnumVariantFieldLayout {
                        span: p.name.span,
                        name: field_name,
                        ty_fqn,
                    });
                }

                variants.push(EnumVariantLayout {
                    span: v.span,
                    name: variant_name,
                    tag,
                    fields,
                });
            }

            out.insert(
                fqn.clone(),
                EnumLayout {
                    fqn,
                    repr,
                    variants,
                },
            );
        }
    }

    out
}

/// 为参数化名义类型构造 mangled FQN（用作 struct_layouts/enum_layouts 的 key）。
///
/// 规则：
/// - 无 type args 时返回 base FQN 本身（如 `"pkg.Point"`）
/// - 有 type args 时返回 `"pkg.Pair<Int, String>"` 格式（与 TypeStore display 格式对齐）
pub fn mangle_nominal_fqn(fqn: &str, args: &[crate::ty::TypeId], types: &TypeStore) -> String {
    if args.is_empty() {
        return fqn.to_string();
    }
    let arg_str = args
        .iter()
        .map(|id| types.display(*id).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{fqn}<{arg_str}>")
}

/// 将 TypeId 转为 layout 索引中使用的 FQN 字符串。
///
/// 用途：为泛型 struct/enum 的字段类型生成 `StructFieldLayout.ty_fqn` / `EnumVariantFieldLayout.ty_fqn`。
/// 返回 `None` 表示无法确定（例如未知类型或未支持的类型类别）。
pub(super) fn type_id_to_layout_fqn(types: &TypeStore, ty: crate::ty::TypeId) -> Option<String> {
    match types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Unit) => Some("scoop.core.Unit".to_string()),
        TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool".to_string()),
        TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int".to_string()),
        TypeKind::Value(ValueTypeKind::UInt) => Some("scoop.core.UInt".to_string()),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(format!("scoop.core.Int{bits}")),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(format!("scoop.core.UInt{bits}")),
        TypeKind::Value(ValueTypeKind::Nothing) => Some("scoop.core.Nothing".to_string()),
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            Some(mangle_nominal_fqn(&nominal.fqn, &nominal.args, types))
        }
        TypeKind::Ref(RefTypeKind::Any) => Some("scoop.core.Any".to_string()),
        TypeKind::Ref(RefTypeKind::String) => Some("scoop.core.String".to_string()),
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            Some(mangle_nominal_fqn(&nominal.fqn, &nominal.args, types))
        }
        _ => None,
    }
}

/// 收集泛型 struct 的具体实例化布局（T0124）。
///
/// 在 typecheck 之后运行：扫描 TypeStore 中所有 `ValueTypeKind::Nominal`（args 非空），
/// 匹配到编译单元中声明的泛型 struct 后，为每个具体实例化生成 StructLayout。
///
/// 布局的 key 使用 mangled FQN（如 `"pkg.Pair<Int, String>"`），
/// 字段的 ty_fqn 通过 type param 替换为具体类型。
pub(super) fn collect_generic_struct_instantiation_layouts(
    pairs: &[(&SourceFile, &ast::File)],
    types: &TypeStore,
) -> StructLayoutIndex {
    // 1) 收集泛型 struct 声明：base_fqn → (source, decl)
    let mut generic_structs: HashMap<String, (&SourceFile, &ast::TypeDecl)> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else { continue };
            if !matches!(ty.kind, ast::TypeKind::Struct) { continue }
            if ty.type_params.is_empty() { continue }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() { name } else { format!("{pkg_prefix}.{name}") };
            generic_structs.insert(fqn, (source, ty));
        }
    }

    if generic_structs.is_empty() {
        return HashMap::new();
    }

    // 2) 扫描 TypeStore 中的具体实例化
    let mut out: StructLayoutIndex = HashMap::new();
    for ty_id in types.iter_ids() {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = types.kind(ty_id) else { continue };
        if nominal.args.is_empty() { continue }

        let Some((source, decl)) = generic_structs.get(&nominal.fqn) else { continue };

        let mangled = mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        if out.contains_key(&mangled) { continue }

        // 构建 type param name → concrete TypeId 映射
        let type_params = &decl.type_params;
        if type_params.len() != nominal.args.len() { continue }

        let mut param_map: HashMap<String, crate::ty::TypeId> = HashMap::new();
        for (idx, p) in type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            param_map.insert(name, nominal.args[idx]);
        }

        // 为每个字段解析 ty_fqn
        let mut fields: Vec<StructFieldLayout> = Vec::new();
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let field_name = p.name.text(source).to_string();
                let field_fqn = format!("{}.{field_name}", nominal.fqn);

                // 解析字段类型：优先检查是否为 type param，若是则替换为具体类型
                let ty_fqn = resolve_field_type_fqn(source, p.ty.as_ref(), &param_map, types);

                fields.push(StructFieldLayout {
                    span: p.name.span,
                    name: field_name,
                    fqn: field_fqn,
                    ty_fqn,
                });
            }
        }

        out.insert(mangled.clone(), StructLayout {
            fqn: mangled,
            fields,
            c_layout: None,
        });
    }

    out
}

/// 收集泛型 enum 的具体实例化布局（T0124）。
///
/// 与 `collect_generic_struct_instantiation_layouts` 类似，为泛型 enum 的具体实例化生成布局。
pub(super) fn collect_generic_enum_instantiation_layouts(
    pairs: &[(&SourceFile, &ast::File)],
    types: &TypeStore,
) -> EnumLayoutIndex {
    // 1) 收集泛型 enum 声明
    let mut generic_enums: HashMap<String, (&SourceFile, &ast::TypeDecl)> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else { continue };
            if !matches!(ty.kind, ast::TypeKind::Enum) { continue }
            if ty.type_params.is_empty() { continue }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() { name } else { format!("{pkg_prefix}.{name}") };
            generic_enums.insert(fqn, (source, ty));
        }
    }

    if generic_enums.is_empty() {
        return HashMap::new();
    }

    // 2) 扫描 TypeStore
    let mut out: EnumLayoutIndex = HashMap::new();
    for ty_id in types.iter_ids() {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = types.kind(ty_id) else { continue };
        if nominal.args.is_empty() { continue }

        let Some((source, decl)) = generic_enums.get(&nominal.fqn) else { continue };

        let mangled = mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        if out.contains_key(&mangled) { continue }

        let type_params = &decl.type_params;
        if type_params.len() != nominal.args.len() { continue }

        let mut param_map: HashMap<String, crate::ty::TypeId> = HashMap::new();
        for (idx, p) in type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            param_map.insert(name, nominal.args[idx]);
        }

        let mut variants: Vec<EnumVariantLayout> = Vec::new();
        let mut next_tag: u64 = 0;

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::EnumVariant(v) = member else { continue };
                let variant_name = v.name.text(source).to_string();
                let tag = next_tag;
                next_tag = next_tag.saturating_add(1);

                let mut fields: Vec<EnumVariantFieldLayout> = Vec::new();
                for p in &v.params {
                    let field_name = p.name.text(source).to_string();
                    let ty_fqn = resolve_field_type_fqn(source, p.ty.as_ref(), &param_map, types);
                    fields.push(EnumVariantFieldLayout {
                        span: p.name.span,
                        name: field_name,
                        ty_fqn,
                    });
                }

                variants.push(EnumVariantLayout {
                    span: v.span,
                    name: variant_name,
                    tag,
                    fields,
                });
            }
        }

        out.insert(mangled.clone(), EnumLayout {
            fqn: mangled,
            repr: EnumRepr::TaggedUnion,
            variants,
        });
    }

    out
}

/// 收集泛型 class 的具体实例化 ClassInit（T0125）。
///
/// 与 `collect_generic_struct_instantiation_layouts` 类似，为泛型 class（如 `class Box<T>`）
/// 的每个具体实例化（如 `Box<Int>`、`Box<String>`）生成独立的 ClassInit 条目。
///
/// 实例化的 ClassInit 使用 mangled FQN 作为 key（如 `"pkg.Box<Int>"`），
/// 字段的 TypeId 通过 type param 替换为具体类型（Param("T") → Int）。
pub(super) fn collect_generic_class_instantiation_inits(
    pairs: &[(&SourceFile, &ast::File)],
    types: &TypeStore,
    base_class_inits: &ClassInitIndex,
) -> ClassInitIndex {
    // 1) 收集泛型 class 声明：base_fqn → (source, decl)
    let mut generic_classes: HashMap<String, (&SourceFile, &ast::TypeDecl)> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        collect_generic_class_decls_in_items(source, &pkg_prefix, &pkg_prefix, &file.items, &mut generic_classes);
    }

    if generic_classes.is_empty() {
        return HashMap::new();
    }

    // 2) 扫描 TypeStore 中的具体实例化（class 是 ref type → RefTypeKind::Nominal）
    let mut out: ClassInitIndex = HashMap::new();
    for ty_id in types.iter_ids() {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(ty_id) else { continue };
        if nominal.args.is_empty() { continue }

        let Some((source, decl)) = generic_classes.get(&nominal.fqn) else { continue };

        let mangled = mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        if out.contains_key(&mangled) { continue }

        // base ClassInit 必须存在
        let Some(base_init) = base_class_inits.get(&nominal.fqn) else { continue };

        let type_params = &decl.type_params;
        if type_params.len() != nominal.args.len() { continue }

        // 构建 type param name → concrete TypeId 映射
        let mut param_map: HashMap<String, crate::ty::TypeId> = HashMap::new();
        for (idx, p) in type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            param_map.insert(name, nominal.args[idx]);
        }

        // 替换字段类型：Param("T") → 具体 TypeId
        let fields: Vec<ClassField> = base_init
            .fields
            .iter()
            .map(|f| ClassField {
                fqn: f.fqn.clone(),
                name: f.name.clone(),
                mutable: f.mutable,
                ty: substitute_type_param(types, f.ty, &param_map),
            })
            .collect();

        let mut field_indices = base_init.field_indices.clone();
        // 如果 field FQN 中使用了基础 FQN 前缀，替换为 mangled 版本不需要——
        // field FQN 使用原始 class FQN 前缀（如 "pkg.Box.inner"），保持不变。
        let _ = &field_indices; // 保留原始映射

        // 替换 ctor 参数类型
        let ctors: Vec<ClassCtor> = base_init
            .ctors
            .iter()
            .map(|ctor| ClassCtor {
                kind: ctor.kind,
                span: ctor.span,
                params: ctor
                    .params
                    .iter()
                    .map(|p| ClassCtorParam {
                        id: p.id,
                        name: p.name.clone(),
                        decl_span: p.decl_span,
                        ty: substitute_type_param(types, p.ty, &param_map),
                        has_default: p.has_default,
                        is_property: p.is_property,
                        property_field_fqn: p.property_field_fqn.clone(),
                    })
                    .collect(),
                delegation: ctor.delegation.clone(),
                body: ctor.body.clone(),
            })
            .collect();

        out.insert(mangled.clone(), ClassInit {
            fqn: mangled,
            super_class_fqn: base_init.super_class_fqn.clone(),
            super_ctor_args_span: base_init.super_ctor_args_span,
            super_ctor_args: base_init.super_ctor_args.clone(),
            this_id: base_init.this_id,
            fields,
            field_indices,
            steps: base_init.steps.clone(),
            ctors,
        });
    }

    out
}

/// 递归收集泛型 class 声明（支持嵌套在 type/object 内的 class）。
fn collect_generic_class_decls_in_items<'a>(
    source: &'a SourceFile,
    _pkg_prefix: &str,
    owner_prefix: &str,
    items: &'a [ast::Item],
    out: &mut HashMap<String, (&'a SourceFile, &'a ast::TypeDecl)>,
) {
    for item in items {
        match item {
            ast::Item::Type(ty) => {
                let name = ty.name.text(source).to_string();
                let fqn = join_prefix(owner_prefix, &name);

                if matches!(ty.kind, ast::TypeKind::Class) && !ty.type_params.is_empty() {
                    out.insert(fqn.clone(), (source, ty));
                }

                // 嵌套声明
                if let Some(body) = &ty.body {
                    for member in &body.members {
                        match member {
                            ast::TypeMember::Type(nested) => {
                                let nested_name = nested.name.text(source).to_string();
                                let nested_fqn = join_prefix(&fqn, &nested_name);
                                if matches!(nested.kind, ast::TypeKind::Class) && !nested.type_params.is_empty() {
                                    out.insert(nested_fqn, (source, nested));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            ast::Item::Object(obj) => {
                let obj_name = obj.name.as_ref().map(|n| n.text(source).to_string());
                if let Some(obj_name) = obj_name {
                    let obj_fqn = join_prefix(owner_prefix, &obj_name);
                    if let Some(body) = &obj.body {
                        for member in &body.members {
                            if let ast::TypeMember::Type(nested) = member {
                                let nested_name = nested.name.text(source).to_string();
                                let nested_fqn = join_prefix(&obj_fqn, &nested_name);
                                if matches!(nested.kind, ast::TypeKind::Class) && !nested.type_params.is_empty() {
                                    out.insert(nested_fqn, (source, nested));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// 替换 TypeId 中的 TypeKind::Param 为具体类型。
fn substitute_type_param(
    types: &TypeStore,
    ty: crate::ty::TypeId,
    param_map: &HashMap<String, crate::ty::TypeId>,
) -> crate::ty::TypeId {
    match types.kind(ty) {
        TypeKind::Param(p) => {
            param_map.get(&p.name).copied().unwrap_or(ty)
        }
        _ => ty,
    }
}

/// 解析字段的类型 FQN：如果字段类型是 type param，替换为具体类型的 FQN。
fn resolve_field_type_fqn(
    source: &SourceFile,
    ty_ref: Option<&ast::TypeRef>,
    param_map: &HashMap<String, crate::ty::TypeId>,
    types: &TypeStore,
) -> Option<String> {
    let ty_ref = ty_ref?;
    // 如果是简单路径（单段），检查是否为 type param
    if let ast::TypeRef::Path(path) = ty_ref {
        if path.segments.len() == 1 && path.args.is_empty() {
            let name = path.segments[0].text(source);
            if let Some(concrete_ty) = param_map.get(name) {
                return type_id_to_layout_fqn(types, *concrete_ty);
            }
        }
    }
    // 非 type param：暂不解析（泛型嵌套留到后续任务）
    None
}

/// T0126: 为所有具体的泛型 class 实例化生成单态化的成员方法 FunDecl。
///
/// 扫描 TypeStore 中的具体 class 实例化（例如 `Box<Int>`, `Box<String>`），
/// 为每个实例化的每个成员方法生成一个带具体类型的 FunDecl。
///
/// 生成的 FunDecl 的 FQN 使用 monomorph 格式：`"pkg.Box.get::<Int>"`,
/// 以与原始的 `"pkg.Box.get"`（含 Param 类型）共存于 `fun_index` 中。
pub(super) fn collect_generic_class_member_fun_instantiations(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Vec<super::super::FunDecl> {
    // 1) 收集泛型 class 声明：base_fqn → (source, file, decl)
    let mut generic_classes: HashMap<String, (&SourceFile, &ast::File, &ast::TypeDecl)> =
        HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        collect_generic_class_decls_with_file(
            source,
            file,
            &pkg_prefix,
            &pkg_prefix,
            &file.items,
            &mut generic_classes,
        );
    }

    if generic_classes.is_empty() {
        return Vec::new();
    }

    // 2) 收集 TypeStore 中所有具体实例化，去重
    let mut instantiations: Vec<(String, Vec<crate::ty::TypeId>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 需要先收集所有 TypeId，因为后面会 &mut types
    let all_ids: Vec<crate::ty::TypeId> = types.iter_ids().collect();
    for ty_id in all_ids {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(ty_id) else {
            continue;
        };
        if nominal.args.is_empty() {
            continue;
        }
        if !generic_classes.contains_key(&nominal.fqn) {
            continue;
        }
        // 跳过仍包含 Param 类型的实例化（例如 Box<T>）
        if nominal.args.iter().any(|&a| matches!(types.kind(a), TypeKind::Param(_))) {
            continue;
        }

        let mangled = mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        if seen.contains(&mangled) {
            continue;
        }
        seen.insert(mangled.clone());
        instantiations.push((nominal.fqn.clone(), nominal.args.clone()));
    }

    // 3) 为每个实例化的每个成员方法生成单态化 FunDecl
    let mut out: Vec<super::super::FunDecl> = Vec::new();

    for (base_fqn, concrete_args) in &instantiations {
        let Some((source, file, decl)) = generic_classes.get(base_fqn) else {
            continue;
        };

        let type_params = &decl.type_params;
        if type_params.len() != concrete_args.len() {
            continue;
        }

        // 构建 type param name → concrete TypeId 映射
        let bindings: Vec<(String, crate::ty::TypeId)> = type_params
            .iter()
            .zip(concrete_args.iter())
            .map(|(p, &arg)| (p.name.text(source).to_string(), arg))
            .collect();

        // 构建 monomorph 后缀（例如 "::<Int>"）
        let type_args_suffix = if concrete_args.is_empty() {
            String::new()
        } else {
            let args_str = concrete_args
                .iter()
                .map(|id| types.display(*id).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("::<{args_str}>")
        };

        // 遍历 class body 中的成员方法
        let Some(body) = &decl.body else {
            continue;
        };
        for member in &body.members {
            let ast::TypeMember::Fun(fun) = member else {
                continue;
            };

            let mut hir_fun = super::lower_member_fun_with_type_bindings(
                source,
                file,
                index,
                type_kinds,
                types,
                builtins,
                base_fqn,
                decl.name.span,
                concrete_args,
                fun,
                bindings.clone(),
            );

            // 重命名 FQN：例如 "pkg.Box.get" → "pkg.Box.get::<Int>"
            hir_fun.fqn = format!("{}{type_args_suffix}", hir_fun.fqn);

            out.push(hir_fun);
        }
    }

    out
}

/// 类似 `collect_generic_class_decls_in_items`，但同时记录 file 引用（用于单态化 lowering）。
fn collect_generic_class_decls_with_file<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
    _pkg_prefix: &str,
    owner_prefix: &str,
    items: &'a [ast::Item],
    out: &mut HashMap<String, (&'a SourceFile, &'a ast::File, &'a ast::TypeDecl)>,
) {
    for item in items {
        match item {
            ast::Item::Type(ty) => {
                let name = ty.name.text(source).to_string();
                let fqn = join_prefix(owner_prefix, &name);

                if matches!(ty.kind, ast::TypeKind::Class) && !ty.type_params.is_empty() {
                    out.insert(fqn.clone(), (source, file, ty));
                }

                // 嵌套声明
                if let Some(body) = &ty.body {
                    for member in &body.members {
                        if let ast::TypeMember::Type(nested) = member {
                            let nested_name = nested.name.text(source).to_string();
                            let nested_fqn = join_prefix(&fqn, &nested_name);
                            if matches!(nested.kind, ast::TypeKind::Class)
                                && !nested.type_params.is_empty()
                            {
                                out.insert(nested_fqn, (source, file, nested));
                            }
                        }
                    }
                }
            }
            ast::Item::Object(obj) => {
                let obj_name = obj.name.as_ref().map(|n| n.text(source).to_string());
                if let Some(obj_name) = obj_name {
                    let obj_fqn = join_prefix(owner_prefix, &obj_name);
                    if let Some(body) = &obj.body {
                        for member in &body.members {
                            if let ast::TypeMember::Type(nested) = member {
                                let nested_name = nested.name.text(source).to_string();
                                let nested_fqn = join_prefix(&obj_fqn, &nested_name);
                                if matches!(nested.kind, ast::TypeKind::Class)
                                    && !nested.type_params.is_empty()
                                {
                                    out.insert(nested_fqn, (source, file, nested));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// T0127: 从 monomorph keys 收集泛型独立函数的具体实例化，生成单态化的 HIR FunDecl。
///
/// 工作原理：
/// 1. 从 AST 中索引所有泛型顶层函数声明（有 type_params 的 `ast::Item::Fun`）。
/// 2. 遍历 monomorph keys，对每个 key 找到对应的函数声明。
/// 3. 调用 `lower_fun_with_type_bindings` 生成具体实例的 HIR FunDecl。
/// 4. 重命名 FQN 为 mangled 形式（例如 `pkg.id::<Int>`）。
pub(super) fn collect_generic_fun_instantiations(
    pairs: &[(&SourceFile, &ast::File)],
    monomorph_keys: &[crate::monomorph::MonomorphKey],
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    typecheck_types: &TypeStore,
) -> Vec<super::super::FunDecl> {
    if monomorph_keys.is_empty() {
        return Vec::new();
    }

    // 1) 索引泛型顶层函数：(fqn, decl_span) → (source, file, fun_decl)
    let mut generic_funs: HashMap<(String, crate::span::Span), (&SourceFile, &ast::File, &ast::FunDecl)> =
        HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Fun(fun) = item else {
                continue;
            };
            if fun.type_params.is_empty() {
                continue;
            }
            let local_name = source.slice(fun.name.span);
            let fqn = if pkg_prefix.is_empty() {
                local_name.to_string()
            } else {
                format!("{pkg_prefix}.{local_name}")
            };
            generic_funs.insert((fqn, fun.name.span), (source, file, fun));
        }
    }

    if generic_funs.is_empty() {
        return Vec::new();
    }

    // 2) 去重 + 生成实例
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<super::super::FunDecl> = Vec::new();

    for key in monomorph_keys {
        // T0130: monomorph key 中的 TypeId 来自 typecheck 阶段的 TypeStore，
        // 需要在当前 HIR lowering 的 TypeStore 中重新 intern。
        let re_interned_args: Vec<crate::ty::TypeId> = key
            .type_args
            .iter()
            .map(|&a| types.re_intern_from(typecheck_types, a))
            .collect();

        // 跳过仍含 Param 类型的 key（泛型传递调用）
        if re_interned_args.iter().any(|&a| matches!(types.kind(a), TypeKind::Param(_))) {
            continue;
        }

        let lookup_key = (key.symbol.fqn.clone(), key.symbol.decl_span);
        let Some((source, file, fun_decl)) = generic_funs.get(&lookup_key) else {
            continue;
        };

        if fun_decl.type_params.len() != re_interned_args.len() {
            continue;
        }

        // 构造 mangled FQN 用于去重
        let args_str = re_interned_args
            .iter()
            .map(|id| types.display(*id).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let instance_fqn = format!("{}::<{args_str}>", key.symbol.fqn);

        if !seen.insert(instance_fqn.clone()) {
            continue;
        }

        // 构建 type param → concrete TypeId 映射
        let bindings: Vec<(String, crate::ty::TypeId)> = fun_decl
            .type_params
            .iter()
            .zip(re_interned_args.iter())
            .map(|(p, &arg)| (p.name.text(source).to_string(), arg))
            .collect();

        let mut hir_fun = super::lower_fun_with_type_bindings(
            source,
            file,
            index,
            type_kinds,
            types,
            builtins,
            fun_decl,
            bindings,
        );

        hir_fun.fqn = instance_fqn;
        out.push(hir_fun);
    }

    out
}

fn eval_value_only_enum_discriminant(source: &SourceFile, expr: &ast::Expr) -> Option<i128> {
    match &expr.kind {
        ast::ExprKind::IntLit => {
            let raw = source.slice(expr.span);
            let text: String = raw.chars().filter(|c| *c != '_').collect();
            text.parse::<i128>().ok()
        }
        ast::ExprKind::Unary {
            op: ast::UnaryOp::Neg,
            expr: inner,
            ..
        } => {
            let v = eval_value_only_enum_discriminant(source, inner)?;
            Some(-v)
        }
        _ => None,
    }
}
