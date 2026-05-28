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
    fn monomorph_preserves_exact_virtual_call_for_mir_pass_owner() {
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
            other => panic!("expected virtual call for MIR devirtualization pass, got {other:?}"),
        }
    }

    #[test]
    fn monomorph_keeps_virtual_call_kind_when_receiver_has_known_subclass() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_virtual_call_non_exact.scoop",
            r#"
package fixtures.monomorph

open class Base() {
    open fun ping(): Int {
        return 1
    }
}

class Derived() : Base() {
    override fun ping(): Int {
        return 2
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
    fn monomorph_preserves_exact_interface_bound_call_for_mir_pass_owner() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_interface_call_exact.scoop",
            r#"
package fixtures.monomorph

interface Ping {
    fun ping(): Int
}

class Box() : Ping {
    override fun ping(): Int {
        return 1
    }
}

fun <T> use(x: T): Int where T: Ping {
    return x.ping()
}

fun entry(): Int {
    return use(Box())
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun) if fun.fqn.contains("use::<fixtures.monomorph.Box>") => {
                    Some(fun)
                }
                _ => None,
            })
            .expect("expected monomorphized use::<Box> instance");
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
            crate::mir::CallKind::Interface { dispatch, .. } => {
                assert_eq!(dispatch.owner_fqn, "fixtures.monomorph.Ping");
                assert_eq!(dispatch.member_name, "ping");
            }
            other => panic!("expected interface call for MIR devirtualization pass, got {other:?}"),
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
                ..
            } => (op_fqn, metadata, args),
            other => panic!("expected perform terminator, got {other:?}"),
        };
        assert_eq!(op_fqn, "fixtures.monomorph.Pair.emit");
        assert!(metadata.payload_tuple_ty.is_some());
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].source_arg_index, 1);
        assert_eq!(args[0].name.as_deref(), None);
        assert_eq!(args[1].source_arg_index, 0);
        assert_eq!(args[1].name.as_deref(), None);

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

    #[test]
    fn monomorph_materializes_compilable_sysroot_generic_template() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_sysroot_print.scoop",
            r#"
package fixtures.monomorph

import scoop.core.*

fun entry(): Unit {
    print(1)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let key = lowered
            .instance_keys
            .iter()
            .find(|key| key.template.fqn == "scoop.core.print")
            .expect("expected print::<Int> instance request");
        assert_eq!(
            key.template
                .source_path
                .file_name()
                .and_then(|name| name.to_str()),
            Some("print.scoop")
        );
        assert!(lowered.file.items.iter().any(|item| matches!(
            item,
            crate::mir::Item::Fun(fun) if fun.fqn == "scoop.core.print::<Int>"
        )));
    }

    #[test]
    fn monomorph_materializes_declaration_only_sysroot_generic_template() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_sysroot_unsafe.scoop",
            r#"
package fixtures.monomorph

import scoop.core.*
import scoop.unsafe.*

fun entry(): Int {
    val ptr: Ptr<Int> = @Unsafe do { stackAlloc<Int>() }
    return 0
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let key = lowered
            .instance_keys
            .iter()
            .find(|key| key.template.fqn == "scoop.unsafe.stackAlloc")
            .expect("expected stackAlloc::<Int> instance request");
        assert_eq!(
            key.template
                .source_path
                .file_name()
                .and_then(|name| name.to_str()),
            Some("unsafe.scoop")
        );

        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun) if fun.fqn == "scoop.unsafe.stackAlloc::<Int>" => {
                    Some(fun)
                }
                _ => None,
            })
            .expect("expected declaration-only sysroot instance");
        assert!(
            fun.body.is_none(),
            "declaration-only generic fun should materialize as bodyless MIR instance"
        );
    }

    #[test]
    fn monomorph_rewrites_external_generic_calls_to_concrete_instances() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_external_generic_fixed_point.scoop",
            r#"
package fixtures.monomorph

import scoop.core.*

fun wrap<T: ToString>(value: T): Unit {
    print(value)
}

fun entry(): Unit {
    wrap(1)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let wrap = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun) if fun.fqn == "fixtures.monomorph.wrap::<Int>" => {
                    Some(fun)
                }
                _ => None,
            })
            .expect("expected wrap::<Int> instance");
        let body = wrap.body.as_ref().expect("wrap::<Int> should have body");
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
            .expect("expected direct call in wrap::<Int>");
        match call_kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                assert_eq!(callee_fqn, "scoop.core.print::<Int>");
            }
            other => panic!("expected direct instantiated print call, got {other:?}"),
        }

        assert_eq!(
            lowered
                .file
                .items
                .iter()
                .filter(|item| matches!(
                    item,
                    crate::mir::Item::Fun(fun) if fun.fqn == "scoop.core.print::<Int>"
                ))
                .count(),
            1
        );
        assert!(
            !lowered.file.items.iter().any(|item| matches!(
                item,
                crate::mir::Item::Fun(fun) if fun.fqn == "scoop.core.print::<T>"
            )),
            "materializer should not emit template-param-only print instances"
        );
    }

    #[test]
    fn monomorph_materializes_effect_only_generic_instance() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_effect_only_generic.scoop",
            r#"
package fixtures.monomorph

effect Boom {
    fun ping(): Unit
}

