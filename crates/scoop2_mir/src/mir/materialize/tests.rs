//! 单态化单元测试。

#![cfg(test)]

use scoop2_hir::ty::TypeStore;

use crate::mir::materialize::{materialize, InstanceKey};
use crate::mir::{FunDecl, Module};

/// 构造一个最小可单态化 Module（单个非泛型 main 函数）。
fn minimal_module() -> Module {
    let mut store = TypeStore::new();
    let ret = store.unit();
    let fn_ty = store.function(scoop2_hir::ty::FunctionType {
        receiver: None,
        params: vec![],
        return_ty: ret,
        effects: scoop2_hir::ty::EffectRow::pure(),
        closed: false,
    });
    let main = FunDecl {
        span: scoop2_base::Span::default(),
        fqn: "main".to_string(),
        name: "main".to_string(),
        ty: fn_ty,
        params: Vec::new(),
        return_ty: ret,
        effect_row: scoop2_hir::ty::EffectRow::pure(),
        type_params: Vec::new(),
        body: None,
        file: scoop2_base::FileId(0),
        stable_template_key: None,
    };
    Module {
        items: vec![crate::mir::Item::Fun(main)],
        types: store,
    }
}

/// 从 main 出发的单态化：应产出含 main 的实例。
#[test]
fn materializes_main_entry() {
    let module = minimal_module();
    let result = materialize(module, Some("main"), &scoop2_hir::hir::TypedHir::empty(scoop2_base::Interner::new())).expect("单态化不应失败");
    assert!(
        result
            .instance_keys
            .iter()
            .any(|k| k.template_fqn == "main"),
        "main 应出现在实例化键中"
    );
}

/// InstanceKey 相等 / 哈希：相同 (fqn, type_args) 应相等。
#[test]
fn instance_key_equality() {
    let a = InstanceKey {
        template_fqn: "f".to_string(),
        overload_sig: String::new(),
        type_args: vec![],
    };
    let b = InstanceKey {
        template_fqn: "f".to_string(),
        overload_sig: String::new(),
        type_args: vec![],
    };
    assert_eq!(a, b);
    let c = InstanceKey {
        template_fqn: "g".to_string(),
        overload_sig: String::new(),
        type_args: vec![],
    };
    assert_ne!(a, c);
}

/// 单态化错误：entry 模板不存在时应报 `monomorph_no_template`。
#[test]
fn materialize_missing_template_errors() {
    let module = minimal_module();
    // entry = "nonexistent"：模板集合中无此 fqn → 单态化报 monomorph_no_template。
    let result = materialize(module, Some("nonexistent"), &scoop2_hir::hir::TypedHir::empty(scoop2_base::Interner::new()));
    assert!(
        result.is_err(),
        "缺模板的 entry 应触发单态化错误"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.code,
        crate::diagnostics::MONOMORPH_NO_TEMPLATE,
        "错误码应为 monomorph_no_template，实际 {}",
        err.code
    );
}

/// 无 entry 单态化：所有非泛型函数应作为种子。
#[test]
fn materializes_all_when_no_entry() {
    let module = minimal_module();
    let result = materialize(module, None, &scoop2_hir::hir::TypedHir::empty(scoop2_base::Interner::new())).expect("单态化不应失败");
    assert!(!result.instance_keys.is_empty(), "无 entry 时仍应单态化非泛型函数");
}

/// 单态化后 materialized MIR 不应含 TypeKind::Param 残留（verify_no_generic_residue 通过）。
#[test]
fn materialized_has_no_generic_residue() {
    let module = minimal_module();
    let result = materialize(module, Some("main"), &scoop2_hir::hir::TypedHir::empty(scoop2_base::Interner::new())).expect("单态化不应失败");
    let errors = crate::mir::verify::verify_materialized(&result.module);
    assert!(
        errors.iter().all(|e| !e.message.contains("TypeKind::Param")),
        "单态化后不应有泛型参数残留，实际发现: {:?}",
        errors.iter().filter(|e| e.message.contains("TypeKind::Param")).collect::<Vec<_>>()
    );
}

/// build_subst arity mismatch：type_args 少于 type_params 时应报错。
#[test]
fn build_subst_rejects_arity_mismatch() {
    // 构造一个泛型函数模板（type_params 有 1 个），但调用时不提供 type_args。
    // 通过单态化入口触发 build_subst 的 arity 检查。
    let mut store = TypeStore::new();
    let ret = store.unit();
    let param_ty = store.int();
    let tp_sym = scoop2_base::Symbol::from_u32(999);
    let fn_ty = store.function(scoop2_hir::ty::FunctionType {
        receiver: None,
        params: vec![param_ty],
        return_ty: ret,
        effects: scoop2_hir::ty::EffectRow::pure(),
        closed: false,
    });
    let generic_fn = FunDecl {
        span: scoop2_base::Span::default(),
        fqn: "generic_id".to_string(),
        name: "generic_id".to_string(),
        ty: fn_ty,
        params: vec![crate::mir::Param {
            span: scoop2_base::Span::default(),
            name: "x".to_string(),
            ty: param_ty,
            local: crate::mir::LocalId(0),
        }],
        return_ty: ret,
        effect_row: scoop2_hir::ty::EffectRow::pure(),
        type_params: vec![tp_sym],
        body: None,
        file: scoop2_base::FileId(0),
        stable_template_key: None,
    };
    // main 调用 generic_id 但不提供 type_args（type_args 为空，但模板有 1 个 type_param）。
    // 由于 scan_call_kind 依赖 stable_template_key 提取 type_args，
    // 这里通过直接调用 build_subst 验证 arity 检查。
    let templates = vec![generic_fn];
    let subst = super::build_subst(&templates, &[]);
    assert!(subst.is_err(), "type_args 少于 type_params 应报错");
}
