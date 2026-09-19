//! Boxing, type tests, and GC runtime word operations.

use super::*;

#[test]
fn box_unbox_and_is_instance_lower_to_runtime_calls() {
    let mut b = Builder::new();
    let s = b.strukt("S", &[("x", INT)]);
    // mir-lower registers the boxed class of every checked / boxed
    // value type.
    let boxed = b.class(
        "box<S>",
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
                            args: vec![int_expr(1)],
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
    register_boxed_source_nominal(
        &mut mir_module,
        mir::Type::Struct(s),
        boxed,
        "S",
        SourceNominalKind::Struct,
    );
    let module = lower(mir_module);

    let descriptor_symbols = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| descriptor.identity.symbol())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        module
            .globals
            .iter()
            .all(|(_, global)| !descriptor_symbols.contains(global.symbol())),
        "TypeDescriptors must never be represented by ordinary globals"
    );

    // Dedicated operations carry the same checked descriptor identity;
    // unbox validates it before writing the typed destination local.
    insta::assert_snapshot!(lir::dump(&module));
}

#[test]
fn gc_intrinsics_exchange_words_with_the_runtime() {
    // The MIR shapes produced by M12's ordinary GC wrappers:
    // `_pin` / `_getGcHandle` return
    // the runtime's raw word into the handle struct, `unpin` /
    // `releaseGcHandle` unwrap field 0 for the reverse call, and
    // the hooks are a void call / an i64 result.
    let mut b = Builder::new();
    let pinned_ptr = b.strukt("PinnedPtr<String>", &[("raw", ULONG)]);
    let gc_handle = b.strukt("GcHandle<String>", &[("raw", ULONG)]);
    let mut locals = Arena::new();
    let v = locals.alloc(local("v", mir::Type::String));
    let raw_pin = locals.alloc(local("$call.1", ULONG));
    let h = locals.alloc(local("h", mir::Type::Struct(pinned_ptr)));
    let gc1 = locals.alloc(local("$gc.1", mir::Type::String));
    let p = locals.alloc(local("p", mir::Type::String));
    let raw_handle = locals.alloc(local("$call.2", ULONG));
    let gh = locals.alloc(local("gh", mir::Type::Struct(gc_handle)));
    let gc2 = locals.alloc(local("$gc.2", mir::Type::String));
    let p2 = locals.alloc(local("p2", mir::Type::String));
    let n = locals.alloc(local("n", ULONG));
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
                        args: vec![local_expr(raw_pin, ULONG)],
                    },
                ),
            ),
            call_value(
                gc1,
                runtime_call(
                    mir::RuntimeFn::Unpin,
                    vec![expr(
                        ULONG,
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
                        args: vec![local_expr(raw_handle, ULONG)],
                    },
                ),
            ),
            call_value(
                gc2,
                runtime_call(
                    mir::RuntimeFn::ReleaseHandle,
                    vec![expr(
                        ULONG,
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
    let module = lower(b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
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
    poll managed-void-target1 sp<managed-poll:0> live=[local0:ptr<managed>@0]
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
    call managed-void-target0 sp<managed-call:0> live=[] sig=void0 () runtime @scoop_rt_gc_collect()
    t8 = aggregate () : {}
    call no-gc-direct-target4 t9 = sig=direct4 () -> i64 runtime @scoop_rt_gc_stats()
    store t9 -> local9
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn0() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td0 GcHandle<String> @scoop$1$td$05c79b4b37c953c45e58822887717a10433301d3b6827935c370b4e5d8454aa6 type-id=3511904370696034429 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td1 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td2 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td10 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td11 PinnedPtr<String> @scoop$1$td$f3d51e63e79edcfed0b8e70e8e6866f3024b397dca5d7feb65f5b262cfa588e9 type-id=13273347026211627739 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout PinnedPtr<String> size=8 align=8 refs=[]
  layout GcHandle<String> size=8 align=8 refs=[]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}
