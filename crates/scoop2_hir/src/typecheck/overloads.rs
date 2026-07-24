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
use crate::ty::{TypeId, TypeKind, TypeParamType};

use super::TypeEnv;
use super::diagnostics;
use super::lower::TypeLowering;

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
    use crate::syntax::ast::GenericBound;
    let mut map = HashMap::new();
    let Some(tpl) = &d.type_params else {
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

    // 1. 有效签名等价（参数数量 + 逐位有效类型相等）。
    if is_equivalent(&ia, &ib) {
        let reason = if ia.effect_row != ib.effect_row {
            "仅 effect row 不同（effect row 不参与重载决议）"
        } else {
            match (ia.return_ty, ib.return_ty) {
                (Some(ra), Some(rb)) if ra != rb => "仅返回类型不同（返回类型不参与重载决议）",
                _ => "重复或不可区分的签名",
            }
        };
        return Some(diagnostics::conflicting_overloads_detail(
            &fqn_text,
            reason,
            &ia.candidate,
            &ib.candidate,
            ib.name_span,
            ia.name_span,
        ));
    }
    // 2. 尾部默认参数导致某位置实参数下不可区分。
    if let Some(arity) = first_ambiguous_positional_arity(&ia, &ib) {
        let reason = format!("默认参数导致在提供 {arity} 个实参时不可区分（位置调用）");
        return Some(diagnostics::conflicting_overloads_detail(
            &fqn_text,
            &reason,
            &ia.candidate,
            &ib.candidate,
            ib.name_span,
            ia.name_span,
        ));
    }
    None
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
            .get(&p.name)
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
        TypeKind::Ref(crate::ty::RefTypeKind::Function(_)) => "function".to_string(),
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

fn type_param_map(d: &FunDecl) -> HashMap<Symbol, TypeParamType> {
    let mut map = HashMap::new();
    if let Some(tpl) = &d.type_params {
        for p in &tpl.params {
            map.insert(
                p.name.symbol,
                TypeParamType {
                    name: p.name.symbol,
                    file: scoop2_base::FileId(0),
                    span: p.name.span,
                },
            );
        }
    }
    map
}
