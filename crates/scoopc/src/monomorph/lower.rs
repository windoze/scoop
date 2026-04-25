//! 旧 `monomorph` dump 入口的薄包装。
//!
//! 当前真实的实例化实现已经迁到 `crate::mir::materialize`：
//! - `MonomorphKey` 继续保留“typecheck 收集到的实例请求”语义；
//! - `InstanceKey` / generic MIR template → monomorphic instance materialization
//!   则由 `mir::materialize_for_dump` 负责。

use crate::mir::{MaterializedMir, MirMaterializeError, materialize_for_dump};
use crate::session::Session;
use crate::source::SourceFile;

pub type LoweredMonomorphMir = MaterializedMir;
pub type MonomorphLowerError = MirMaterializeError;

pub fn lower_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<LoweredMonomorphMir, Box<MonomorphLowerError>> {
    materialize_for_dump(session, source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monomorph_collects_two_instances_for_id() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_id.scoop",
            r#"
package fixtures.monomorph

import scoop.core.*

fun id<T>(x: T): T {
    return x
}

fun f() {
    val a = id(1)
    val b = id("s")
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fqn_list: Vec<String> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                crate::mir::Item::Fun(fun) => Some(fun.fqn.clone()),
                _ => None,
            })
            .collect();

        assert!(fqn_list.iter().any(|fqn| fqn.contains("id::<Int>")));
        assert!(fqn_list.iter().any(|fqn| fqn.contains("id::<String>")));
        assert_eq!(
            fqn_list.iter().filter(|fqn| fqn.contains("id::<")).count(),
            2
        );
        assert_eq!(lowered.instance_keys.len(), 2);
    }

    #[test]
    fn monomorph_discovers_direct_call_fixed_point_in_mir_instances() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_fixed_point.scoop",
            r#"
package fixtures.monomorph

fun id<T>(x: T): T {
    return x
}

fun use<T>(x: T): T {
    return id(x)
}

fun entry(): Int {
    return use(1)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fqn_list: Vec<String> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                crate::mir::Item::Fun(fun) => Some(fun.fqn.clone()),
                _ => None,
            })
            .collect();
        assert!(
            fqn_list
                .iter()
                .any(|fqn| fqn == "fixtures.monomorph.use::<Int>")
        );
        assert!(
            fqn_list
                .iter()
                .any(|fqn| fqn == "fixtures.monomorph.id::<Int>")
        );

        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun) if fun.fqn == "fixtures.monomorph.use::<Int>" => {
                    Some(fun)
                }
                _ => None,
            })
            .expect("expected monomorphized use::<Int> instance");
        let body = fun.body.as_ref().expect("use::<Int> should have body");
        let call_kind = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::Call { kind, .. },
                    ..
                } => Some(kind),
                _ => None,
            })
            .expect("expected direct call in monomorphized body");
        match call_kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                assert_eq!(callee_fqn, "fixtures.monomorph.id::<Int>");
            }
            other => panic!("expected direct instantiated call, got {other:?}"),
        }
    }

    #[test]
    fn monomorph_rewrites_nested_closure_family_fn_ptrs() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_closure_family.scoop",
            r#"
package fixtures.monomorph

fun make<T>(x: T): () -> T {
    return { x }
}

fun entry(): Int {
    return make(1)()
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let root = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun) if fun.fqn == "fixtures.monomorph.make::<Int>" => {
                    Some(fun)
                }
                _ => None,
            })
            .expect("expected make::<Int> instance");
        let body = root.body.as_ref().expect("make::<Int> should have body");
        let closure_fn_ptr = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::MakeClosure { fn_ptr, .. },
                    ..
                } => Some(fn_ptr.as_str()),
                _ => None,
            })
            .expect("expected closure allocation in make::<Int>");
        assert_eq!(closure_fn_ptr, "fixtures.monomorph.make::<Int>.$lambda0");
        assert!(lowered.file.items.iter().any(|item| matches!(
            item,
            crate::mir::Item::Fun(fun)
                if fun.fqn == "fixtures.monomorph.make::<Int>.$lambda0"
        )));
    }

    #[test]
    fn monomorph_preserves_virtual_call_kind_in_instantiated_body() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_virtual_call.scoop",
            r#"
package fixtures.monomorph

open class Base() {
    open fun ping(): Int {
        return 1
    }
}

fun use<T>(marker: T, b: Base): Int {
    return b.ping()
}

fun entry(b: Base): Int {
    return use(1, b)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun) if fun.fqn.contains("use::<Int>") => Some(fun),
                _ => None,
            })
            .expect("expected monomorphized use::<Int> instance");
        let body = fun
            .body
            .as_ref()
            .expect("monomorphized instance should have body");
        let stmt = body.blocks[0]
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::Call { kind, .. },
                    ..
                } => Some(kind),
                _ => None,
            })
            .expect("expected call in monomorphized body");

        match stmt {
            crate::mir::CallKind::Virtual { dispatch, .. } => {
                assert_eq!(dispatch.owner_fqn, "fixtures.monomorph.Base");
                assert_eq!(dispatch.member_name, "ping");
            }
            other => panic!("expected virtual call kind, got {other:?}"),
        }
    }

    #[test]
    fn monomorph_preserves_perform_metadata_and_arg_order_in_instantiated_body() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_perform.scoop",
            r#"
package fixtures.monomorph

effect Pair {
    fun emit(a: Int, b: String): Int
}

fun use<T>(marker: T): Int / Pair {
    return Pair.emit(b = "x", a = 1)
}

fun entry(): Int / Pair {
    return use(0)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun) if fun.fqn.contains("use::<Int>") => Some(fun),
                _ => None,
            })
            .expect("expected monomorphized use::<Int> instance");
        let body = fun
            .body
            .as_ref()
            .expect("monomorphized instance should have body");
        let block = body
            .blocks
            .iter()
            .find(|block| {
                matches!(
                    block.terminator.kind,
                    crate::mir::TerminatorKind::Perform { .. }
                )
            })
            .expect("expected perform terminator in monomorphized body");

        let (op_fqn, metadata, args) = match &block.terminator.kind {
            crate::mir::TerminatorKind::Perform {
                op_fqn,
                metadata,
                args,
            } => (op_fqn, metadata, args),
            other => panic!("expected perform terminator, got {other:?}"),
        };
        assert_eq!(op_fqn, "fixtures.monomorph.Pair.emit");
        assert!(metadata.payload_tuple_ty.is_some());
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].source_arg_index, 1);
        assert_eq!(args[0].name.as_deref(), Some("a"));
        assert_eq!(args[1].source_arg_index, 0);
        assert_eq!(args[1].name.as_deref(), Some("b"));

        let (result_op_fqn, result_effect_ty) = block
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                crate::mir::StatementKind::Assign {
                    value:
                        crate::mir::Rvalue::PerformResult {
                            op_fqn, effect_ty, ..
                        },
                    ..
                } => Some((op_fqn.as_str(), *effect_ty)),
                _ => None,
            })
            .expect("expected perform result provenance in monomorphized body");
        assert_eq!(result_op_fqn, "fixtures.monomorph.Pair.emit");
        assert_eq!(result_effect_ty, metadata.effect_ty);
    }
}
