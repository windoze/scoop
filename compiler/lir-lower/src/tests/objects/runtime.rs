//! Boxing, type tests, and GC runtime word operations.

use super::*;

#[test]
fn box_unbox_and_is_instance_lower_to_runtime_calls() {
    let mut b = Builder::new();
    let s = b.strukt("S", &[("x", mir::Type::Int)]);
    // mir-lower registers the boxed class of every checked / boxed
    // value type.
    let boxed = b.class(
        "box$D1_SX",
        None,
        &[("value", mir::Type::Struct(s))],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", mir::Type::Any));
    let v = locals.alloc(local("v", mir::Type::Struct(s)));
    let chk = locals.alloc(local("chk", mir::Type::Boolean));
    let main = b.main(
        locals,
        vec![
            val_decl(
                a,
                expr(
                    mir::Type::Any,
                    mir::ExprKind::Box(Box::new(expr(
                        mir::Type::Struct(s),
                        mir::ExprKind::StructInit {
                            struct_id: s,
                            args: vec![mir::Expr::int(1)],
                        },
                    ))),
                ),
            ),
            val_decl(
                v,
                expr(
                    mir::Type::Struct(s),
                    mir::ExprKind::Unbox(Box::new(local_expr(a, mir::Type::Any))),
                ),
            ),
            val_decl(
                chk,
                expr(
                    mir::Type::Boolean,
                    mir::ExprKind::IsInstance {
                        operand: Box::new(local_expr(a, mir::Type::Any)),
                        check_ty: Box::new(mir::Type::Struct(s)),
                    },
                ),
            ),
        ],
    );
    let mut mir_module = b.finish(main);
    mir_module.meta.boxed_types.push(mir::BoxedType {
        payload: mir::Type::Struct(s),
        class: boxed,
    });
    let module = lower(&mir_module);

    assert!(
        module
            .globals
            .iter()
            .all(|(_, global)| !global.symbol.starts_with("scoop_td_")),
        "TypeDescriptors must never be represented by ordinary globals"
    );

    // Box → `scoop_rt_box(td, payload, size, scan)`; Unbox → the payload
    // field behind the header; `is` → `scoop_rt_is_instance(obj,
    // td)`. Both checks share one typed descriptor reference.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop_main() -> void
    local %0 a: ptr<managed>
    local %1 v: struct0
    local %2 chk: i1
    local %3 $sc.1: struct0
  block entry
    poll managed-void-target0 sp2 live=[]
    t0 = aggregate (1) : struct0
    store t0 -> local3
    t1 = local_address local3 : ptr
    call managed-direct-target0 sp1 live=[] t2 = sig=direct0 (ptr<metadata>, ptr<raw>, i64, ptr<metadata>) -> ptr<managed> runtime @scoop_rt_box(td0, t1, 8, root-scan0)
    store t2 -> local0
    t3 = heap_load local0 +16 : struct0
    store t3 -> local1
    call no-gc-direct-target0 t4 = sig=direct1 (ptr<managed>, ptr<metadata>) -> i1 runtime @scoop_rt_is_instance(local0, td0)
    store t4 -> local2
    ret
  td td0 box$D1_SX @scoop_td_box$D1_SX type-id=2 size=24 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout S size=8 align=8 refs=[]
  layout box$D1_SX size=24 align=8 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn gc_intrinsics_exchange_words_with_the_runtime() {
    // The MIR shapes produced by M12's ordinary GC wrappers:
    // `_pin` / `_getGcHandle` return
    // the runtime's raw word into the handle struct, `unpin` /
    // `releaseGcHandle` unwrap field 0 for the reverse call, and
    // the hooks are a void call / an i64 result.
    let mut b = Builder::new();
    let pinned_ptr = b.strukt("PinnedPtr$S", &[("raw", mir::Type::UInt)]);
    let gc_handle = b.strukt("GcHandle$S", &[("raw", mir::Type::UInt)]);
    let mut locals = Arena::new();
    let v = locals.alloc(local("v", mir::Type::String));
    let raw_pin = locals.alloc(local("$call.1", mir::Type::UInt));
    let h = locals.alloc(local("h", mir::Type::Struct(pinned_ptr)));
    let gc1 = locals.alloc(local("$gc.1", mir::Type::String));
    let p = locals.alloc(local("p", mir::Type::String));
    let raw_handle = locals.alloc(local("$call.2", mir::Type::UInt));
    let gh = locals.alloc(local("gh", mir::Type::Struct(gc_handle)));
    let gc2 = locals.alloc(local("$gc.2", mir::Type::String));
    let p2 = locals.alloc(local("p2", mir::Type::String));
    let n = locals.alloc(local("n", mir::Type::UInt));
    let main = b.main(
        locals,
        vec![
            call_value(
                raw_pin,
                runtime_call(mir::RuntimeFn::Pin, vec![local_expr(v, mir::Type::String)]),
            ),
            val_decl(
                h,
                expr(
                    mir::Type::Struct(pinned_ptr),
                    mir::ExprKind::StructInit {
                        struct_id: pinned_ptr,
                        args: vec![local_expr(raw_pin, mir::Type::UInt)],
                    },
                ),
            ),
            call_value(
                gc1,
                runtime_call(
                    mir::RuntimeFn::Unpin,
                    vec![expr(
                        mir::Type::UInt,
                        mir::ExprKind::FieldAccess {
                            receiver: Box::new(local_expr(h, mir::Type::Struct(pinned_ptr))),
                            index: 0,
                        },
                    )],
                ),
            ),
            val_decl(p, local_expr(gc1, mir::Type::String)),
            call_value(
                raw_handle,
                runtime_call(
                    mir::RuntimeFn::GetHandle,
                    vec![local_expr(v, mir::Type::String)],
                ),
            ),
            val_decl(
                gh,
                expr(
                    mir::Type::Struct(gc_handle),
                    mir::ExprKind::StructInit {
                        struct_id: gc_handle,
                        args: vec![local_expr(raw_handle, mir::Type::UInt)],
                    },
                ),
            ),
            call_value(
                gc2,
                runtime_call(
                    mir::RuntimeFn::ReleaseHandle,
                    vec![expr(
                        mir::Type::UInt,
                        mir::ExprKind::FieldAccess {
                            receiver: Box::new(local_expr(gh, mir::Type::Struct(gc_handle))),
                            index: 0,
                        },
                    )],
                ),
            ),
            val_decl(p2, local_expr(gc2, mir::Type::String)),
            call_stmt(runtime_call(mir::RuntimeFn::GcCollect, vec![])),
            call_value(n, runtime_call(mir::RuntimeFn::GcStats, vec![])),
        ],
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop_main() -> void
    local %0 v: ptr<managed>
    local %1 $call.1: i64
    local %2 h: struct0
    local %3 $gc.1: ptr<managed>
    local %4 p: ptr<managed>
    local %5 $call.2: i64
    local %6 gh: struct1
    local %7 $gc.2: ptr<managed>
    local %8 p2: ptr<managed>
    local %9 n: i64
  block entry
    poll managed-void-target1 sp2 live=[local0:ptr<managed>@0]
    call no-gc-direct-target0 t0 = sig=direct0 (ptr<managed>) -> i64 runtime @scoop_rt_pin(local0)
    store t0 -> local1
    t1 = aggregate (local1) : struct0
    store t1 -> local2
    t2 = extract local2, 0 : i64
    call no-gc-direct-target1 t3 = sig=direct1 (i64) -> ptr<managed> runtime @scoop_rt_unpin(t2)
    store t3 -> local3
    store local3 -> local4
    call no-gc-direct-target2 t4 = sig=direct2 (ptr<managed>) -> i64 runtime @scoop_rt_get_handle(local0)
    store t4 -> local5
    t5 = aggregate (local5) : struct1
    store t5 -> local6
    t6 = extract local6, 0 : i64
    call no-gc-direct-target3 t7 = sig=direct3 (i64) -> ptr<managed> runtime @scoop_rt_release_handle(t6)
    store t7 -> local7
    store local7 -> local8
    call managed-void-target0 sp1 live=[] sig=void0 () runtime @scoop_rt_gc_collect()
    t8 = aggregate () : {}
    call no-gc-direct-target4 t9 = sig=direct4 () -> i64 runtime @scoop_rt_gc_stats()
    store t9 -> local9
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout PinnedPtr$S size=8 align=8 refs=[]
  layout GcHandle$S size=8 align=8 refs=[]
  entry @scoop_main
"###);
}
