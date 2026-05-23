//! 分支结果类型合并（T0514）。
//!
//! 目标：
//! - 为 `if/when` 等“多分支表达式”的结果类型提供稳定的合并规则；
//! - 在存在合理公共超类型时返回该类型（例如继承层级上的 LUB）；
//! - 当缺少合适公共超类型、且简单退化为 `Any` 过于粗糙时，构造受限 union：`A | B | ...`。

use std::collections::{HashMap, VecDeque};

use crate::ast;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, ValueTypeKind,
};

use super::assignable::is_type_assignable;
use super::lower::TypeLowering;

pub(super) fn merge_branch_result_type(
    a: TypeId,
    b: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> TypeId {
    // 快路径：相等 / bottom / top。
    if a == b {
        return a;
    }
    if a == builtins.nothing {
        return b;
    }
    if b == builtins.nothing {
        return a;
    }
    if a == builtins.any || b == builtins.any {
        return builtins.any;
    }

    // 子类型关系：直接 pick supertype。
    if is_subtype(a, b, lower, builtins) {
        return b;
    }
    if is_subtype(b, a, lower, builtins) {
        return a;
    }

    // 结构类型：Option / Tuple / Function 的“可比较情况”。
    let a_kind = lower.type_kind(a);
    let b_kind = lower.type_kind(b);

    if let Some(out) = merge_structural_lub(a, &a_kind, b, &b_kind, lower, builtins)
        && out != builtins.any
    {
        return out;
    }
    // `Any` 在这里通常意味着“结构上无法更精确合并”，继续进入 union fallback。

    // 名义类型：尝试找“最接近的公共超类型”（不包含隐式 `Any`）。
    if let Some(common) = merge_nominal_common_supertype(a, &a_kind, b, &b_kind, lower, builtins)
        && common != builtins.any
    {
        return common;
    }

    // 最后：受限 union（避免无脑退化到 `Any`）。
    lower.ty_union(vec![a, b])
}

fn merge_structural_lub(
    _a: TypeId,
    a_kind: &TypeKind,
    _b: TypeId,
    b_kind: &TypeKind,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<TypeId> {
    match (a_kind, b_kind) {
        // `Option<T>`：按 inner 做 LUB，再包回 `Option<...>`。
        (
            TypeKind::Value(ValueTypeKind::Option(a_inner)),
            TypeKind::Value(ValueTypeKind::Option(b_inner)),
        ) => {
            let inner = merge_branch_result_type(*a_inner, *b_inner, lower, builtins);
            Some(lower.ty_option(inner))
        }

        // tuple：逐元素合并（长度不一致则放弃）。
        (
            TypeKind::Value(ValueTypeKind::Tuple(a_elems)),
            TypeKind::Value(ValueTypeKind::Tuple(b_elems)),
        ) => {
            if a_elems.len() != b_elems.len() {
                return None;
            }
            let mut out: Vec<TypeId> = Vec::with_capacity(a_elems.len());
            for (x, y) in a_elems.iter().copied().zip(b_elems.iter().copied()) {
                out.push(merge_branch_result_type(x, y, lower, builtins));
            }
            Some(lower.ty_tuple(out))
        }

        // function：当前阶段只做“可比较情况”：
        // - receiver/params 必须全等（避免引入 GLB/交叉类型）
        // - return 取 LUB
        // - effects 取并集（因为 supertype 必须允许两者的 effects）
        (
            TypeKind::Ref(RefTypeKind::Function(a_fun)),
            TypeKind::Ref(RefTypeKind::Function(b_fun)),
        ) => {
            if a_fun.receiver != b_fun.receiver {
                return None;
            }
            if a_fun.params != b_fun.params {
                return None;
            }

            let ret = merge_branch_result_type(a_fun.return_ty, b_fun.return_ty, lower, builtins);
            let effects = EffectRow::new({
                let mut terms = a_fun.effects.terms.clone();
                terms.extend(b_fun.effects.terms.iter().copied());
                terms
            });
            // 只有当两个分支的函数类型都是闭合 row 时，合并后的 LUB 才能保持闭合语义。
            // 若任一分支为 open row，则保守退化为 open。
            let effects_closed = a_fun.effects_closed && b_fun.effects_closed;

            Some(lower.ty_function(
                a_fun.receiver,
                a_fun.params.clone(),
                ret,
                effects,
                effects_closed,
            ))
        }

        // 其它结构类型：暂不处理。
        _ => None,
    }
}

fn merge_nominal_common_supertype(
    _a: TypeId,
    a_kind: &TypeKind,
    _b: TypeId,
    b_kind: &TypeKind,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<TypeId> {
    let a_info = nominal_info(a_kind)?;
    let b_info = nominal_info(b_kind)?;

    // 若直接存在 assignable 关系，外层已提前返回；这里处理“兄弟类型”的共同祖先。
    let a_sup = supertypes_with_distance(&a_info.fqn, lower);
    let b_sup = supertypes_with_distance(&b_info.fqn, lower);

    let mut best: Option<(String, usize)> = None;
    for (fqn, da) in &a_sup {
        // 同名但不同实参/eff 的 nominal：当前阶段不做“丢实参”的合并，
        // 否则会把 `Foo<Int>` 与 `Foo<String>` 合并成不带实参的 `Foo`（语义不明确）。
        if a_info.fqn == b_info.fqn && !(a_info.is_plain && b_info.is_plain) && fqn == &a_info.fqn {
            continue;
        }

        let Some(db) = b_sup.get(fqn) else {
            continue;
        };

        let score = da.saturating_add(*db);
        match &mut best {
            None => best = Some((fqn.clone(), score)),
            Some((best_fqn, best_score)) => {
                if score < *best_score || (score == *best_score && fqn < best_fqn) {
                    *best_fqn = fqn.clone();
                    *best_score = score;
                }
            }
        }
    }

    let Some((best_fqn, _)) = best else {
        // 没有显式共同祖先；隐式共同祖先为 `Any`，由外层决定是否退化或构造 union。
        return Some(builtins.any);
    };

    Some(type_id_for_nominal_fqn_no_args(lower, &best_fqn, builtins))
}

struct NominalInfo {
    fqn: String,
    /// 当前阶段的“plain nominal”定义：无 type args、无 use-site eff 实参。
    ///
    /// 说明：用于避免把 `Foo<Int>` 与 `Foo<String>` 这类类型在 LUB 里错误“擦除”为 `Foo`。
    is_plain: bool,
}

fn nominal_info(kind: &TypeKind) -> Option<NominalInfo> {
    match kind {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => Some(NominalInfo {
            fqn: n.fqn.clone(),
            is_plain: n.args.is_empty() && n.eff.is_none(),
        }),
        TypeKind::Value(ValueTypeKind::Nominal(n)) => Some(NominalInfo {
            fqn: n.fqn.clone(),
            is_plain: n.args.is_empty() && n.eff.is_none(),
        }),
        _ => None,
    }
}

fn supertypes_with_distance(root_fqn: &str, lower: &TypeLowering<'_>) -> HashMap<String, usize> {
    let mut out: HashMap<String, usize> = HashMap::new();
    let mut q: VecDeque<(String, usize)> = VecDeque::new();
    out.insert(root_fqn.to_string(), 0);
    q.push_back((root_fqn.to_string(), 0));

    while let Some((cur, dist)) = q.pop_front() {
        let Some(supers) = lower.env().direct_supertypes(&cur) else {
            continue;
        };
        for st in supers {
            let next_dist = dist.saturating_add(1);
            if out.contains_key(st) {
                continue;
            }
            out.insert(st.clone(), next_dist);
            q.push_back((st.clone(), next_dist));
        }
    }

    out
}

fn type_id_for_nominal_fqn_no_args(
    lower: &mut TypeLowering<'_>,
    fqn: &str,
    builtins: BuiltinTypes,
) -> TypeId {
    // builtin/special-case：保持与 lowering 一致（避免重复创建 nominal）。
    match fqn {
        "scoop.core.Any" => return builtins.any,
        "scoop.core.String" => return builtins.string,
        "scoop.core.Unit" => return builtins.unit,
        "scoop.core.Nothing" => return builtins.nothing,
        "scoop.core.Bool" => return builtins.bool_,
        "scoop.core.Char" => return builtins.char_,
        "scoop.core.Float64" => return builtins.float64,
        "scoop.core.Float32" => return builtins.float32,
        "scoop.core.Int" => return builtins.int,
        "scoop.core.UInt" => return builtins.uint,
        "scoop.core.Option" => {
            // `Option<T>` 需要 type arg；作为共同祖先出现在这里通常意味着类型环境异常。
            return builtins.any;
        }
        _ => {}
    }

    let Some(decl_kind) = lower.nominal_decl_kind(fqn) else {
        // 防御性兜底：缺少类型符号时回退到 `Any`，避免 panic。
        return builtins.any;
    };

    let nominal = NominalType {
        fqn: fqn.to_string(),
        args: Vec::new(),
        eff: None,
    };

    let kind = match decl_kind {
        ast::TypeKind::Struct | ast::TypeKind::Enum => {
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
        }
        _ => TypeKind::Ref(RefTypeKind::Nominal(nominal)),
    };

    lower.intern_type_kind(kind)
}

fn is_subtype(sub: TypeId, sup: TypeId, lower: &TypeLowering<'_>, builtins: BuiltinTypes) -> bool {
    if sub == sup {
        return true;
    }

    if sub == builtins.nothing {
        return true;
    }

    if sup == builtins.any {
        return true;
    }

    // union：最小分配规则（用于分支合并与化简）。
    match (lower.type_kind(sub), lower.type_kind(sup)) {
        // `T <: (A | B)`：只要 `T` 可以赋值给任一分支即可。
        (_, TypeKind::Ref(RefTypeKind::Union(u))) => u
            .variants
            .iter()
            .copied()
            .any(|v| is_subtype(sub, v, lower, builtins)),
        // `(A | B) <: T`：要求所有分支都可赋值给 T。
        (TypeKind::Ref(RefTypeKind::Union(u)), _) => u
            .variants
            .iter()
            .copied()
            .all(|v| is_subtype(v, sup, lower, builtins)),

        // `Option<T>`：covariant（对 inner 做递归）。
        (TypeKind::Value(ValueTypeKind::Option(a)), TypeKind::Value(ValueTypeKind::Option(b))) => {
            is_subtype(a, b, lower, builtins)
        }

        // tuple：逐元素 covariant。
        (TypeKind::Value(ValueTypeKind::Tuple(a)), TypeKind::Value(ValueTypeKind::Tuple(b))) => {
            a.len() == b.len()
                && a.iter()
                    .copied()
                    .zip(b.iter().copied())
                    .all(|(x, y)| is_subtype(x, y, lower, builtins))
        }

        // 其它：复用现有的最小 assignable 规则（名义继承、函数子类型、装箱等）。
        _ => is_type_assignable(sub, sup, lower, builtins),
    }
}
