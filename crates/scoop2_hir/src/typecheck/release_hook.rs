//! `@ReleaseHook` 宿主 / 释放函数 / 实参字段契约校验（spec §15.x）。
//!
//! 复刻 legacy `scoopc_hir::typecheck::annotations` 中 `@ReleaseHook` 校验。校验按固定
//! 顺序**短路**（首个错误即返回）：host 形状 → 实验性 gating → 释放函数解析（存在性 /
//! leaf / Unit 返回）→ 实参与形参逐位对应（存在 / GC-free / 类型相等）。

use std::collections::HashSet;

use scoop2_base::diag::DiagnosticSink;

use crate::syntax::ast::{AnnotationUse, ExprKind, ModifierKind, TypeDecl, TypeKind};
use crate::ty::{TypeId, TypeKind as TyKind, ValueTypeKind};

use super::TypeEnv;
use super::diagnostics;

/// 校验一个带 `@ReleaseHook` 注解的 class 宿主（短路：首个错误即 push 并返回）。
pub fn check_release_hook(
    env: &mut TypeEnv,
    diags: &mut DiagnosticSink,
    d: &TypeDecl,
    package_prefix: &str,
) {
    let Some(rh_ann) = find_annotation(&d.annotations, "ReleaseHook", env.interner) else {
        return;
    };
    let type_fqn = {
        let name_text = env.interner.resolve(d.name.symbol);
        if package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{package_prefix}.{name_text}")
        }
    };

    // ---- host 形状（顺序短路）----
    // 1. 必须是普通 class（拒绝 struct/enum/interface/annotation class）。
    let found_kind = host_kind_name(d);
    if d.kind != TypeKind::Class
        || d.modifiers
            .iter()
            .any(|m| m.kind == ModifierKind::Annotation)
    {
        diags.push(diagnostics::release_hook_host_must_be_class(
            &type_fqn,
            found_kind,
            d.name.span,
        ));
        return;
    }
    // 2. 必须 non-generic。
    if let Some(tp) = &d.type_params
        && let Some(first) = tp.params.first()
    {
        let last = tp.params.last().unwrap_or(first);
        let span = scoop2_base::Span::new(first.span.start, last.span.end);
        diags.push(diagnostics::release_hook_host_must_be_non_generic(
            &type_fqn, span,
        ));
        return;
    }
    // 3. 必须 final（无 open/abstract/sealed）。
    for mk in [
        ModifierKind::Open,
        ModifierKind::Abstract,
        ModifierKind::Sealed,
    ] {
        if d.modifiers.iter().any(|m| m.kind == mk) {
            diags.push(diagnostics::release_hook_host_must_be_final(
                &type_fqn,
                modifier_name(mk),
                d.name.span,
            ));
            return;
        }
    }
    // 4. 必须 `@Experimental(feature = "releaseHook")`。
    if !has_release_hook_experimental(&d.annotations, env.interner) {
        diags.push(diagnostics::release_hook_host_requires_experimental(
            &type_fqn,
            annotation_name_span(rh_ann),
        ));
        return;
    }

    // ---- 解析 name / args ----
    let Some((name_fqn, arg_fields)) = parse_release_hook_annotation(rh_ann, env.interner) else {
        return;
    };

    // ---- 释放函数解析 ----
    let Some(func_sym) = env.interner.get(&name_fqn) else {
        diags.push(diagnostics::release_hook_function_not_found(
            &type_fqn,
            &name_fqn,
            rh_ann.span,
        ));
        return;
    };
    let Some(sig) = env.signatures(func_sym).and_then(|s| s.first()) else {
        diags.push(diagnostics::release_hook_function_not_found(
            &type_fqn,
            &name_fqn,
            rh_ann.span,
        ));
        return;
    };
    // leaf：@NoGC（且非 @Extern）或 C-ABI @Extern。
    let attrs = env.fun_attrs(func_sym).unwrap_or_default();
    let is_leaf = (attrs.is_nogc && !attrs.is_native_extern) || attrs.is_native_extern;
    if !is_leaf {
        diags.push(diagnostics::release_hook_function_must_be_nogc_or_c_extern(
            &name_fqn,
            func_name_span(env, func_sym).unwrap_or(rh_ann.span),
        ));
        return;
    }
    // 返回必须 Unit。
    if !env.store.is_unit(sig.return_ty) {
        diags.push(diagnostics::release_hook_function_return_must_be_unit(
            &name_fqn,
            &fmt_type(env, sig.return_ty),
            rh_ann.span,
        ));
        return;
    }

    // ---- 实参与形参对应 ----
    if arg_fields.len() != sig.params.len() {
        diags.push(diagnostics::release_hook_arg_count_mismatch(
            &type_fqn,
            &name_fqn,
            arg_fields.len(),
            sig.params.len(),
            rh_ann.span,
        ));
        return;
    }
    let host_sym = env.interner.get(&type_fqn).unwrap_or(d.name.symbol);
    for (idx, field_name) in arg_fields.iter().enumerate() {
        let field_sym = env.interner.get(field_name);
        let Some(field_ty) = field_sym.and_then(|fs| env.member_type(host_sym, fs)) else {
            diags.push(diagnostics::release_hook_arg_field_not_found(
                &type_fqn,
                field_name,
                rh_ann.span,
            ));
            return;
        };
        if !is_gc_free_value_type(env, field_ty) {
            diags.push(diagnostics::release_hook_arg_field_must_be_gc_free(
                &type_fqn,
                field_name,
                &fmt_type(env, field_ty),
                field_span(env, host_sym, field_sym).unwrap_or(rh_ann.span),
            ));
            return;
        }
        let param_ty = sig.params[idx];
        if field_ty != param_ty {
            diags.push(diagnostics::release_hook_arg_type_mismatch(
                &type_fqn,
                field_name,
                &fmt_type(env, field_ty),
                &fmt_type(env, param_ty),
                field_span(env, host_sym, field_sym).unwrap_or(rh_ann.span),
            ));
            return;
        }
    }
}

