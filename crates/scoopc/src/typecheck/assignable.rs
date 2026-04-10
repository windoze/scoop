//! 可赋值/子类型（最小子集）。
//!
//! 说明：
//! - 该模块最初在 `expr` 中实现，用于 call args / return 等位置的最小类型关系判断；
//! - T0458（where 约束）需要在 type lowering 阶段复用“是否满足上界”的判定，因此抽取为共享模块。

use std::collections::HashSet;

use crate::ast;
use crate::ty::{BuiltinTypes, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::TypeEnv;
use super::lower::TypeLowering;

pub(super) fn nominal_is_subtype_by_fqn(
    found_fqn: &str,
    expected_fqn: &str,
    env: &TypeEnv,
) -> bool {
    if found_fqn == expected_fqn {
        return true;
    }

    // DFS（防循环）。
    let mut stack: Vec<&str> = vec![found_fqn];
    let mut seen: HashSet<&str> = HashSet::new();

    while let Some(cur) = stack.pop() {
        if !seen.insert(cur) {
            continue;
        }

        if cur == expected_fqn {
            return true;
        }

        let Some(supers) = env.direct_supertypes(cur) else {
            continue;
        };
        for st in supers {
            stack.push(st.as_str());
        }
    }

    false
}

/// 检查“found 是否可赋值给 expected”（最小子集）。
///
/// 当前阶段实现的最小规则（用于 `val` initializer / call args / `return` / where bound 等）：
/// - `Nothing <: T`（对任意 T，bottom type）
/// - `T <: Any`（对任意 T；ref 直接上转，value 通过 boxing 上转）
/// - nominal ref types：沿 direct supertypes 做最小上转（class 继承 / interface 实现与继承）
/// - nominal value types：当目标是 interface 时允许 boxing（同上）
///
/// 其余更完整的子类型系统（接口、类继承、值类型装箱等）
/// 会在后续任务中逐步补齐。
pub(super) fn is_type_assignable(
    found: TypeId,
    expected: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    if found == expected {
        return true;
    }

    // `Nothing`：不可达/空类型，可以视为任意类型的子类型。
    //
    // 说明：即使 `Nothing` 是值类型，也允许“赋值到引用类型”，因为这种赋值在运行时不会发生：
    // 表达式求值不会返回一个 `Nothing` 的值（它只能通过 `raise/return` 等控制流中止）。
    if found == builtins.nothing {
        return true;
    }

    // spec §2 / §2.5：
    // - `Any` 是所有引用类型的顶类型；
    // - 值类型可在需要时装箱（boxing）为 `Any`。
    if expected == builtins.any {
        return matches!(
            lower.type_kind(found),
            TypeKind::Ref(_) | TypeKind::Value(_) | TypeKind::Param(_)
        );
    }

    // T0437：声明处变型（declaration-site variance）的最小子类型规则（spec §3.2）。
    //
    // 规则（Kotlin-like）：
    // - invariant：要求 type args 全等
    // - out：covariant（found_arg <: expected_arg）
    // - in：contravariant（expected_arg <: found_arg）
    //
    // Scoop-specific restriction：
    // - 只有当该 type argument 是引用类型时，variance 才参与子类型（值类型布局不同，需显式转换）。
    let found_kind = lower.type_kind(found);
    let expected_kind = lower.type_kind(expected);

    // T0125：TypeKind::Param 作为期望类型时，接受任何值/引用类型（未约束的类型参数）。
    // 完整的泛型约束检查（where clauses）留给后续任务。
    if matches!(expected_kind, TypeKind::Param(_)) {
        return true;
    }
    // T0125：TypeKind::Param 作为 found 类型也应可赋值给任何目标类型（ erasure 语义）。
    if matches!(found_kind, TypeKind::Param(_)) {
        return true;
    }

    // T0435：函数类型的最小子类型关系。
    //
    // 规则（常见的函数子类型规则）：
    // - 参数逆变：expected.param <: found.param
    // - 返回协变：found.ret <: expected.ret
    // - effect row：found.effects ⊆ expected.effects（requires no more effects than）
    //
    // 注意：当前阶段类型系统仍不完整（名义继承/泛型/row 变量等），因此这里的判断只基于已有的
    // `is_type_assignable` 能力做递归。
    match (found_kind, expected_kind) {
        (
            TypeKind::Ref(RefTypeKind::Nominal(found_nominal)),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            if found_nominal.fqn == expected_nominal.fqn {
                // T0624：名义类型的 `eff` row 参数参与 subeffecting：
                // `Type<eff R1> <: Type<eff R2>` 当且仅当 `R1 ⊆ R2`（requires no more effects than）。
                match (found_nominal.eff.as_ref(), expected_nominal.eff.as_ref()) {
                    (None, None) => {}
                    (Some(found), Some(expected)) => {
                        if !found.is_subset_of(expected) {
                            return false;
                        }
                    }
                    _ => return false,
                }
                if found_nominal.args.len() != expected_nominal.args.len() {
                    return false;
                }

                let variances = lower.env().type_param_variances(&found_nominal.fqn);

                for (idx, (found_arg, expected_arg)) in found_nominal
                    .args
                    .iter()
                    .copied()
                    .zip(expected_nominal.args.iter().copied())
                    .enumerate()
                {
                    let declared = variances.and_then(|v| v.get(idx).copied()).unwrap_or(None);

                    // 默认：invariant（或者因为 value type 而禁用 variance）
                    let both_ref = lower.is_ref(found_arg) && lower.is_ref(expected_arg);

                    match declared {
                        None => {
                            if found_arg != expected_arg {
                                return false;
                            }
                        }
                        Some(ast::TypeParamVariance::Out) if both_ref => {
                            if !is_type_assignable(found_arg, expected_arg, lower, builtins) {
                                return false;
                            }
                        }
                        Some(ast::TypeParamVariance::In) if both_ref => {
                            if !is_type_assignable(expected_arg, found_arg, lower, builtins) {
                                return false;
                            }
                        }
                        Some(_) => {
                            // value types（或 unknown kind，例如 type param）占位：variance 不生效。
                            if found_arg != expected_arg {
                                return false;
                            }
                        }
                    }
                }

                return true;
            }

            // 当前阶段的最小继承/实现规则：只在目标类型“未带实参”时做上转，
            // 避免过早引入“泛型超类型实例化”的复杂语义。
            if !expected_nominal.args.is_empty() || expected_nominal.eff.is_some() {
                return false;
            }

            nominal_is_subtype_by_fqn(&found_nominal.fqn, &expected_nominal.fqn, lower.env())
        }
        // builtin 标量值类型（Int/Bool/...）→ interface：允许 boxing，并复用 sysroot 的继承/实现关系。
        //
        // 说明：
        // - builtin 类型在 type system 中不是 `Nominal`（例如 `ValueTypeKind::Int`），因此无法走
        //   “nominal value → nominal ref（interface）”的默认分支；
        // - 但在语义层面它们依然可以实现 interface（spec §2.2.2 / §2.3），并且 sysroot 会提供
        //   `struct Int : Hashable` 这类声明用于约束与工具链可见性；
        // - 这里把 builtin 映射回其 sysroot FQN，再按 direct supertypes 做最小上转判断。
        (
            TypeKind::Value(ValueTypeKind::Bool),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            expected_nominal.args.is_empty()
                && expected_nominal.eff.is_none()
                && nominal_is_subtype_by_fqn("scoop.core.Bool", &expected_nominal.fqn, lower.env())
        }
        (
            TypeKind::Value(ValueTypeKind::Char),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            expected_nominal.args.is_empty()
                && expected_nominal.eff.is_none()
                && nominal_is_subtype_by_fqn("scoop.core.Char", &expected_nominal.fqn, lower.env())
        }
        (
            TypeKind::Value(ValueTypeKind::Int),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            expected_nominal.args.is_empty()
                && expected_nominal.eff.is_none()
                && nominal_is_subtype_by_fqn("scoop.core.Int", &expected_nominal.fqn, lower.env())
        }
        (
            TypeKind::Value(ValueTypeKind::UInt),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            expected_nominal.args.is_empty()
                && expected_nominal.eff.is_none()
                && nominal_is_subtype_by_fqn("scoop.core.UInt", &expected_nominal.fqn, lower.env())
        }
        (
            TypeKind::Value(ValueTypeKind::IntN(bits)),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            if !expected_nominal.args.is_empty() || expected_nominal.eff.is_some() {
                return false;
            }
            let found_fqn = format!("scoop.core.Int{bits}");
            nominal_is_subtype_by_fqn(&found_fqn, &expected_nominal.fqn, lower.env())
        }
        (
            TypeKind::Value(ValueTypeKind::UIntN(bits)),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            if !expected_nominal.args.is_empty() || expected_nominal.eff.is_some() {
                return false;
            }
            let found_fqn = format!("scoop.core.UInt{bits}");
            nominal_is_subtype_by_fqn(&found_fqn, &expected_nominal.fqn, lower.env())
        }
        (
            TypeKind::Ref(RefTypeKind::String),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            expected_nominal.args.is_empty()
                && expected_nominal.eff.is_none()
                && nominal_is_subtype_by_fqn(
                    "scoop.core.String",
                    &expected_nominal.fqn,
                    lower.env(),
                )
        }
        (
            TypeKind::Value(ValueTypeKind::Nominal(found_nominal)),
            TypeKind::Value(ValueTypeKind::Nominal(expected_nominal)),
        ) => {
            if found_nominal.fqn != expected_nominal.fqn {
                return false;
            }
            // T0624：名义值类型的 `eff` row 参数同样参与 subeffecting（row 不影响布局）。
            match (found_nominal.eff.as_ref(), expected_nominal.eff.as_ref()) {
                (None, None) => {}
                (Some(found), Some(expected)) => {
                    if !found.is_subset_of(expected) {
                        return false;
                    }
                }
                _ => return false,
            }
            if found_nominal.args.len() != expected_nominal.args.len() {
                return false;
            }

            let variances = lower.env().type_param_variances(&found_nominal.fqn);

            for (idx, (found_arg, expected_arg)) in found_nominal
                .args
                .iter()
                .copied()
                .zip(expected_nominal.args.iter().copied())
                .enumerate()
            {
                let declared = variances.and_then(|v| v.get(idx).copied()).unwrap_or(None);

                // Kotlin-like restriction（spec §3.2）：
                // variance 只对引用类型实参生效（值类型布局不同，需显式转换）。
                let both_ref = lower.is_ref(found_arg) && lower.is_ref(expected_arg);

                match declared {
                    None => {
                        if found_arg != expected_arg {
                            return false;
                        }
                    }
                    Some(ast::TypeParamVariance::Out) if both_ref => {
                        if !is_type_assignable(found_arg, expected_arg, lower, builtins) {
                            return false;
                        }
                    }
                    Some(ast::TypeParamVariance::In) if both_ref => {
                        if !is_type_assignable(expected_arg, found_arg, lower, builtins) {
                            return false;
                        }
                    }
                    Some(_) => {
                        if found_arg != expected_arg {
                            return false;
                        }
                    }
                }
            }

            true
        }
        (
            TypeKind::Value(ValueTypeKind::Nominal(found_nominal)),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            // value → interface：允许 boxing；同样限制目标不带 type args。
            expected_nominal.args.is_empty()
                && nominal_is_subtype_by_fqn(&found_nominal.fqn, &expected_nominal.fqn, lower.env())
        }
        (
            TypeKind::Ref(RefTypeKind::Function(found_fun)),
            TypeKind::Ref(RefTypeKind::Function(expected_fun)),
        ) => {
            if !found_fun.effects.is_subset_of(&expected_fun.effects) {
                return false;
            }

            if !is_type_assignable(found_fun.return_ty, expected_fun.return_ty, lower, builtins) {
                return false;
            }

            // receiver function type：把 receiver 当作第一个参数参与逆变比较。
            let found_arity = found_fun.params.len() + found_fun.receiver.is_some() as usize;
            let expected_arity =
                expected_fun.params.len() + expected_fun.receiver.is_some() as usize;
            if found_arity != expected_arity {
                return false;
            }

            let mut found_params: Vec<TypeId> = Vec::with_capacity(found_arity);
            if let Some(r) = found_fun.receiver {
                found_params.push(r);
            }
            found_params.extend(found_fun.params.iter().copied());

            let mut expected_params: Vec<TypeId> = Vec::with_capacity(expected_arity);
            if let Some(r) = expected_fun.receiver {
                expected_params.push(r);
            }
            expected_params.extend(expected_fun.params.iter().copied());

            for (expected_param, found_param) in expected_params
                .iter()
                .copied()
                .zip(found_params.iter().copied())
            {
                if !is_type_assignable(expected_param, found_param, lower, builtins) {
                    return false;
                }
            }

            true
        }
        _ => false,
    }
}
