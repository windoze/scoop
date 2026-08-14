//! Stable keys：MIR 层 key 构造。
//!
//! canonical 类型文本编码与 scope 哈希核心已上移（PLAN.md M0-1）：
//! - 编码：`scoop2_hir::stable_id`（HIR / MIR 两层共用同一编码）；
//! - 哈希：`scoop2_base::stable`。
//!
//! 本模块保留 MIR 特有的 key 构造（依赖 `transport` 的 key 类型）：
//! - [`make_stable_template_key`] / [`make_stable_instance_key`]：实例身份
//!   （指向「哪个模板的哪组实参实例」）；与 HIR 层 `StableDefKey`（定义身份）
//!   同纪律、不同 key 值。
//! - [`build_overload_sig`]：MIR 侧重载消歧（仅参数类型 canonical；保持既有
//!   字节语义不变）。
//! - [`compute_public_stable_keys`]：为模块函数 / ctor / variant 填充 stable key。

pub use scoop2_base::{StableHashScope, stable_hash};
pub use scoop2_hir::stable_id::canonical_type_text;

use scoop2_base::Interner;
use scoop2_hir::stable_id::canonical_effect_row_text;
use scoop2_hir::ty::{EffectRow, TypeId, TypeStore};

use super::transport::{StableInstanceKey, StableTemplateKey};

// ---------------------------------------------------------------------------
// StableTemplateKey / StableInstanceKey construction
// ---------------------------------------------------------------------------

/// 构造 StableTemplateKey。
///
/// `fqn` 是函数的全限定名文本（稳定）。
/// `type_params` 是其类型参数名文本序列（稳定）。
/// `overload_sig` 是参数类型的 canonical 编码（用于区分同名重载）。
pub fn make_stable_template_key(
    scope: StableHashScope,
    fqn: &str,
    type_params: &[String],
    overload_sig: &str,
) -> StableTemplateKey {
    let tp_strs: Vec<String> = type_params.iter().map(|tp| format!("P({tp})")).collect();
    let canonical = if tp_strs.is_empty() && overload_sig.is_empty() {
        format!("template({fqn})")
    } else if tp_strs.is_empty() {
        format!("template({fqn};sig={overload_sig})")
    } else if overload_sig.is_empty() {
        format!("template({fqn};[{}])", tp_strs.join(","))
    } else {
        format!("template({fqn};[{}];sig={overload_sig})", tp_strs.join(","))
    };
    let hash = stable_hash(scope, &canonical);
    StableTemplateKey { canonical, hash }
}

/// 构造 StableInstanceKey。
pub fn make_stable_instance_key(
    scope: StableHashScope,
    template: StableTemplateKey,
    types: &TypeStore,
    interner: &Interner,
    type_args: &[TypeId],
    eff_args: &[EffectRow],
) -> StableInstanceKey {
    let canonical_type_args: Vec<String> = type_args
        .iter()
        .map(|&ty| canonical_type_text(types, interner, ty))
        .collect();
    let canonical_effect_args: Vec<String> = eff_args
        .iter()
        .map(|row| canonical_effect_row_text(types, interner, row))
        .collect();
    let instance_canonical = format!(
        "{}/T[{}]/E[{}]",
        template.canonical,
        canonical_type_args.join(","),
        canonical_effect_args.join(",")
    );
    let hash = stable_hash(scope, &instance_canonical);
    StableInstanceKey {
        template,
        canonical_type_args,
        canonical_effect_args,
        hash,
    }
}

/// 从参数类型列表构建 overload signature canonical 文本。
pub fn build_overload_sig(
    types: &TypeStore,
    interner: &Interner,
    param_types: &[TypeId],
) -> String {
    param_types
        .iter()
        .map(|&ty| canonical_type_text(types, interner, ty))
        .collect::<Vec<_>>()
        .join(",")
}

