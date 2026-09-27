use super::*;

mod odr;

#[test]
fn trap_calls_branch_to_a_shared_trap_block() {
    // fun f(o: Option<Int>): Int { return o!! + o!! } — in the
    // mir-lower shape: each `o!!` is `if (tag == Some) { val $uw =
    // field0 } else { trap(msg) }`.
    let mut b = Builder::new();
    let option_i = b.option_enum("Option<Int>", INT);
    let option_ty = mir::Type::Enum(option_i, vec![INT]);
    let message = b.string("unwrap on None (function f)");
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_ty.clone()));
    let uw1 = locals.alloc(local("$uw.1", INT));
    let uw2 = locals.alloc(local("$uw.2", INT));
    let cond = || {
        binary(
            mir::BinOp::MachineEq(mir::MachineScalarKind::EnumTag),
            expr(
                mir::Type::MachineScalar(mir::MachineScalarKind::EnumTag),
                mir::ExprKind::EnumTag(Box::new(local_expr(o, option_ty.clone()))),
            ),
            mir::Expr::machine_scalar(mir::MachineScalarValue::EnumTag(0)),
            mir::Type::Boolean,
        )
    };
    let init = |result| {
        val_decl(
            result,
            expr(
                INT,
                mir::ExprKind::EnumField {
                    operand: Box::new(local_expr(o, option_ty.clone())),
                    variant: 0,
                    index: 0,
                },
            ),
        )
    };
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let then1 = cfg_block(&mut blocks, "if.then.1");
    let else1 = cfg_block(&mut blocks, "if.else.2");
    let merge1 = cfg_block(&mut blocks, "if.merge.3");
    let then2 = cfg_block(&mut blocks, "if.then.5");
    let else2 = cfg_block(&mut blocks, "if.else.6");
    let merge2 = cfg_block(&mut blocks, "if.merge.7");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Branch {
            cond: cond(),
            then_block: then1,
            else_block: else1,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then1,
        vec![init(uw1)],
        mir::Terminator::Goto(merge1),
        None,
    );
    set_cfg_block(
        &mut blocks,
        else1,
        Vec::new(),
        mir::Terminator::Trap { message },
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge1,
        Vec::new(),
        mir::Terminator::Branch {
            cond: cond(),
            then_block: then2,
            else_block: else2,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then2,
        vec![init(uw2)],
        mir::Terminator::Goto(merge2),
        None,
    );
    set_cfg_block(
        &mut blocks,
        else2,
        Vec::new(),
        mir::Terminator::Trap { message },
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge2,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(integer_binary_expr(
                mir::IntegerKind::SIGNED_32,
                mir::IntegerBinaryOperator::Add,
                local_expr(uw1, INT),
                local_expr(uw2, INT),
            )),
        },
        None,
    );
    let f = b.user_fn_body(
        "f",
        vec![param("o", option_ty, o)],
        INT,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), Vec::new());
    let module = lower(b.finish(main));

    let trap_global = module
        .globals
        .iter()
        .find_map(|(_, global)| match &global.init {
            lir::GlobalInit::CString { identity, .. } => Some((global, identity)),
            _ => None,
        })
        .expect("the trap body owns one C string support atom");
    assert_eq!(
        trap_global.1.owner(),
        module.functions[0].callable_body.id()
    );
    assert_eq!(
        trap_global.1.path().segments(),
        &[StructuralPathSegment::new(
            StructuralDefinitionSiteRole::StringConstant,
            0,
        )]
    );
    let foundation = lir::ConeLirFoundation::from_module(&module).unwrap();
    let surface = lir::ObjectSymbolSurfaceV1::from_foundation(&foundation).unwrap();
    let boundary = surface
        .plans()
        .iter()
        .flat_map(|plan| plan.atom_boundaries())
        .find(|boundary| boundary.atom() == trap_global.1.atom_record().id())
        .expect("the trap support atom has physical boundaries");
    assert_eq!(boundary.start().symbol().as_str(), trap_global.0.symbol());

    // Both `!!` share the one trap block of the function.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$io$628de209327518e6dd1b8cb671b0800d34d8c4a09fd4dafae1ff244dfb49e582 = "unwrap on None (function f)"
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  global @scoop$1$bs$00437761c5a0d7252a5394267da8aa30509fe249acd6e8aff2304d37b1b16433 = c"unwrap on None (function f)"
  enum Option<Int> tagged size=16 align=8 variants=(i32)@8+4 ()@8+0
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e(indirect<enum0 size=16 align=8 scan=none>) -> i32
    local %0 $uw.1: i32
    local %1 $uw.2: i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    t0 = enum_tag e0 param0 : machine<enum-tag>
    t1 = MachineEq(EnumTag) t0, machine<enum-tag>(EnumTag(0)) : i1
    cbr t1 then @if.then.1 else @if.else.2
  block if.then.1
    t2 = enum_field e0 v0 f0 param0 : i32
    store t2 -> local0
    br @if.merge.3
  block if.else.2
    br @unwrap.trap.1
  block if.merge.3
    t3 = enum_tag e0 param0 : machine<enum-tag>
    t4 = MachineEq(EnumTag) t3, machine<enum-tag>(EnumTag(0)) : i1
    cbr t4 then @if.then.5 else @if.else.6
  block if.then.5
    t5 = enum_field e0 v0 f0 param0 : i32
    store t5 -> local1
    br @if.merge.7
  block if.else.6
    br @unwrap.trap.1
  block if.merge.7
    t6 = integer_Add<Int> local0, local1 : i32
    ret t6
  block unwrap.trap.1
    call no-gc-void-target0 sig=void0 (ptr<raw>) runtime @scoop_rt_trap(global2)
    unreachable
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
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
    global_store global1, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Option<Int><Int> @scoop$1$td$788cbf7a74af24a4ac1fd29f1cc8c147f7003a53cef568d986886a74b83631a7 type-id=4844072839988068642 shape=BoxedValue minimum-size=32 align=8 parent=none vtable=[] itables=[]
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
  layout Option<Int> size=16 align=8 enum-scan=none
  layout String value size=8 align=8 refs=[0]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}