// ===== 解析辅助 =====

fn find_annotation<'a>(
    anns: &'a [AnnotationUse],
    name: &str,
    interner: &scoop2_base::Interner,
) -> Option<&'a AnnotationUse> {
    anns.iter()
        .find(|a| annotation_last_text(a, interner) == Some(name))
}

fn annotation_last_text<'i>(
    ann: &AnnotationUse,
    interner: &'i scoop2_base::Interner,
) -> Option<&'i str> {
    ann.path.segments.last().map(|s| interner.resolve(s.symbol))
}

fn annotation_name_span(ann: &AnnotationUse) -> scoop2_base::Span {
    ann.path
        .segments
        .last()
        .map(|s| s.span)
        .unwrap_or(ann.path.span)
}

/// 解析 `@ReleaseHook(name = "...", args = ["...", ...])`；失败返回 None（暂不报形态错误）。
fn parse_release_hook_annotation(
    ann: &AnnotationUse,
    interner: &scoop2_base::Interner,
) -> Option<(String, Vec<String>)> {
    let mut name: Option<String> = None;
    let mut args: Option<Vec<String>> = None;
    for arg in &ann.args {
        let key = arg.name.as_ref().map(|n| interner.resolve(n.symbol))?;
        match key {
            "name" => {
                if let ExprKind::StringLit(s) = &arg.value.kind {
                    name = Some(s.value.clone());
                }
            }
            "args" => {
                if let ExprKind::ArrayLit(els) = &arg.value.kind {
                    let mut v = Vec::new();
                    for e in els {
                        if let ExprKind::StringLit(s) = &e.kind {
                            v.push(s.value.clone());
                        }
                    }
                    args = Some(v);
                }
            }
            _ => {}
        }
    }
    Some((name?, args.unwrap_or_default()))
}

fn has_release_hook_experimental(anns: &[AnnotationUse], interner: &scoop2_base::Interner) -> bool {
    for ann in anns {
        if annotation_last_text(ann, interner) != Some("Experimental") {
            continue;
        }
        for arg in &ann.args {
            if arg
                .name
                .as_ref()
                .is_some_and(|n| interner.resolve(n.symbol) == "feature")
                && let ExprKind::StringLit(s) = &arg.value.kind
                && s.value == "releaseHook"
            {
                return true;
            }
        }
    }
    false
}

