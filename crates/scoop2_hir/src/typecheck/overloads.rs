//! 重载冲突检测（复刻 legacy `scoopc_hir::typecheck::overloads.rs`）。
//!
//! 同 FQN 的顶层函数两两比较：有效参数类型相等（类型参数擦除为 `Any`）→ 冲突；
//! 否则若尾部默认参数使某位置实参数下不可区分 → 冲突。短路：按源序首个冲突即返回。
//!
//! 当前未覆盖：typealias 展开（typealias_equivalent）、effect-row reason（effect 未降级）。

use std::collections::HashMap;

use scoop2_base::Symbol;
use scoop2_base::diag::{Diagnostic, DiagnosticSink};

use crate::resolve::imports::ImportTable;
use crate::syntax::ast::FunDecl;
use crate::ty::{TypeId, TypeKind, TypeParamId};

use super::TypeEnv;
use super::diagnostics;
use super::lower::TypeLowering;

/// 检查一个 class 的次构造器（secondary ctor）重载冲突。
pub fn check_ctor_overload_conflicts(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    class_name: scoop2_base::Symbol,
    class_type_params: Option<&crate::syntax::ast::TypeParamList>,
    members: &[crate::syntax::ast::TypeMember],
) {
    use crate::syntax::ast::TypeMemberKind;
    // 收集 (member_span, &SecondaryCtorDecl)。
    let ctors: Vec<(scoop2_base::Span, &crate::syntax::ast::SecondaryCtorDecl)> = members
        .iter()
        .filter_map(|m| match &m.kind {
            TypeMemberKind::SecondaryCtor(c) => Some((m.span, c)),
            _ => None,
        })
        .collect();
    let fqn_text = fqn_of(env, package_prefix, class_name);
    // 次构造器之间的冲突（需要 >= 2 个次构造器）。
    if ctors.len() >= 2 {
        let infos: Vec<OverloadInfo> = ctors
            .iter()
            .map(|(span, c)| {
                build_ctor_info(
                    env,
                    imports,
                    diags,
                    package_prefix,
                    class_name,
                    *span,
                    class_type_params,
                    c,
                )
            })
            .collect();
        let mut hit = false;
        for i in 0..infos.len() {
            if hit {
                break;
            }
            for j in (i + 1)..infos.len() {
                if let Some(e) = detect_conflict(&infos[i], &infos[j], &fqn_text, true) {
                    diags.push(e);
                    hit = true;
                    break;
                }
            }
        }
    }
    // 主构造器 vs 次构造器：默认参数导致 0-arg 调用永远歧义。
    if !ctors.is_empty()
        && let Some(class_fqn) = env.interner.get(&fqn_text)
        && let Some(all_sigs) = env.ctor_signatures(class_fqn)
        && all_sigs.len() >= 2
    {
        let primary = &all_sigs[0];
        for secondary in &all_sigs[1..] {
            let primary_has_default = primary.has_defaults.iter().any(|d| *d);
            let secondary_has_default = secondary.has_defaults.iter().any(|d| *d);
            let primary_min = primary
                .has_defaults
                .iter()
                .position(|d| *d)
                .unwrap_or(primary.params.len());
            let secondary_min = secondary
                .has_defaults
                .iter()
                .position(|d| *d)
                .unwrap_or(secondary.params.len());
            // 两个 ctor 的 min_arity 都 <= 对方的 params.len() → 存在共匹配的 arity。
            let overlap =
                primary_min <= secondary.params.len() && secondary_min <= primary.params.len();
            // 但仅 arity 重叠不够：在共匹配的 arity 上，参数类型必须不可区分（任一可赋值给另一）
            // 才算真正歧义。如 `(Int, Int=0)` vs `(String)`：arity 1 时 Int vs String 不可互换。
            let type_indistinguishable = overlap && {
                let start = primary_min.max(secondary_min);
                let end = primary.params.len().min(secondary.params.len());
                (start..=end).any(|n| {
                    (0..n).all(|i| {
                        let pt = primary.params.get(i).copied();
                        let st = secondary.params.get(i).copied();
                        match (pt, st) {
                            (Some(a), Some(b)) => types_mutually_assignable(&env.store, a, b),
                            _ => false,
                        }
                    })
                })
            };
            if type_indistinguishable && (primary_has_default || secondary_has_default) {
                diags.push(
                    scoop2_base::diag::Diagnostic::error(
                        "scoop::typecheck::conflicting_overloads",
                        format!(
                            "默认参数导致构造器重载冲突：{fqn_text} 的构造器在部分调用点永远歧义"
                        ),
                    )
                    .with_primary(secondary.decl_span, "这里"),
                );
                return;
            }
        }
    }
}

