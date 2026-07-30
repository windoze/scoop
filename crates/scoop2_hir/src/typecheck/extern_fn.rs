//! `@Extern` 函数 / 变量的 ABI 契约校验（spec §15.x）。
//!
//! 复刻 legacy `scoopc_hir::typecheck::annotations` 中 `@Extern` 相关的**声明侧**
//! 校验：注解实参解析（`abi` / `callingConvention` / `name` / `lib`）、修饰符契约
//! （`@Unsafe`/`@NoGC` 与 ABI 的关系）、effect 契约、native C ABI 签名面、scoop
//! managed ABI v1 面。校验按固定顺序短路（首个错误即返回），与 legacy 一致。

use std::collections::HashMap;

use scoop2_base::Symbol;
use scoop2_base::diag::{Diagnostic, DiagnosticSink};

use crate::resolve::imports::ImportTable;
use crate::syntax::ast::{AnnotationUse, ExprKind, FunDecl, TypeRef};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeParamId, ValueTypeKind};

use super::TypeEnv;
use super::diagnostics;
use super::lower::TypeLowering;

/// `@Extern` 的 ABI。
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExternAbi {
    C,
    Scoop,
}

impl ExternAbi {
    fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "c" => Some(Self::C),
            "scoop" => Some(Self::Scoop),
            _ => None,
        }
    }
}

/// `@Extern` 函数所在位置（顶层 vs 成员）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExternSite {
    TopLevel,
    Member,
}

/// 已解析的 `@Extern` 实参（仅消费 `abi` / `callingConvention`；`name`/`lib` 只校验形态）。
struct ParsedExtern {
    abi: ExternAbi,
    calling_convention_span: Option<scoop2_base::Span>,
}

/// 校验一个 `@Extern` 函数声明（顶层或成员）。短路：首个错误即 push 并返回。
pub fn check_extern_fun(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    d: &FunDecl,
    site: ExternSite,
) {
    // 1. 解析 `@Extern` 注解实参。
    let Some(extern_ann) = find_extern_annotation(&d.annotations, env.interner) else {
        return;
    };
    let Some(parsed) = parse_extern_args(extern_ann, env.interner, diags) else {
        return;
    };

    // 2. 不允许单独叠加 `@CallingConvention`。
    if let Some(cc_ann) = find_annotation(&d.annotations, "CallingConvention", env.interner) {
        diags.push(
            diagnostics::extern_fun_calling_convention_annotation_not_allowed(
                annotation_name_span(cc_ann),
            ),
        );
        return;
    }

    // 3. 修饰符契约：`@Unsafe` / `@NoGC` 与 ABI 的关系。
    if let Some(e) = check_modifier_contract(&d.annotations, parsed.abi, env.interner) {
        diags.push(e);
        return;
    }

    // 4. effect 契约：不允许 effect row 参数，不允许非 Pure 的 outward effect row。
    if let Some(e) = check_effect_contract(d, env.interner) {
        diags.push(e);
        return;
    }

    // 5. 按 ABI 分支做签名面校验。
    match parsed.abi {
        ExternAbi::C => {
            if let Some(e) =
                check_native_abi_signature(env, imports, package_prefix, diags, d, site)
            {
                diags.push(e);
            }
        }
        ExternAbi::Scoop => {
            if let Some(span) = parsed.calling_convention_span {
                diags
                    .push(diagnostics::extern_fun_scoop_abi_calling_convention_not_supported(span));
                return;
            }
            if let Some(e) = check_scoop_abi_decl_shape(d, site) {
                diags.push(e);
                return;
            }
            if let Some(e) =
                check_scoop_abi_callable_surface(env, imports, package_prefix, diags, d)
            {
                diags.push(e);
            }
        }
    }
}

/// 校验 `@Extern var` 顶层变量声明：必须省略 initializer。
pub fn check_extern_var(diags: &mut DiagnosticSink, init_span: scoop2_base::Span) {
    diags.push(diagnostics::extern_var_initializer_not_allowed(init_span));
}

