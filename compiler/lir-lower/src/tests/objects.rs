use super::*;

// ---- M6: reference types ----

#[test]
fn virtual_calls_load_the_vtable_and_call_indirect() {
    let mut b = Builder::new();
    let c = b.class("C", None, &[], empty_vtable(), vec![]);
    // `C.m(this: C): Int { return 1 }`.
    let mut method_locals = Arena::new();
    let this = method_locals.alloc(local("this", mir::Type::Class(c)));
    let m = b.user_fn_body(
        "C.m",
        "scoop.C.m",
        vec![param("this", mir::Type::Class(c), this)],
        mir::Type::Int,
        returning_body(method_locals, mir::Expr::int(1)),
    );
    b.classes[c].vtable.push(mir::TableSlot::Function(m));
    // main: `val p: C; val r = p.m()` (the first ordinary virtual slot).
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let r = locals.alloc(local("r", mir::Type::Int));
    let main = b.main(
        locals,
        vec![call_value(
            r,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Virtual { slot: 0 },
                    callee: mir::Callee::User(m),
                },
                args: vec![local_expr(p, mir::Type::Class(c))],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // The receiver's object header (index 0) holds the TD; its
    // vtable pointer is ScoopTypeDescriptor field 5; the callee is
    // vtable[0].
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop.C.m(ptr<managed>) -> i64
  block entry
    poll managed-void-target0 sp2 live=[]
    ret 1
  fun @scoop_main() -> void
    local %0 p: ptr<managed>
    local %1 r: i64
  block entry
    poll managed-void-target0 sp3 live=[local0:ptr<managed>@0]
    t0 = heap_load local0 +0 : ptr<metadata>
    t1 = heap_load t0 +40 : ptr<metadata>
    call managed-direct-target0 sp1 live=[local0:ptr<managed>@0] t2 = sig=direct0 (ptr<managed>) -> i64 dispatch[Virtual:0] t1(local0)
    store t2 -> local1
    ret
  td td0 C @scoop_td_C type-id=2 size=16 parent=none vtable=[local-fn0] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout C size=16 align=8 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn interface_calls_look_up_the_itable() {
    let mut b = Builder::new();
    let iface = b.interface("Describable", &["describe", "label"]);
    // The interface method shell (signature only, never emitted).
    let mut shell_locals = Arena::new();
    let this = shell_locals.alloc(local("this", mir::Type::Interface(iface)));
    let label = b.decl_fn(
        "Describable.label",
        "scoop.Describable.label",
        vec![param("this", mir::Type::Interface(iface), this)],
        mir::Type::Int,
    );
    // main: `val i: Describable; val r = i.label()` (itable slot 1).
    let mut locals = Arena::new();
    let i = locals.alloc(local("i", mir::Type::Interface(iface)));
    let r = locals.alloc(local("r", mir::Type::Int));
    let main = b.main(
        locals,
        vec![call_value(
            r,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: iface,
                        slot: 1,
                    },
                    callee: mir::Callee::User(label),
                },
                args: vec![local_expr(i, mir::Type::Interface(iface))],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // `scoop_rt_itable_lookup(td, iface_td)` finds the table; the
    // interface TD is a typed metadata reference, not an ordinary
    // globals-arena entry.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop_main() -> void
    local %0 i: ptr<managed>
    local %1 r: i64
  block entry
    poll managed-void-target0 sp2 live=[local0:ptr<managed>@0]
    t0 = heap_load local0 +0 : ptr<metadata>
    call no-gc-direct-target0 t1 = sig=direct0 (ptr<metadata>, ptr<metadata>) -> ptr<metadata> runtime @scoop_rt_itable_lookup(t0, td0)
    call managed-direct-target0 sp1 live=[local0:ptr<managed>@0] t2 = sig=direct1 (ptr<managed>) -> i64 dispatch[Interface:1] t1(local0)
    store t2 -> local1
    ret
  td td0 Describable @scoop_td_Describable type-id=2 size=0 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn type_descriptors_carry_tables_parents_and_itables() {
    let mut b = Builder::new();
    let iface = b.interface("I", &["m"]);
    // Methods (bodies don't matter for the meta).
    let mut m_locals = Arena::new();
    let base_m_this = m_locals.alloc(local("this", mir::Type::Int));
    let base_m = b.user_fn_full(
        "Base.m",
        "scoop.Base.m",
        vec![param("this", mir::Type::Int, base_m_this)],
        mir::Type::Unit,
        m_locals,
        vec![],
    );
    let mut dm_locals = Arena::new();
    let derived_m_this = dm_locals.alloc(local("this", mir::Type::Int));
    let derived_m = b.user_fn_full(
        "Derived.m",
        "scoop.Derived.m",
        vec![param("this", mir::Type::Int, derived_m_this)],
        mir::Type::Unit,
        dm_locals,
        vec![],
    );
    let mut dm2_locals = Arena::new();
    let derived_m2_this = dm2_locals.alloc(local("this", mir::Type::Int));
    let derived_m2 = b.user_fn_full(
        "Derived.m2",
        "scoop.Derived.m2",
        vec![param("this", mir::Type::Int, derived_m2_this)],
        mir::Type::Unit,
        dm2_locals,
        vec![],
    );
    // Base implements I; Derived overrides `m` and adds `m2`.
    let mut base_vtable = empty_vtable();
    base_vtable.push(mir::TableSlot::Function(base_m));
    let base = b.class(
        "Base",
        None,
        &[("a", mir::Type::Int)],
        base_vtable,
        vec![mir::ItableRecord {
            interface: iface,
            slots: vec![mir::TableSlot::Function(base_m)],
        }],
    );
    let mut derived_vtable = empty_vtable();
    derived_vtable.push(mir::TableSlot::Function(derived_m));
    derived_vtable.push(mir::TableSlot::Function(derived_m2));
    let _derived = b.class(
        "Derived",
        Some(base),
        // mir-lower flattens the base prefix into the field list.
        &[("a", mir::Type::Int), ("b", mir::Type::String)],
        derived_vtable,
        vec![mir::ItableRecord {
            interface: iface,
            slots: vec![mir::TableSlot::Function(derived_m)],
        }],
    );
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    // Interfaces first (itable keys), then classes
    // base-before-derived — references always name
    // already-emitted entries.
    assert_eq!(module.meta.type_descriptors.len(), 4);
    let descriptor_by_name = |name: &str| {
        module
            .meta
            .type_descriptors
            .iter()
            .find_map(|(id, descriptor)| {
                (descriptor.name == name).then_some((lir::TypeDescriptorRef::Local(id), descriptor))
            })
            .unwrap_or_else(|| panic!("missing descriptor {name}"))
    };
    let (i_ref, i_td) = descriptor_by_name("I");
    let (base_ref, base_td) = descriptor_by_name("Base");
    let (_, derived_td) = descriptor_by_name("Derived");
    let (_, string_td) = descriptor_by_name("String");
    assert_eq!(i_td.symbol, "scoop_td_I");
    assert_eq!((i_td.size, i_td.align), (0, 0));
    assert!(i_td.parent.is_none());

    assert_eq!(base_td.name, "Base");
    assert_eq!(base_td.symbol, "scoop_td_Base");
    // 16-byte header + Int @16 → size 24.
    assert_eq!((base_td.size, base_td.align), (24, 8));
    assert_eq!(*fixed_scan(base_td), lir::RefScan::None);
    assert!(base_td.parent.is_none());
    assert_eq!(
        base_td.vtable,
        [lir::DispatchEntry {
            callable: lir::CallableRef::Local(lir::LocalFunctionId::from_u32(0)),
        }]
    );
    assert_eq!(base_td.itables.len(), 1);
    assert_eq!(base_td.itables[0].interface, i_ref);
    assert_eq!(base_td.itables[0].slots, base_td.vtable);

    assert_eq!(derived_td.symbol, "scoop_td_Derived");
    assert_eq!(derived_td.parent, Some(base_ref));
    // header 16 + Int @16 + String @24 → size 32; the String is
    // the one reference.
    assert_eq!((derived_td.size, derived_td.align), (32, 8));
    assert_eq!(*fixed_scan(derived_td), lir::RefScan::References(vec![24]));
    assert_eq!(derived_td.vtable.len(), 2);
    assert_eq!(derived_td.itables[0].slots, [derived_td.vtable[0]]);
    assert_eq!(string_td.symbol, lir::STRING_TD_SYMBOL);
}

#[test]
fn class_layouts_shift_ref_offsets_by_the_header() {
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[
            ("a", mir::Type::Int),
            ("s", mir::Type::String),
            ("flag", mir::Type::Boolean),
            ("r", mir::Type::Any),
        ],
        empty_vtable(),
        vec![],
    );
    let _ = c;
    // A boxed value type: header + the inline payload; references
    // inside the payload shift by the header too.
    let s = b.strukt("S", &[("x", mir::Type::Int), ("s", mir::Type::String)]);
    let boxed = b.class(
        "box$S",
        None,
        &[("value", mir::Type::Struct(s))],
        empty_vtable(),
        vec![],
    );
    let main = b.main(Arena::new(), vec![]);
    let mut mir_module = b.finish(main);
    mir_module.meta.boxed_types.push(mir::BoxedType {
        payload: mir::Type::Struct(s),
        class: boxed,
    });
    let module = lower(&mir_module);

    let by_name = |name: &str| {
        layout_values(&module)
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("missing layout for {name}"))
    };
    // C: header 16; a @16, s @24, flag @32, r @40 → size 48.
    let c_layout = by_name("C");
    assert_eq!((c_layout.size, c_layout.align), (48, 8));
    assert_eq!(plain_refs(c_layout), [24, 40]);
    // box$S: header 16 + payload { Int @0, String @8 } @16 → the
    // String lands at 24.
    let boxed_layout = by_name("box$S");
    assert_eq!((boxed_layout.size, boxed_layout.align), (32, 8));
    assert_eq!(plain_refs(boxed_layout), [24]);
    // The TypeDescriptors carry the same reference offsets.
    let td = |name: &str| {
        descriptor_values(&module)
            .find(|td| td.name == name)
            .unwrap_or_else(|| panic!("missing TypeDescriptor for {name}"))
    };
    assert_eq!(*fixed_scan(td("C")), lir::RefScan::References(vec![24, 40]));
    assert_eq!(*fixed_scan(td("box$S")), lir::RefScan::References(vec![24]));
    assert!(td("C").parent.is_none());
    assert!(td("box$S").parent.is_none());
}

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

#[test]
fn class_field_reads_are_heap_loads() {
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[("a", mir::Type::Int), ("s", mir::Type::String)],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let s = locals.alloc(local("s", mir::Type::String));
    let main = b.main(
        locals,
        vec![val_decl(
            s,
            expr(
                mir::Type::String,
                mir::ExprKind::FieldAccess {
                    receiver: Box::new(local_expr(p, mir::Type::Class(c))),
                    index: 1,
                },
            ),
        )],
    );
    let module = lower(&b.finish(main));

    // The String field follows the 16-byte header and Int field,
    // so its natural byte offset is 24.
    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::HeapLoad { out, offset, .. } = instructions[0] else {
        panic!("a class field read must be a heap object load")
    };
    assert_eq!(*offset, 24);
    assert_eq!(function.temps[*out].ty, lir::MANAGED_PTR);
}

#[test]
fn class_init_allocates_and_stores_fields() {
    // The ctor body mir-lower generates for
    // `class Point(val x: Int, val s: String)`:
    // `return ClassInit Point [x, s]` — allocation plus one heap
    // store per flattened field.
    let mut b = Builder::new();
    let str_x = b.string("x");
    let point = b.class(
        "Point",
        None,
        &[("x", mir::Type::Int), ("s", mir::Type::String)],
        empty_vtable(),
        vec![],
    );
    let mut ctor_locals = Arena::new();
    let x = ctor_locals.alloc(local("x", mir::Type::Int));
    let s = ctor_locals.alloc(local("s", mir::Type::String));
    let ctor = b.user_fn_body(
        "ctor.Point",
        "scoop.ctor.Point",
        vec![
            param("x", mir::Type::Int, x),
            param("s", mir::Type::String, s),
        ],
        mir::Type::Class(point),
        returning_body(
            ctor_locals,
            expr(
                mir::Type::Class(point),
                mir::ExprKind::ClassInit {
                    class_id: point,
                    args: vec![
                        local_expr(x, mir::Type::Int),
                        local_expr(s, mir::Type::String),
                    ],
                },
            ),
        ),
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(point)));
    let main = b.main(
        locals,
        vec![call_value(
            p,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(ctor),
                },
                args: vec![mir::Expr::int(1), string_expr(str_x)],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // `scoop_rt_alloc(td, size)` with the class layout size (16
    // header + Int @16 + String @24 = 32), then the fields at
    // those byte offsets.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "x"
  fun @scoop.ctor.Point(i64, ptr<managed>) -> ptr<managed>
  block entry
    poll managed-void-target0 sp3 live=[param1:ptr<managed>@0]
    call managed-direct-target0 sp1 live=[param1:ptr<managed>@0] t0 = sig=direct0 (ptr<metadata>, i64) -> ptr<managed> runtime @scoop_rt_alloc(td0, 32)
    heap_store t0 +16 param0
    heap_store t0 +24 param1
    ret t0
  fun @scoop_main() -> void
    local %0 p: ptr<managed>
  block entry
    poll managed-void-target0 sp4 live=[]
    call managed-direct-target0 sp2 live=[] t0 = sig=direct0 (i64, ptr<managed>) -> ptr<managed> local-fn0(1, global0)
    store t0 -> local0
    ret
  td td0 Point @scoop_td_Point type-id=2 size=32 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Point size=32 align=8 refs=[24]
  entry @scoop_main
"###);
}

#[test]
fn field_set_lowers_to_a_heap_store() {
    // `p.y = 3`: MIR FieldSet index 1 → byte offset 24 after the
    // 16-byte header and the first Int field.
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[("x", mir::Type::Int), ("y", mir::Type::Int)],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let main = b.main(
        locals,
        vec![stmt(mir::StatementKind::FieldSet {
            object: local_expr(p, mir::Type::Class(c)),
            index: 1,
            value: mir::Expr::int(3),
        })],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::HeapStore {
        object,
        offset: 24,
        value,
    } = instructions[0]
    else {
        panic!("a FieldSet must lower to a HeapStore")
    };
    assert!(matches!(object, lir::Value::Local(_)));
    assert!(matches!(value, lir::Value::IntConst(3)));
}

#[test]
fn a_trap_only_body_seals_the_function() {
    // mir-lower's abstract-method stub is a single trap call: the
    // block is sealed by the trap branch, so the "non-Unit
    // functions must end with `return`" check must not fire (it
    // applies to hir-lower-produced bodies that fall off the end,
    // not to noreturn bodies like this one).
    let mut b = Builder::new();
    let message = b.string("call to abstract method `Base.id`");
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Any));
    let _stub = b.user_fn_full(
        "Base.id",
        "scoop.Base.id",
        vec![param("this", mir::Type::Any, this)],
        mir::Type::Int,
        locals,
        vec![call_stmt(runtime_call(
            mir::RuntimeFn::Trap,
            vec![string_expr(message)],
        ))],
    );
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    assert!(matches!(
        function.blocks[function.entry].terminator,
        lir::Terminator::Br(_)
    ));
}
