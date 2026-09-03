//! Virtual/interface dispatch and descriptor table contracts.

use super::*;

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