/// 为模块中所有顶层函数（含闭包 invoke 函数）计算 stable template key。
///
/// 遍历 `Module.items` 中的 `Item::Fun`，对每个有 fqn 的函数：
/// 1. 从 type_params 构造类型参数名列表；
/// 2. 从 params 构造 overload signature（canonical param types）；
/// 3. 调用 `make_stable_template_key` 生成 key；
/// 4. 填充 `FunDecl.stable_template_key`。
///
/// 这确保即使函数未被调用（public API），也有 stable key 供分离编译使用。
pub fn compute_public_stable_keys(module: &mut crate::mir::Module, interner: &Interner) {
    let store = &module.types;
    for item in &mut module.items {
        if let crate::mir::Item::Fun(fd) = item {
            if fd.stable_template_key.is_none() {
                // 构造类型参数名列表（从 type_params TypeParamId → 经 store 侧表查 name → 文本）。
                let tp_names: Vec<String> = fd
                    .type_params
                    .iter()
                    .map(|&id| interner.resolve(store.param_decl(id).name).to_string())
                    .collect();
                // 构造 overload signature（canonical param types）。
                let param_types: Vec<TypeId> = fd.params.iter().map(|p| p.ty).collect();
                let overload_sig = build_overload_sig(store, interner, &param_types);
                let stk = make_stable_template_key(
                    StableHashScope::Dump,
                    &fd.fqn,
                    &tp_names,
                    &overload_sig,
                );
                fd.stable_template_key = Some(stk);
            }
            // 为函数体中的 ClassCtor / EnumVariant 计算 stable key。
            if let Some(body) = &mut fd.body {
                compute_rvalue_stable_keys(body, store, interner);
            }
        }
        if let crate::mir::Item::Initializer(ir) = item {
            compute_rvalue_stable_keys(&mut ir.body, store, interner);
        }
    }
}

/// 为 body 中所有 ClassCtor / EnumVariant 的 stable key 字段填充值。
fn compute_rvalue_stable_keys(body: &mut crate::mir::Body, store: &TypeStore, interner: &Interner) {
    for block in &mut body.blocks {
        for stmt in &mut block.stmts {
            if let crate::mir::StatementKind::Assign { value, .. } = &mut stmt.kind {
                fill_rvalue_stable_key(value, store, interner);
            }
        }
    }
}

/// 为单个 Rvalue 的 ClassCtor / EnumVariant stable key 填充（若为 None）。
fn fill_rvalue_stable_key(rv: &mut crate::mir::Rvalue, store: &TypeStore, interner: &Interner) {
    match rv {
        crate::mir::Rvalue::ClassCtor {
            ctor,
            args,
            type_fqn,
            ..
        } => {
            if ctor.stable_template_key.is_none() {
                let fqn_text = interner.resolve(*type_fqn).to_string();
                let param_types: Vec<TypeId> = args.iter().map(|a| a.value_ty).collect();
                let overload_sig = build_overload_sig(store, interner, &param_types);
                ctor.stable_template_key = Some(make_stable_template_key(
                    StableHashScope::Dump,
                    &fqn_text,
                    &[],
                    &overload_sig,
                ));
            }
        }
        crate::mir::Rvalue::EnumVariant {
            enum_fqn,
            variant_name,
            args,
            stable_key,
            ..
        } => {
            if stable_key.is_none() {
                let enum_text = interner.resolve(*enum_fqn);
                let variant_text = interner.resolve(*variant_name);
                let fqn = format!("{}.{}", enum_text, variant_text);
                let param_types: Vec<TypeId> = args.iter().map(|a| a.value_ty).collect();
                let overload_sig = build_overload_sig(store, interner, &param_types);
                *stable_key = Some(make_stable_template_key(
                    StableHashScope::Dump,
                    &fqn,
                    &[],
                    &overload_sig,
                ));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_hash_is_deterministic() {
        let h1 = stable_hash(StableHashScope::Dump, "test");
        let h2 = stable_hash(StableHashScope::Dump, "test");
        assert_eq!(h1, h2);
    }

    #[test]
    fn stable_hash_scope_isolates() {
        let h_dump = stable_hash(StableHashScope::Dump, "test");
        let h_abi = stable_hash(StableHashScope::Abi, "test");
        assert_ne!(h_dump, h_abi);
    }

    #[test]
    fn template_key_includes_fqn() {
        let key = make_stable_template_key(StableHashScope::Dump, "pkg.main", &[], "");
        assert!(key.canonical.contains("pkg.main"));
        assert!(!key.hash.is_empty());
    }

    #[test]
    fn template_key_with_overload_sig() {
        let key_no_sig = make_stable_template_key(StableHashScope::Dump, "pkg.f", &[], "");
        let key_with_sig = make_stable_template_key(StableHashScope::Dump, "pkg.f", &[], "V(Int)");
        assert_ne!(key_no_sig.canonical, key_with_sig.canonical);
        assert_ne!(key_no_sig.hash, key_with_sig.hash);
    }

    #[test]
    fn instance_key_includes_type_args() {
        let mut store = TypeStore::new();
        let interner = Interner::new();
        let int = store.int();
        let template = make_stable_template_key(StableHashScope::Dump, "pkg.id", &[], "");
        let instance = make_stable_instance_key(
            StableHashScope::Dump,
            template,
            &store,
            &interner,
            &[int],
            &[],
        );
        assert_eq!(instance.canonical_type_args, vec!["V(Int)".to_string()]);
        assert!(!instance.hash.is_empty());
    }
}