/// 校验独立 `@CallingConvention` 函数（与 native C ABI 同面）：不支持泛型、不允许非 Pure
/// effect row、签名必须在 native value surface 上。短路：首个错误即返回。
pub fn check_calling_convention(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    d: &FunDecl,
) {
    // 泛型。
    if let Some(first) = d.type_params.as_ref().and_then(|tp| tp.params.first()) {
        diags.push(diagnostics::calling_convention_fun_generics_not_supported(
            first.span,
        ));
        return;
    }
    // 非 Pure effect row。
    if let Some(eff) = &d.effect
        && !is_pure_effect_row(eff, env.interner)
    {
        diags.push(diagnostics::calling_convention_fun_effects_not_allowed(
            eff.span,
        ));
        return;
    }
    // native ABI 签名面。
    let tp_map = type_param_map(d);
    let mut ty_refs: Vec<&TypeRef> = Vec::new();
    for p in &d.params {
        if let Some(t) = &p.ty {
            ty_refs.push(t);
        }
    }
    if let Some(ret) = &d.return_ty {
        ty_refs.push(ret);
    }
    for ty_ref in ty_refs {
        let ty = {
            let mut lower = TypeLowering::new(
                env,
                imports,
                tp_map.clone(),
                package_prefix.to_string(),
                diags,
            );
            lower.lower(ty_ref)
        };
        if !is_native_abi_value_type(env, ty) {
            diags.push(
                diagnostics::calling_convention_fun_signature_not_supported_by_native_abi(
                    &fmt_type(env, ty),
                    ty_ref.span,
                ),
            );
            return;
        }
    }
}

// ===== 实参解析 =====

fn find_extern_annotation<'a>(
    anns: &'a [AnnotationUse],
    interner: &scoop2_base::Interner,
) -> Option<&'a AnnotationUse> {
    anns.iter()
        .find(|a| annotation_last_text(a, interner) == Some("Extern"))
}

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

fn string_lit_value(expr: &crate::syntax::ast::Expr) -> Option<(&str, scoop2_base::Span)> {
    match &expr.kind {
        ExprKind::StringLit(s) => Some((&s.value, s.span)),
        _ => None,
    }
}

fn parse_extern_args(
    ann: &AnnotationUse,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) -> Option<ParsedExtern> {
    let mut abi = ExternAbi::C;
    let mut calling_convention_span: Option<scoop2_base::Span> = None;
    let mut seen_named = false;
    let mut positional_seen = false;
    let mut name_seen = false;
    let mut lib_seen = false;
    let mut abi_seen = false;
    let mut cc_seen = false;

    for arg in &ann.args {
        match &arg.name {
            Some(name_id) => {
                seen_named = true;
                let key = interner.resolve(name_id.symbol);
                let (text, _vspan) = match string_lit_value(&arg.value) {
                    Some(t) => t,
                    None => {
                        diags.push(diagnostics::extern_annotation_args_invalid(arg.value.span));
                        return None;
                    }
                };
                match key {
                    "name" => {
                        if name_seen {
                            diags.push(diagnostics::extern_annotation_arg_duplicate(
                                "name",
                                name_id.span,
                            ));
                            return None;
                        }
                        name_seen = true;
                    }
                    "lib" => {
                        if lib_seen {
                            diags.push(diagnostics::extern_annotation_arg_duplicate(
                                "lib",
                                name_id.span,
                            ));
                            return None;
                        }
                        lib_seen = true;
                    }
                    "abi" => {
                        if abi_seen {
                            diags.push(diagnostics::extern_annotation_arg_duplicate(
                                "abi",
                                name_id.span,
                            ));
                            return None;
                        }
                        abi_seen = true;
                        match ExternAbi::parse(text) {
                            Some(parsed_abi) => abi = parsed_abi,
                            None => {
                                diags.push(diagnostics::extern_annotation_abi_not_supported(
                                    text,
                                    arg.value.span,
                                ));
                                return None;
                            }
                        }
                    }
                    "callingConvention" => {
                        if cc_seen {
                            diags.push(diagnostics::extern_annotation_arg_duplicate(
                                "callingConvention",
                                name_id.span,
                            ));
                            return None;
                        }
                        cc_seen = true;
                        if !is_valid_calling_convention(text) {
                            diags.push(diagnostics::calling_convention_not_supported(
                                text,
                                arg.value.span,
                            ));
                            return None;
                        }
                        calling_convention_span = Some(arg.value.span);
                    }
                    _ => {
                        diags.push(diagnostics::extern_annotation_args_invalid(name_id.span));
                        return None;
                    }
                }
            }
            None => {
                // 位置实参：只能有一个字符串字面量，且不能出现在命名实参之后。
                if seen_named || positional_seen {
                    diags.push(diagnostics::extern_annotation_args_invalid(arg.span));
                    return None;
                }
                if string_lit_value(&arg.value).is_none() {
                    diags.push(diagnostics::extern_annotation_args_invalid(arg.value.span));
                    return None;
                }
                positional_seen = true;
            }
        }
    }

    // 同时给出位置符号名与 `name=` → 形态不合法。
    if positional_seen && name_seen {
        diags.push(diagnostics::extern_annotation_args_invalid(ann.span));
        return None;
    }

    Some(ParsedExtern {
        abi,
        calling_convention_span,
    })
}

