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
        type_args: vec![],
    };
    let b = InstanceKey {
        template_fqn: "f".to_string(),
        type_args: vec![],
    };
    assert_eq!(a, b);
    let c = InstanceKey {
        template_fqn: "g".to_string(),
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