/// 从次构造器构建重载视图。
#[allow(clippy::too_many_arguments)]
fn build_ctor_info(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    class_name: scoop2_base::Symbol,
    span: scoop2_base::Span,
    class_type_params: Option<&crate::syntax::ast::TypeParamList>,
    ctor: &crate::syntax::ast::SecondaryCtorDecl,
) -> OverloadInfo {
    // 合并类型自身 + 次构造器的类型参数（次构造器可引用类型的类型参数）。
    let mut tp_map = type_param_map_of(class_type_params);
    tp_map.extend(type_param_map_of(ctor.type_params.as_ref()));
    let mut tp_bounds =
        type_param_effective_bounds_of(env, imports, diags, package_prefix, class_type_params);
    tp_bounds.extend(type_param_effective_bounds_of(
        env,
        imports,
        diags,
        package_prefix,
        ctor.type_params.as_ref(),
    ));
    let mut eff: Vec<String> = Vec::new();
    let mut defaults: Vec<bool> = Vec::new();
    for p in &ctor.params {
        let ty_str = match &p.ty {
            Some(t) => {
                let ty = {
                    let mut lower = TypeLowering::new(
                        env,
                        imports,
                        tp_map.clone(),
                        package_prefix.to_string(),
                        diags,
                    );
                    lower.lower(t)
                };
                effective_type_str(env, ty, &tp_bounds)
            }
            None => "Any".to_string(),
        };
        eff.push(ty_str);
        defaults.push(p.default.is_some());
    }
    let name = env.interner.resolve(class_name);
    let candidate = format!("{name}({})", eff.join(", "));
    OverloadInfo {
        name_span: span,
        effective_params: eff,
        has_defaults: defaults,
        return_ty: None,
        effect_row: String::new(),
        is_vararg: ctor.params.last().is_some_and(|p| p.is_vararg),
        type_param_count: ctor
            .type_params
            .as_ref()
            .map(|tp| tp.params.len())
            .unwrap_or(0),
        candidate,
    }
}

/// 检查一组顶层函数的重载冲突（按 FQN 分组，组内 >1 才比较）。
pub fn check_top_level_overload_conflicts(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    funs: &[&FunDecl],
) {
    // 按 FQN 分组（忽略扩展函数：receiver.is_some）。
    let mut groups: HashMap<Symbol, Vec<&FunDecl>> = HashMap::new();
    for d in funs {
        if d.receiver.is_some() {
            continue;
        }
        let fqn_text = fqn_of(env, package_prefix, d.name.symbol);
        let Some(fqn) = env.interner.get(&fqn_text) else {
            continue;
        };
        groups.entry(fqn).or_default().push(*d);
    }
    for (fqn, decls) in groups {
        if decls.len() <= 1 {
            continue;
        }
        let mut decls = decls;
        decls.sort_by_key(|d| d.name.span.start);
        let mut hit = false;
        for i in 0..decls.len() {
            if hit {
                break;
            }
            for j in (i + 1)..decls.len() {
                if let Some(e) =
                    check_pair(env, imports, diags, package_prefix, fqn, decls[i], decls[j])
                {
                    diags.push(e);
                    hit = true;
                    break;
                }
            }
        }
    }
}

fn fqn_of(env: &TypeEnv, package_prefix: &str, name: Symbol) -> String {
    let name_text = env.interner.resolve(name);
    if package_prefix.is_empty() {
        name_text.to_string()
    } else {
        format!("{package_prefix}.{name_text}")
    }
}

/// 构建一个重载的有效签名视图。
struct OverloadInfo {
    name_span: scoop2_base::Span,
    effective_params: Vec<String>,
    has_defaults: Vec<bool>,
    return_ty: Option<TypeId>,
    /// effect 行规范键（去重排序后的 effect 短名 + 闭合标记）。纯（无项或 Pure）= ""。
    effect_row: String,
    /// 最后一个参数是否为 vararg。
    is_vararg: bool,
    /// 类型参数个数（>0 表示泛型）。
    type_param_count: usize,
    candidate: String,
}