fn is_valid_calling_convention(name: &str) -> bool {
    matches!(name.trim().to_ascii_lowercase().as_str(), "c" | "cdecl")
}

// ===== 修饰符 / effect 契约 =====

fn check_modifier_contract(
    anns: &[AnnotationUse],
    abi: ExternAbi,
    interner: &scoop2_base::Interner,
) -> Option<Diagnostic> {
    for ann in anns {
        let Some(name) = annotation_last_text(ann, interner) else {
            continue;
        };
        let modifier = match name {
            "Unsafe" => "@Unsafe",
            "NoGC" => "@NoGC",
            _ => continue,
        };
        return Some(match abi {
            ExternAbi::C => diagnostics::extern_fun_c_abi_modifier_redundant(
                modifier,
                annotation_name_span(ann),
            ),
            ExternAbi::Scoop => diagnostics::extern_fun_scoop_abi_modifier_not_supported(
                modifier,
                annotation_name_span(ann),
            ),
        });
    }
    None
}

fn check_effect_contract(d: &FunDecl, interner: &scoop2_base::Interner) -> Option<Diagnostic> {
    // effect row 参数（`<eff E = Pure>`）。
    if let Some(tp) = &d.type_params
        && let Some(eff) = &tp.effect_row
    {
        return Some(diagnostics::extern_fun_eff_param_not_allowed(eff.span));
    }
    // 非 Pure 的 outward effect row（`/ Raise<...>`；`Pure` 视为空行允许）。
    if let Some(eff) = &d.effect
        && !is_pure_effect_row(eff, interner)
    {
        return Some(diagnostics::extern_fun_effects_not_allowed(eff.span));
    }
    None
}

fn is_pure_effect_row(
    eff: &crate::syntax::ast::EffectRowExpr,
    interner: &scoop2_base::Interner,
) -> bool {
    eff.terms.iter().all(|t| {
        t.path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == "Pure")
    })
}

// ===== native C ABI 签名面 =====

#[allow(clippy::too_many_arguments)]
fn check_native_abi_signature(
    env: &mut TypeEnv,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
    d: &FunDecl,
    _site: ExternSite,
) -> Option<Diagnostic> {
    let tp_map = type_param_map(d);
    // receiver 作为首个 native 参数，随后各形参，最后返回类型。
    let mut ty_refs: Vec<&TypeRef> = Vec::new();
    if let Some(recv) = &d.receiver {
        ty_refs.push(recv);
    }
    for p in &d.params {
        if let Some(ty) = &p.ty {
            ty_refs.push(ty);
        }
    }
    if let Some(ret) = &d.return_ty {
        ty_refs.push(ret);
    }
    for ty_ref in ty_refs {
        let ty = {
            let mut lower = TypeLowering::new(
                env,
                imports,
                tp_map.clone(),
                package_prefix.to_string(),
                diags,
            );
            lower.lower(ty_ref)
        };
        if !is_native_abi_value_type(env, ty) {
            return Some(
                diagnostics::extern_fun_signature_not_supported_by_native_abi(
                    &fmt_type(env, ty),
                    ty_ref.span,
                ),
            );
        }
    }
    None
}

/// native C ABI 接受的值类型面（复刻 legacy `is_native_abi_value_type`）：
/// 标量、`Unit`、tuple(递归)、`Ptr<T>`、`FunPtr<F>`、`@CLayout` struct。
/// 所有引用类型（含 `String`/`Any`/`Continuation`/`Pinned`/函数值）一律拒绝。
pub(crate) fn is_native_abi_value_type(env: &TypeEnv, id: TypeId) -> bool {
    match env.store.kind(id) {
        TypeKind::Ref(_) | TypeKind::Nothing | TypeKind::StarProjection => false,
        TypeKind::Param(_) => false,
        TypeKind::Value(v) => match v {
            ValueTypeKind::Unit
            | ValueTypeKind::Bool
            | ValueTypeKind::Char
            | ValueTypeKind::Float64
            | ValueTypeKind::Float32
            | ValueTypeKind::Int
            | ValueTypeKind::UInt
            | ValueTypeKind::IntN(_)
            | ValueTypeKind::UIntN(_) => true,
            ValueTypeKind::Tuple(els) => els.iter().all(|e| is_native_abi_value_type(env, *e)),
            ValueTypeKind::Nominal(n) => {
                // Option<T> 不属于 native ABI（与其它 nominal 的声明级判定不同）。
                if n.fqn == env.store.option_fqn() {
                    false
                } else {
                    is_native_abi_nominal(env, n.fqn)
                }
            }
        },
    }
}

