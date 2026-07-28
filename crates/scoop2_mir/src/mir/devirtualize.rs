//! 去虚化 pass：final 接收者的 Virtual 调用改写为 Direct。
//!
//! 在 materialize 的 rewrite 阶段之后运行。遍历 body 中每个 `CallKind::Virtual`，
//! 检查 `dispatch.receiver_ty` 对应的类型是否为 final。若为 final，将 Virtual 改写为
//! Direct `{ callee_fqn: owner_fqn.member_name, ... }`。
//!
//! final 判定原则：
//! - **所有值类型**（`TypeKind::Value(_)`）都是 final——Scoop 的值类型语义保证不可继承。
//!   这统一覆盖标量（Int/Bool/...）、Option、Tuple、struct nominal、enum nominal。
//!   不对 Option/Iterator/Iterable 等做任何特殊处理——它们只是普通的值类型 nominal。
//! - **Nothing**（bottom type）：final。
//! - **引用类型**（`TypeKind::Ref(_)`）：class/interface/object——保守不去虚化
//!   （需要 open/abstract/override 修饰符信息，当前 MIR 不携带）。

use scoop2_hir::ty::{TypeKind, TypeStore};

use crate::mir::{Body, CallKind, Module, Rvalue, StatementKind};

/// 判断一个类型是否为 final（不可有子类 → 虚方法可安全退化为直接调用）。
///
/// 判定基于类型系统的结构规则，不对任何具体类型（Option/Iterator/...）做特殊处理：
/// - 所有 `TypeKind::Value(_)` → true（值类型不可继承，天然 final）；
/// - `TypeKind::Nothing` → true（bottom type）；
/// - 其余（`TypeKind::Ref(_)`/`Param`/`StarProjection`）→ false。
fn is_final_type(store: &TypeStore, ty: scoop2_hir::ty::TypeId) -> bool {
    matches!(
        store.kind(ty),
        TypeKind::Value(_) | TypeKind::Nothing
    )
}

/// 对单个 body 执行去虚化。
pub fn devirtualize_body(store: &TypeStore, body: &mut Body) {
    for block in &mut body.blocks {
        for stmt in &mut block.stmts {
            if let StatementKind::Assign { value, .. } = &mut stmt.kind {
                devirtualize_rvalue(store, value);
            }
        }
    }
}

fn devirtualize_rvalue(store: &TypeStore, rv: &mut Rvalue) {
    match rv {
        Rvalue::Call { kind, .. } => {
            devirtualize_call_kind(store, kind);
        }
        _ => {}
    }
}

fn devirtualize_call_kind(store: &TypeStore, kind: &mut CallKind) {
    if let CallKind::Virtual { dispatch, .. } = kind {
        if is_final_type(store, dispatch.receiver_ty) {
            // 接收者类型是 final → 改写为 Direct。
            let callee_fqn = if dispatch.member_fqn.is_empty() {
                format!("{}.{}", dispatch.owner_fqn, dispatch.member_name)
            } else {
                dispatch.member_fqn.clone()
            };
            *kind = CallKind::Direct {
                callee_fqn,
                type_args: dispatch.generic_type_args.clone(),
                is_intrinsic: false,
                stable_template_key: dispatch.stable_template_key.clone(),
                stable_instance_key: dispatch.stable_template_key.as_ref().map(|stk| {
                    crate::mir::stable_id::make_stable_instance_key(
                        crate::mir::stable_id::StableHashScope::Dump,
                        stk.clone(),
                        store,
                        &dispatch.generic_type_args,
                        &dispatch.generic_eff_args,
                    )
                }),
                generic_type_args: dispatch.generic_type_args.clone(),
                generic_eff_args: dispatch.generic_eff_args.clone(),
            };
        }
    }
}

/// 对整个 Module 执行去虚化 pass。
pub fn devirtualize_module(module: &mut Module) {
    let store = &module.types;
    for item in &mut module.items {
        if let crate::mir::Item::Fun(fd) = item {
            if let Some(body) = &mut fd.body {
                devirtualize_body(store, body);
            }
        }
        if let crate::mir::Item::Initializer(ir) = item {
            devirtualize_body(store, &mut ir.body);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop2_hir::ty::TypeStore;

    #[test]
    fn value_types_are_final() {
        let mut store = TypeStore::new();
        let int = store.int();
        assert!(is_final_type(&store, int));
        let bool_ty = store.bool();
        assert!(is_final_type(&store, bool_ty));
        let unit = store.unit();
        assert!(is_final_type(&store, unit));
        let opt = store.option(int);
        assert!(is_final_type(&store, opt));
        let tup = store.tuple(vec![int, bool_ty]);
        assert!(is_final_type(&store, tup));
    }

    #[test]
    fn ref_types_are_not_final() {
        let mut store = TypeStore::new();
        let str_ty = store.string();
        assert!(!is_final_type(&store, str_ty));
        let any_ty = store.any();
        assert!(!is_final_type(&store, any_ty));
    }

    #[test]
    fn nothing_is_final() {
        let mut store = TypeStore::new();
        let nothing = store.nothing();
        assert!(is_final_type(&store, nothing));
    }
}
