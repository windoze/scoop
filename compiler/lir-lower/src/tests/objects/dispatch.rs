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
        vec![param("this", mir::Type::Class(c), this)],
        INT,
        returning_body(method_locals, int_expr(1)),
    );
    b.classes[c].vtable.push(mir::TableSlot::Function(m));
    // main: `val p: C; val r = p.m()` (the first ordinary virtual slot).
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let r = locals.alloc(local("r", INT));
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
                pending: mir::CoroutinePendingContext::Root,
            },
        )],
    );
    let module = lower(b.finish(main));

    // The receiver's object header (index 0) holds the TD; its
    // vtable pointer is ScoopTypeDescriptor field 5; the callee is
    // vtable[0].
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e(ptr<managed>) -> i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret integer<Int>(0x00000001)
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 p: ptr<managed>
    local %1 r: i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[local0:ptr<managed>@0]
    t0 = heap_load local0 +0 : ptr<metadata>
    t1 = heap_load t0 +40 : ptr<metadata>
    call managed-direct-target0 sp<managed-call:0> live=[local0:ptr<managed>@0] t2 = sig=direct0 (ptr<managed>) -> i32 dispatch[Virtual:0] t1(local0)
    store t2 -> local1
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn1() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td0 C @scoop$1$td$eb205ad260a812589e9f030260657692c3e8a971a60e730337a3c28f28bc6cc9 type-id=1930812111026443540 shape=FixedObject minimum-size=16 align=8 parent=none vtable=[local-fn0] itables=[]
  td td2 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td10 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout C value size=8 align=8 refs=[0]
  layout C size=16 align=8 refs=[]
  layout String value size=8 align=8 refs=[0]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
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
        vec![param("this", mir::Type::Interface(iface), this)],
        INT,
    );
    // main: `val i: Describable; val r = i.label()` (itable slot 1).
    let mut locals = Arena::new();
    let i = locals.alloc(local("i", mir::Type::Interface(iface)));
    let r = locals.alloc(local("r", INT));
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
                pending: mir::CoroutinePendingContext::Root,
            },
        )],
    );
    let module = lower(b.finish(main));

    // `scoop_rt_itable_lookup(td, iface_td)` finds the table; the
    // interface TD is a typed metadata reference, not an ordinary
    // globals-arena entry.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 i: ptr<managed>
    local %1 r: i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[local0:ptr<managed>@0]
    t0 = heap_load local0 +0 : ptr<metadata>
    call no-gc-direct-target0 t1 = sig=direct0 (ptr<metadata>, ptr<metadata>) -> ptr<metadata> runtime @scoop_rt_itable_lookup(t0, td0)
    call managed-direct-target0 sp<managed-call:0> live=[local0:ptr<managed>@0] t2 = sig=direct1 (ptr<managed>) -> i32 dispatch[Interface:1] t1(local0)
    store t2 -> local1
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
  td td0 Describable @scoop$1$td$2297a60bc362ce3d8b2494a877d19cf862c59a12c3f68dc36b859a026e02eecc type-id=2551552645907048390 shape=AbstractRef minimum-size=0 align=0 parent=none vtable=[] itables=[]
  td td2 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td10 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Describable value size=8 align=8 refs=[0]
  layout String value size=8 align=8 refs=[0]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}