fn build_info(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    d: &FunDecl,
) -> OverloadInfo {
    let tp_map = type_param_map(d);
    // 类型参数 → 有效约束串（无约束为 `Any`；Type 约束按其类型；ref/value 约束按标记）。
    let tp_bounds = type_param_effective_bounds(env, imports, diags, package_prefix, d);
    let mut eff: Vec<String> = Vec::new();
    let mut defaults: Vec<bool> = Vec::new();
    for p in &d.params {
        let ty_str = match &p.ty {
            Some(t) => {
                let ty = {
                    let mut lower = TypeLowering::new(
                        env,
                        imports,
                        tp_map.clone(),
                        package_prefix.to_string(),
                        diags,
                    );
                    lower.lower(t)
                };
                effective_type_str(env, ty, &tp_bounds)
            }
            None => "Any".to_string(),
        };
        eff.push(ty_str);
        defaults.push(p.default.is_some());
    }
    let return_ty = d.return_ty.as_ref().map(|t| {
        let mut lower = TypeLowering::new(
            env,
            imports,
            tp_map.clone(),
            package_prefix.to_string(),
            diags,
        );
        lower.lower(t)
    });
    let name = env.interner.resolve(d.name.symbol);
    let candidate = format!("{name}({})", eff.join(", "));
    OverloadInfo {
        name_span: d.name.span,
        effective_params: eff,
        has_defaults: defaults,
        return_ty,
        effect_row: effect_row_key(d, env.interner),
        is_vararg: d.params.last().is_some_and(|p| p.is_vararg),
        type_param_count: d
            .type_params
            .as_ref()
            .map(|tp| tp.params.len())
            .unwrap_or(0),
        candidate,
    }
}

/// effect 行的规范键：去重排序后的 effect 短名（`Pure`/无项 = 空），尾随 `!` 表示闭合行。
/// 仅用于判断两个重载的 effect 行是否相同——不参与重载决议，仅作为冲突理由细分。
fn effect_row_key(d: &FunDecl, interner: &scoop2_base::Interner) -> String {
    let Some(eff) = &d.effect else {
        return String::new();
    };
    let mut names: Vec<&str> = eff
        .terms
        .iter()
        .filter_map(|t| t.path.segments.last())
        .map(|seg| interner.resolve(seg.symbol))
        .filter(|n| *n != "Pure")
        .collect();
    names.sort_unstable();
    names.dedup();
    let mut s = names.join("+");
    if eff.closed.is_some() {
        s.push('!');
    }
    s
}

/// 类型参数名 → 有效约束串（复刻 legacy `collect_callable_type_param_effective_bounds`：
/// 无约束 → `Any`；`Type` 约束 → 该类型；`ref`/`value` → 固定标记）。
fn type_param_effective_bounds(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    d: &FunDecl,
) -> HashMap<Symbol, String> {
    type_param_effective_bounds_of(env, imports, diags, package_prefix, d.type_params.as_ref())
}

/// 同 [`type_param_effective_bounds`]，但直接接受类型参数列表（供构造器复用）。
fn type_param_effective_bounds_of(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    tpl: Option<&crate::syntax::ast::TypeParamList>,
) -> HashMap<Symbol, String> {
    use crate::syntax::ast::GenericBound;
    let mut map = HashMap::new();
    let Some(tpl) = tpl else {
        return map;
    };
    for p in &tpl.params {
        let s = match &p.bound {
            Some(GenericBound::Type(t)) => {
                let ty = {
                    let mut lower = TypeLowering::new(
                        env,
                        imports,
                        HashMap::new(),
                        package_prefix.to_string(),
                        diags,
                    );
                    lower.lower(t)
                };
                effective_type_str(env, ty, &HashMap::new())
            }
            Some(GenericBound::Ref(_)) => "ref".to_string(),
            Some(GenericBound::Value(_)) => "value".to_string(),
            None => "Any".to_string(),
        };
        map.insert(p.name.symbol, s);
    }
    map
}

/// 比较一对重载；返回首个冲突诊断（或 None）。
fn check_pair(
    env: &mut TypeEnv,
    imports: &ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    fqn: Symbol,
    a: &FunDecl,
    b: &FunDecl,
) -> Option<Diagnostic> {
    let ia = build_info(env, imports, diags, package_prefix, a);
    let ib = build_info(env, imports, diags, package_prefix, b);
    let fqn_text = env.interner.resolve(fqn).to_string();
    detect_conflict(&ia, &ib, &fqn_text, false)
}