fun <eff E = Pure> forward(x: Int): Int / E {
    return x
}

fun entry(): Int / Boom {
    return forward<eff Boom>(1)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        assert_eq!(lowered.instance_keys.len(), 1);
        let key = lowered
            .instance_keys
            .iter()
            .find(|key| key.template.fqn == "fixtures.monomorph.forward")
            .expect("expected forward effect-only instance");
        assert!(key.type_args.is_empty());
        assert_eq!(key.eff_args.len(), 1);
        assert!(lowered.file.items.iter().any(|item| matches!(
            item,
            crate::mir::Item::Fun(fun)
                if fun.fqn == "fixtures.monomorph.forward::<eff fixtures.monomorph.Boom>"
        )));
    }

    #[test]
    fn monomorph_distinguishes_same_type_args_with_different_effect_rows() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_same_type_diff_effect.scoop",
            r#"
package fixtures.monomorph

effect Boom {
    fun ping(): Unit
}

effect Zap {
    fun ping(): Unit
}

fun <T, eff E = Pure> wrap(x: T): T / E {
    return x
}

fun entry(): Unit / (Boom + Zap) {
    val a = wrap<Int, eff Boom>(1)
    val b = wrap<Int, eff Zap>(2)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let wrap_keys = lowered
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.monomorph.wrap")
            .collect::<Vec<_>>();
        assert_eq!(wrap_keys.len(), 2);
        assert!(wrap_keys.iter().all(|key| key.type_args.len() == 1));
        assert!(wrap_keys.iter().all(|key| key.eff_args.len() == 1));
        assert!(lowered.file.items.iter().any(|item| matches!(
            item,
            crate::mir::Item::Fun(fun)
                if fun.fqn == "fixtures.monomorph.wrap::<Int, eff fixtures.monomorph.Boom>"
        )));
        assert!(lowered.file.items.iter().any(|item| matches!(
            item,
            crate::mir::Item::Fun(fun)
                if fun.fqn == "fixtures.monomorph.wrap::<Int, eff fixtures.monomorph.Zap>"
        )));
    }

    #[test]
    fn monomorph_rewrites_top_level_fun_value_effect_instance() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_fun_value_effect_instance.scoop",
            r#"
package fixtures.monomorph

effect Boom {
    fun ping(): Unit
}

fun <eff E = Pure> forward(x: Int): Int / E {
    return x
}

fun <eff E = Pure> makeForward(): (Int) -> Int / E {
    return forward<eff E>
}

fun entry(): Int / Boom {
    val f = makeForward<eff Boom>()
    return f(1)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let lambda = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun)
                    if fun.fqn
                        == "fixtures.monomorph.makeForward::<eff fixtures.monomorph.Boom>.$lambda0" =>
                {
                    Some(fun)
                }
                _ => None,
            })
            .expect("expected instantiated lambda family member");
        let body = lambda
            .body
            .as_ref()
            .expect("lambda instance should have body");
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
            .expect("expected direct call in lambda instance");
        match call_kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                assert_eq!(
                    callee_fqn,
                    "fixtures.monomorph.forward::<eff fixtures.monomorph.Boom>"
                );
            }
            other => panic!("expected direct instantiated call, got {other:?}"),
        }

        assert!(lowered.file.items.iter().any(|item| matches!(
            item,
            crate::mir::Item::Fun(fun)
                if fun.fqn == "fixtures.monomorph.forward::<eff fixtures.monomorph.Boom>"
        )));
    }

    #[test]
    fn monomorph_rewrites_effect_generic_extension_call_to_concrete_instance() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/monomorph_extension_effect_instance.scoop",
            r#"
package fixtures.monomorph

effect Boom {
    fun ping(): Unit
}

fun <eff E = Pure> Int.forward(): Int / E {
    return this
}

fun <eff E = Pure> wrap(x: Int): Int / E {
    return x.forward<eff E>()
}

fun entry(): Int / Boom {
    return wrap<eff Boom>(1)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let forward_keys = lowered
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.monomorph.forward")
            .collect::<Vec<_>>();
        assert_eq!(forward_keys.len(), 1);
        assert_eq!(forward_keys[0].eff_args.len(), 1);
        assert!(!forward_keys[0].eff_args[0].is_pure());
        assert!(lowered.file.items.iter().any(|item| matches!(
            item,
            crate::mir::Item::Fun(fun)
                if fun.fqn == "fixtures.monomorph.forward::<eff fixtures.monomorph.Boom>"
        )));
        assert!(!lowered.file.items.iter().any(|item| matches!(
            item,
            crate::mir::Item::Fun(fun)
                if fun.fqn == "fixtures.monomorph.forward::<eff Pure>"
        )));

        let wrap = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::Fun(fun)
                    if fun.fqn == "fixtures.monomorph.wrap::<eff fixtures.monomorph.Boom>" =>
                {
                    Some(fun)
                }
                _ => None,
            })
            .expect("expected instantiated wrap body");
        let body = wrap.body.as_ref().expect("wrap instance should have body");
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
            .expect("expected direct call in wrap instance");
        match call_kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                assert_eq!(
                    callee_fqn,
                    "fixtures.monomorph.forward::<eff fixtures.monomorph.Boom>"
                );
            }
            other => panic!("expected instantiated extension direct call, got {other:?}"),
        }
    }
}