fn is_native_abi_nominal(env: &TypeEnv, fqn: Symbol) -> bool {
    let name = env.interner.resolve(fqn);
    // `Ptr<T>` / `FunPtr<F>`（scoop.unsafe）是 native token。
    if name.ends_with(".Ptr") || name.ends_with(".FunPtr") {
        return true;
    }
    // 其余 nominal 值类型必须是 `@CLayout` struct。
    env.is_clayout_struct(fqn)
}

// ===== scoop managed ABI v1 面 =====

fn check_scoop_abi_decl_shape(d: &FunDecl, site: ExternSite) -> Option<Diagnostic> {
    if site != ExternSite::TopLevel || d.receiver.is_some() {
        let span = d.receiver.as_ref().map(|r| r.span).unwrap_or(d.name.span);
        return Some(diagnostics::extern_fun_scoop_abi_requires_top_level_fun(
            span,
        ));
    }
    if let Some(first) = d.type_params.as_ref().and_then(|tp| tp.params.first()) {
        return Some(diagnostics::extern_fun_scoop_abi_generics_not_supported(
            first.span,
        ));
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn check_scoop_abi_callable_surface(
    env: &mut TypeEnv,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
    d: &FunDecl,
) -> Option<Diagnostic> {
    let tp_map = type_param_map(d);
    let mut ty_refs: Vec<&TypeRef> = Vec::new();
    for p in &d.params {
        if let Some(ty) = &p.ty {
            ty_refs.push(ty);
        }
    }
    if let Some(ret) = &d.return_ty {
        ty_refs.push(ret);
    }
    for ty_ref in ty_refs {
        let ty = {
            let mut lower = TypeLowering::new(
                env,
                imports,
                tp_map.clone(),
                package_prefix.to_string(),
                diags,
            );
            lower.lower(ty_ref)
        };
        if !scoop_abi_v1_type_is_supported(env, ty) {
            return Some(
                diagnostics::extern_fun_scoop_abi_callable_surface_not_supported(
                    &fmt_type(env, ty),
                    ty_ref.span,
                ),
            );
        }
    }
    None
}

/// scoop managed ABI v1 接受面（复刻 legacy `scoop_abi_v1_type_is_supported`）：
/// 拒绝函数值引用、`Continuation` nominal、星投影，以及含上述的 tuple/Option。
fn scoop_abi_v1_type_is_supported(env: &TypeEnv, id: TypeId) -> bool {
    match env.store.kind(id) {
        TypeKind::Ref(RefTypeKind::Function(_)) | TypeKind::StarProjection => false,
        TypeKind::Ref(RefTypeKind::Nominal(n)) => !is_continuation_fqn(env.interner.resolve(n.fqn)),
        TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String | RefTypeKind::Union(_)) => true,
        TypeKind::Param(_) => true,
        TypeKind::Nothing => true,
        TypeKind::Value(ValueTypeKind::Nominal(n)) if n.fqn == env.store.option_fqn() => {
            // Option<T>：按 inner 递归判定。
            match n.args.first() {
                Some(inner) => scoop_abi_v1_type_is_supported(env, *inner),
                None => true,
            }
        }
        TypeKind::Value(ValueTypeKind::Tuple(els)) => {
            els.iter().all(|e| scoop_abi_v1_type_is_supported(env, *e))
        }
        TypeKind::Value(_) => true,
    }
}

fn is_continuation_fqn(name: &str) -> bool {
    name == "scoop.core.Continuation"
}

// ===== 工具 =====

fn type_param_map(d: &FunDecl) -> HashMap<Symbol, TypeParamId> {
    let mut map = HashMap::new();
    if let Some(tpl) = &d.type_params {
        for p in &tpl.params {
            map.insert(p.name.symbol, TypeParamId(p.id.as_u32()));
        }
    }
    map
}

/// 类型短名（用于诊断 `found` 字段）。委托给统一的 [`crate::ty::render_type`]。
fn fmt_type(env: &TypeEnv, id: TypeId) -> String {
    crate::ty::render_type(&env.store, env.interner, id, false)
}