/// 两个已构建的重载视图之间的冲突检测（顶层函数与构造器共用）。
fn detect_conflict(
    ia: &OverloadInfo,
    ib: &OverloadInfo,
    fqn_text: &str,
    is_ctor: bool,
) -> Option<Diagnostic> {
    // 0. vararg 与非 vararg 重载在某 arity 下不可区分（优先于通用等价判断）。
    if let Some(diag) = vararg_overlap(ia, ib, fqn_text) {
        return Some(diag);
    }
    // 0b. 两个泛型重载的类型参数 shape 不同（仅 differ-by-bound 受支持）。
    if ia.type_param_count > 0
        && ib.type_param_count > 0
        && ia.type_param_count != ib.type_param_count
        && is_equivalent(ia, ib)
    {
        return Some(diagnostics::generic_overload_shape_mismatch(
            ib.name_span,
            ia.name_span,
        ));
    }
    // 1. 有效签名等价（参数数量 + 逐位有效类型相等）。
    if is_equivalent(ia, ib) {
        let reason = if ia.effect_row != ib.effect_row {
            "仅 effect row 不同（effect row 不参与重载决议）"
        } else {
            match (ia.return_ty, ib.return_ty) {
                (Some(ra), Some(rb)) if ra != rb => "仅返回类型不同（返回类型不参与重载决议）",
                _ if is_ctor => "重复或不可区分的构造器签名",
                _ => "重复或不可区分的签名",
            }
        };
        return Some(diagnostics::conflicting_overloads_detail(
            fqn_text,
            reason,
            &ia.candidate,
            &ib.candidate,
            ib.name_span,
            ia.name_span,
        ));
    }
    // 2. 尾部默认参数导致某位置实参数下不可区分。
    if let Some(arity) = first_ambiguous_positional_arity(ia, ib) {
        let reason = format!("默认参数导致在提供 {arity} 个实参时不可区分（位置调用）");
        return Some(diagnostics::conflicting_overloads_detail(
            fqn_text,
            &reason,
            &ia.candidate,
            &ib.candidate,
            ib.name_span,
            ia.name_span,
        ));
    }
    None
}

/// vararg 重载与非 vararg 重载的重叠：若非 vararg 重载的固定 arity 落在 vararg 重载
/// 的可接受 arity 范围内，且参数类型兼容（前缀相等 + 剩余等于 vararg 元素类型），则冲突。
fn vararg_overlap(ia: &OverloadInfo, ib: &OverloadInfo, fqn_text: &str) -> Option<Diagnostic> {
    if ia.is_vararg == ib.is_vararg {
        return None;
    }
    let (var_info, fix_info) = if ia.is_vararg { (ia, ib) } else { (ib, ia) };
    let var_params = &var_info.effective_params;
    if var_params.is_empty() {
        return None;
    }
    let var_min = var_params.len() - 1;
    let element = var_params.last().unwrap();
    let fix_n = fix_info.effective_params.len();
    if fix_n < var_min {
        return None;
    }
    let prefix_ok = var_params[..var_min]
        .iter()
        .zip(fix_info.effective_params[..var_min].iter())
        .all(|(a, b)| a == b);
    if !prefix_ok {
        return None;
    }
    let rest_ok = fix_info.effective_params[var_min..]
        .iter()
        .all(|p| p == element);
    if !rest_ok {
        return None;
    }
    Some(diagnostics::vararg_overlaps_non_vararg(
        fqn_text,
        &var_info.candidate,
        &fix_info.candidate,
        var_info.name_span,
        fix_info.name_span,
    ))
}

fn is_equivalent(a: &OverloadInfo, b: &OverloadInfo) -> bool {
    if a.effective_params.len() != b.effective_params.len() {
        return false;
    }
    a.effective_params
        .iter()
        .zip(b.effective_params.iter())
        .all(|(pa, pb)| pa == pb)
}

/// 找到一个位置实参数 k，使两边在该 arity 下前缀有效类型相等且不可区分（默认参数使然）。
fn first_ambiguous_positional_arity(a: &OverloadInfo, b: &OverloadInfo) -> Option<usize> {
    let min_a = min_positional_arity(a);
    let min_b = min_positional_arity(b);
    let max_k = a.effective_params.len().min(b.effective_params.len());
    for k in 0..=max_k {
        if k < min_a || k < min_b {
            continue;
        }
        if prefixes_equal(&a.effective_params, &b.effective_params, k) {
            // 完全相等由 is_equivalent 覆盖；此处只报更短的歧义 arity。
            if k == a.effective_params.len() && k == b.effective_params.len() {
                continue;
            }
            return Some(k);
        }
    }
    None
}

/// 仅尾部默认参数可省略：从末尾连续数 has_default。
fn min_positional_arity(info: &OverloadInfo) -> usize {
    let mut trailing = 0usize;
    for d in info.has_defaults.iter().rev() {
        if *d {
            trailing += 1;
        } else {
            break;
        }
    }
    info.effective_params.len().saturating_sub(trailing)
}