#[test]
fn type_descriptors_carry_tables_parents_and_itables() {
    let mut b = Builder::new();
    let iface = b.interface("I", &["m"]);
    // Methods (bodies don't matter for the meta).
    let mut m_locals = Arena::new();
    let base_m_this = m_locals.alloc(local("this", INT));
    let base_m = b.user_fn_full(
        "Base.m",
        vec![param("this", INT, base_m_this)],
        mir::Type::Unit,
        m_locals,
        vec![],
    );
    let mut dm_locals = Arena::new();
    let derived_m_this = dm_locals.alloc(local("this", INT));
    let derived_m = b.user_fn_full(
        "Derived.m",
        vec![param("this", INT, derived_m_this)],
        mir::Type::Unit,
        dm_locals,
        vec![],
    );
    let mut dm2_locals = Arena::new();
    let derived_m2_this = dm2_locals.alloc(local("this", INT));
    let derived_m2 = b.user_fn_full(
        "Derived.m2",
        vec![param("this", INT, derived_m2_this)],
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
        &[("a", INT)],
        base_vtable,
        vec![mir::ItableRecord {
            interface: iface,
            slots: vec![mir::TableSlot::Function(base_m)],
        }],
    );
    let mut derived_vtable = empty_vtable();
    derived_vtable.push(mir::TableSlot::Function(derived_m));
    derived_vtable.push(mir::TableSlot::Function(derived_m2));
    let derived = b.class(
        "Derived",
        Some(base),
        // mir-lower flattens the base prefix into the field list.
        &[("a", INT), ("b", mir::Type::String)],
        derived_vtable,
        vec![mir::ItableRecord {
            interface: iface,
            slots: vec![mir::TableSlot::Function(derived_m)],
        }],
    );
    let main = b.main(Arena::new(), vec![]);
    let source = b.finish(main);
    let exact_type = |ty: &mir::Type| {
        source
            .meta
            .source_exact_types
            .get(ty)
            .expect("dispatch owner has an exact type identity")
            .identity_record()
            .id()
    };
    let interface_exact = exact_type(&mir::Type::Interface(iface));
    let base_exact = exact_type(&mir::Type::Class(base));
    let derived_exact = exact_type(&mir::Type::Class(derived));
    let module = lower(source);

    // Every materialized nominal has one descriptor. Interface and class
    // references remain typed even though value descriptors are also
    // materialized for strong production.
    assert_eq!(
        module.meta.type_descriptors.len(),
        module.meta.exact_types.len()
    );
    let descriptor_by_name = |name: &str| {
        module
            .meta
            .type_descriptors
            .iter()
            .find_map(|(id, descriptor)| {
                (descriptor.diagnostic_name == name)
                    .then_some((lir::TypeDescriptorRef::Local(id), descriptor))
            })
            .unwrap_or_else(|| panic!("missing descriptor {name}"))
    };
    let (i_ref, i_td) = descriptor_by_name("I");
    let (base_ref, base_td) = descriptor_by_name("Base");
    let (_, derived_td) = descriptor_by_name("Derived");
    for (descriptor, exact_type) in [
        (i_td, interface_exact),
        (base_td, base_exact),
        (derived_td, derived_exact),
    ] {
        assert!(
            descriptor
                .instance_layout
                .is_managed_instance_of(exact_type, lir::LirTargetProfile::DARWIN_AARCH64,)
        );
    }
    assert_eq!(
        i_td.identity.symbol_request().key(),
        scoop_identity::PersistentSymbolKey::TypeDescriptor(interface_exact)
    );
    assert_eq!(
        (
            i_td.instance_shape.minimum_size(),
            i_td.instance_shape.instance_alignment()
        ),
        (0, 0)
    );
    assert!(i_td.parent.is_none());
    assert_eq!(
        i_td.vtable.identity_record(),
        &scoop_identity::CborIdentityRecord::from_key(scoop_identity::DispatchTableKey::vtable(
            interface_exact
        ))
        .unwrap()
    );

    assert_eq!(base_td.diagnostic_name, "Base");
    assert_eq!(
        base_td.identity.symbol_request().key(),
        scoop_identity::PersistentSymbolKey::TypeDescriptor(base_exact)
    );
    // 16-byte header + Int @16 → size 24.
    assert_eq!(
        (
            base_td.instance_shape.minimum_size(),
            base_td.instance_shape.instance_alignment()
        ),
        (24, 8)
    );
    assert_eq!(*fixed_scan(base_td), lir::RefScan::None);
    assert!(base_td.parent.is_none());
    assert_eq!(
        base_td.vtable.slots(),
        [lir::DispatchEntry {
            callable: lir::CallableRef::Local(lir::LocalFunctionId::from_u32(0)),
        }]
    );
    assert_eq!(base_td.itables.len(), 1);
    assert_eq!(base_td.itables[0].interface(), i_ref);
    assert_eq!(base_td.itables[0].slots(), base_td.vtable.slots());
    assert_eq!(
        base_td.vtable.identity_record(),
        &scoop_identity::CborIdentityRecord::from_key(scoop_identity::DispatchTableKey::vtable(
            base_exact
        ))
        .unwrap()
    );
    assert_eq!(
        base_td.itables[0].identity_record(),
        &scoop_identity::CborIdentityRecord::from_key(scoop_identity::DispatchTableKey::itable(
            base_exact,
            interface_exact
        ))
        .unwrap()
    );

    assert_eq!(
        derived_td.identity.symbol_request().key(),
        scoop_identity::PersistentSymbolKey::TypeDescriptor(derived_exact)
    );
    assert_eq!(derived_td.parent, Some(base_ref));
    // header 16 + Int @16 + String @24 → size 32; the String is
    // the one reference.
    assert_eq!(
        (
            derived_td.instance_shape.minimum_size(),
            derived_td.instance_shape.instance_alignment()
        ),
        (32, 8)
    );
    assert_eq!(*fixed_scan(derived_td), lir::RefScan::References(vec![24]));
    assert_eq!(derived_td.vtable.slots().len(), 2);
    assert_eq!(
        derived_td.itables[0].slots(),
        [derived_td.vtable.slots()[0]]
    );
    assert_eq!(
        derived_td.vtable.identity_record(),
        &scoop_identity::CborIdentityRecord::from_key(scoop_identity::DispatchTableKey::vtable(
            derived_exact
        ))
        .unwrap()
    );
    assert_eq!(
        derived_td.itables[0].identity_record(),
        &scoop_identity::CborIdentityRecord::from_key(scoop_identity::DispatchTableKey::itable(
            derived_exact,
            interface_exact
        ))
        .unwrap()
    );
}