fn host_kind_name(d: &TypeDecl) -> &'static str {
    if d.modifiers
        .iter()
        .any(|m| m.kind == ModifierKind::Annotation)
    {
        "annotation class"
    } else {
        match d.kind {
            TypeKind::Class => "class",
            TypeKind::Interface => "interface",
            TypeKind::Struct => "struct",
            TypeKind::Enum => "enum",
            TypeKind::Effect => "effect",
        }
    }
}

fn modifier_name(mk: ModifierKind) -> &'static str {
    match mk {
        ModifierKind::Open => "open",
        ModifierKind::Abstract => "abstract",
        ModifierKind::Sealed => "sealed",
        ModifierKind::Public => "public",
        ModifierKind::Internal => "internal",
        ModifierKind::Private => "private",
        ModifierKind::Override => "override",
        ModifierKind::Operator => "operator",
        ModifierKind::Annotation => "annotation",
    }
}

// ===== GC-free 值类型（递归；复刻 legacy is_gc_free_value_type）=====

/// GC-free 值类型查询（release-hook / 顶层 global var 共用）。
pub(crate) fn is_gc_free_value_type(env: &TypeEnv, id: TypeId) -> bool {
    let mut visiting = HashSet::new();
    is_gc_free_value_type_inner(env, id, &mut visiting)
}

fn is_gc_free_value_type_inner(env: &TypeEnv, id: TypeId, visiting: &mut HashSet<TypeId>) -> bool {
    if !visiting.insert(id) {
        return false; // 递归环保守判否。
    }
    let ok = match env.store.kind(id) {
        TyKind::Ref(_) | TyKind::StarProjection | TyKind::Param(_) => false,
        TyKind::Nothing => true,
        TyKind::Value(v) => match v {
            ValueTypeKind::Unit
            | ValueTypeKind::Bool
            | ValueTypeKind::Char
            | ValueTypeKind::Float64
            | ValueTypeKind::Float32
            | ValueTypeKind::Int
            | ValueTypeKind::UInt
            | ValueTypeKind::IntN(_)
            | ValueTypeKind::UIntN(_) => true,
            ValueTypeKind::Option(inner) => is_gc_free_value_type_inner(env, *inner, visiting),
            ValueTypeKind::Tuple(els) => els
                .iter()
                .all(|e| is_gc_free_value_type_inner(env, *e, visiting)),
            ValueTypeKind::Nominal(n) => is_gc_free_nominal(env, n.fqn, visiting),
        },
    };
    visiting.remove(&id);
    ok
}

fn is_gc_free_nominal(
    env: &TypeEnv,
    fqn: scoop2_base::Symbol,
    visiting: &mut HashSet<TypeId>,
) -> bool {
    let name = env.interner.resolve(fqn);
    // `Ptr<T>` / `FunPtr<F>` 是 native token，视为 GC-free。
    if name.ends_with(".Ptr") || name.ends_with(".FunPtr") {
        return true;
    }
    // 引用 nominal（class/interface）非 GC-free；值 nominal（struct/enum）递归其字段。
    if env.is_reference_nominal(fqn) {
        return false;
    }
    match env.member_types(fqn) {
        Some(fields) => fields
            .values()
            .all(|ty| is_gc_free_value_type_inner(env, *ty, visiting)),
        None => false,
    }
}

// ===== 工具 =====

/// 类型短名。委托给统一的 [`crate::ty::render_type`]。
fn fmt_type(env: &TypeEnv, id: TypeId) -> String {
    crate::ty::render_type(&env.store, env.interner, id, false)
}

/// 释放函数名 span（用于 leaf/Unit 错误定位）。
fn func_name_span(env: &TypeEnv, fqn: scoop2_base::Symbol) -> Option<scoop2_base::Span> {
    env.index.lookup_funs(fqn).first().map(|s| s.span)
}

/// 宿主字段名 span（尽力；查不到时调用方回退）。
fn field_span(
    env: &TypeEnv,
    host_sym: scoop2_base::Symbol,
    field_sym: Option<scoop2_base::Symbol>,
) -> Option<scoop2_base::Span> {
    let _ = (env, host_sym, field_sym);
    None
}