fn prefixes_equal(a: &[String], b: &[String], k: usize) -> bool {
    if a.len() < k || b.len() < k {
        return false;
    }
    a[..k].iter().zip(b[..k].iter()).all(|(x, y)| x == y)
}

/// 有效类型字符串：类型参数擦除为**其有效约束**（无约束为 `Any`）；其余按类别短名。
fn effective_type_str(env: &TypeEnv, id: TypeId, tp_bounds: &HashMap<Symbol, String>) -> String {
    match env.store.kind(id) {
        TypeKind::Param(p) => tp_bounds
            .get(&env.store.param_decl(*p).name)
            .cloned()
            .unwrap_or_else(|| "Any".to_string()),
        TypeKind::Ref(crate::ty::RefTypeKind::Any) => "Any".to_string(),
        TypeKind::Ref(crate::ty::RefTypeKind::String) => "String".to_string(),
        TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
        | TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => env
            .interner
            .resolve(n.fqn)
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_string(),
        TypeKind::Ref(crate::ty::RefTypeKind::Function(f)) => {
            let receiver = f
                .receiver
                .map(|r| format!("{}.", effective_type_str(env, r, tp_bounds)))
                .unwrap_or_default();
            let params: Vec<String> = f
                .params
                .iter()
                .map(|p| effective_type_str(env, *p, tp_bounds))
                .collect();
            format!(
                "{}({}) -> {}",
                receiver,
                params.join(", "),
                effective_type_str(env, f.return_ty, tp_bounds)
            )
        }
        TypeKind::Ref(crate::ty::RefTypeKind::Union(_)) => "union".to_string(),
        TypeKind::Nothing => "Nothing".to_string(),
        TypeKind::StarProjection => "*".to_string(),
        TypeKind::Value(crate::ty::ValueTypeKind::Unit) => "Unit".to_string(),
        TypeKind::Value(crate::ty::ValueTypeKind::Bool) => "Bool".to_string(),
        TypeKind::Value(crate::ty::ValueTypeKind::Char) => "Char".to_string(),
        TypeKind::Value(crate::ty::ValueTypeKind::Float64) => "Float64".to_string(),
        TypeKind::Value(crate::ty::ValueTypeKind::Float32) => "Float32".to_string(),
        TypeKind::Value(crate::ty::ValueTypeKind::Int) => "Int".to_string(),
        TypeKind::Value(crate::ty::ValueTypeKind::UInt) => "UInt".to_string(),
        TypeKind::Value(crate::ty::ValueTypeKind::IntN(n)) => format!("Int{n}"),
        TypeKind::Value(crate::ty::ValueTypeKind::UIntN(n)) => format!("UInt{n}"),
        TypeKind::Value(crate::ty::ValueTypeKind::Option(inner)) => {
            format!("{}?", effective_type_str(env, *inner, tp_bounds))
        }
        TypeKind::Value(crate::ty::ValueTypeKind::Tuple(els)) => {
            let inner: Vec<String> = els
                .iter()
                .map(|e| effective_type_str(env, *e, tp_bounds))
                .collect();
            format!("({})", inner.join(", "))
        }
    }
}

fn type_param_map(d: &FunDecl) -> HashMap<Symbol, TypeParamId> {
    type_param_map_of(d.type_params.as_ref())
}

/// 两个类型在重载不可区分意义上是否相容（有效类型字符串相等）。
/// 用于主/次构造器默认参数冲突判定：仅在共匹配 arity 上参数类型不可区分时才报冲突。
fn types_mutually_assignable(store: &crate::ty::TypeStore, a: TypeId, b: TypeId) -> bool {
    // 简化：按 TypeId 直接相等（hash-consing 保证同结构类型同 id）。
    // 对 nominal 比较 FQN（忽略类型实参，与 effective_type_str 一致）。
    let ka = store.kind(a);
    let kb = store.kind(b);
    match (ka, kb) {
        (TypeKind::Param(_), TypeKind::Param(_)) => true,
        (TypeKind::Param(_), _) | (_, TypeKind::Param(_)) => true,
        (
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(na)),
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nb)),
        )
        | (
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(na)),
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nb)),
        ) => na.fqn == nb.fqn,
        _ => a == b,
    }
}

/// 同 [`type_param_map`]，但直接接受类型参数列表（供构造器复用）。
fn type_param_map_of(
    tpl: Option<&crate::syntax::ast::TypeParamList>,
) -> HashMap<Symbol, TypeParamId> {
    let mut map = HashMap::new();
    if let Some(tpl) = tpl {
        for p in &tpl.params {
            map.insert(p.name.symbol, TypeParamId(p.id.as_u32()));
        }
    }
    map
}