#[test]
fn class_layouts_shift_ref_offsets_by_the_header() {
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[
            ("a", INT),
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
    let s = b.strukt("S", &[("x", INT), ("s", mir::Type::String)]);
    let boxed = b.class(
        "box<S>",
        None,
        &[("value", mir::Type::Struct(s))],
        empty_vtable(),
        vec![],
    );
    let main = b.main(Arena::new(), vec![]);
    let mut mir_module = b.finish(main);
    register_boxed_source_nominal(
        &mut mir_module,
        mir::Type::Struct(s),
        boxed,
        "S",
        SourceNominalKind::Struct,
    );
    let module = lower(mir_module);

    let by_name = |name: &str| {
        layout_values(&module)
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("missing layout for {name}"))
    };
    // C: header 16; a @16, s @24, flag @32, r @40 → size 48.
    let c_layout = by_name("C");
    assert_eq!((c_layout.size, c_layout.align), (48, 8));
    assert_eq!(plain_refs(c_layout), [24, 40]);
    // box<S>: header 16 + payload { Int @0, String @8 } @16 → the
    // String lands at 24.
    let boxed_layout = by_name("box<S>");
    assert_eq!((boxed_layout.size, boxed_layout.align), (32, 8));
    assert_eq!(plain_refs(boxed_layout), [24]);
    // The TypeDescriptors carry the same reference offsets.
    let td = |name: &str| {
        descriptor_values(&module)
            .find(|td| td.diagnostic_name == name)
            .unwrap_or_else(|| panic!("missing TypeDescriptor for {name}"))
    };
    assert_eq!(*fixed_scan(td("C")), lir::RefScan::References(vec![24, 40]));
    assert_eq!(
        *fixed_scan(td("box<S>")),
        lir::RefScan::References(vec![24])
    );
    assert!(td("C").parent.is_none());
    assert!(td("box<S>").parent.is_none());
}
