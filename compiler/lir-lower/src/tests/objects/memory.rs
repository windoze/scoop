//! Class construction, field memory access, and trap termination.

use super::*;

#[test]
fn class_field_reads_are_heap_loads() {
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[("a", INT), ("s", mir::Type::String)],
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
    let module = lower(b.finish(main));

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
fn class_allocation_and_initializer_store_fields() {
    // M19 splits class construction into an exact allocation at the use site
    // and a Unit-returning initializer call on the same receiver.
    let mut b = Builder::new();
    let str_x = b.string("x");
    let point = b.class(
        "Point",
        None,
        &[("x", INT), ("s", mir::Type::String)],
        empty_vtable(),
        vec![],
    );
    let mut ctor_locals = Arena::new();
    let this = ctor_locals.alloc(local("this", mir::Type::Class(point)));
    let x = ctor_locals.alloc(local("x", INT));
    let s = ctor_locals.alloc(local("s", mir::Type::String));
    let ctor = b.user_fn_body(
        "init.Point.$c0",
        vec![
            param("this", mir::Type::Class(point), this),
            param("x", INT, x),
            param("s", mir::Type::String, s),
        ],
        mir::Type::Unit,
        body_with_terminator(
            ctor_locals,
            vec![
                stmt(mir::StatementKind::FieldSet {
                    object: local_expr(this, mir::Type::Class(point)),
                    index: 0,
                    value: local_expr(x, INT),
                }),
                stmt(mir::StatementKind::FieldSet {
                    object: local_expr(this, mir::Type::Class(point)),
                    index: 1,
                    value: local_expr(s, mir::Type::String),
                }),
            ],
            mir::Terminator::Return { value: None },
        ),
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(point)));
    let main = b.main(
        locals,
        vec![
            val_decl(
                p,
                expr(
                    mir::Type::Class(point),
                    mir::ExprKind::ClassAlloc { class_id: point },
                ),
            ),
            call_stmt(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(ctor),
                },
                args: vec![
                    local_expr(p, mir::Type::Class(point)),
                    int_expr(1),
                    string_expr(str_x),
                ],
                pending: mir::CoroutinePendingContext::Root,
            }),
        ],
    );
    let module = lower(b.finish(main));

    // `scoop_rt_alloc(td, size)` with the class layout size (16
    // header + Int @16 + String @24 = 32), then the fields at
    // those byte offsets.
    insta::assert_snapshot!(lir::dump(&module), @r#"
    Module
      global @scoop$1$io$628de209327518e6dd1b8cb671b0800d34d8c4a09fd4dafae1ff244dfb49e582 = "x"
      global @scoop$1$ss$9b273ab0bbc562dd7f8e8b0487c0e98f4a7d0781b1cb5aa5b6d69c2d8a7f66b1 : ptr<managed> scan=refs[0]
      fun @scoop$1$cb$6cb4a66fa9aacac49c232a58b41d5c6876ef37a3aeb59531418a4b8f28cce6e2(ptr<managed>, i32, ptr<managed>) -> void
      block entry
        poll managed-void-target0 sp<managed-poll:0> live=[param0:ptr<managed>@0, param2:ptr<managed>@0]
        heap_store param0 +16 param1
        heap_store param0 +24 param2
        ret
      fun @scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca() -> void
        local %0 p: ptr<managed>
      block entry
        poll managed-void-target1 sp<managed-poll:0> live=[]
        call managed-direct-target0 sp<managed-call:0> live=[] t0 = sig=direct0 (ptr<metadata>, machine<byte-size>) -> ptr<managed> runtime @scoop_rt_alloc(td0, machine<byte-size>(ByteSize(32)))
        store t0 -> local0
        call managed-void-target0 sp<managed-call:1> live=[local0:ptr<managed>@0] sig=void0 (ptr<managed>, i32, ptr<managed>) local-fn0(local0, integer<Int>(0x00000001), global0)
        t1 = aggregate () : {}
        ret
      fun @scoop$1$cb$d3bd523ea7c4b775508c06e622f76772db6a21fddb406c6d3fe7d1f20a2a89c1(i32, ptr<raw>, ptr<raw>) -> i32
      block entry
        poll managed-void-target1 sp<managed-poll:0> live=[]
        call managed-direct-target1 sp<managed-call:0> live=[] t4 = sig=direct1 (ptr<metadata>) -> ptr<managed> runtime @scoop_rt_context_ensure_root(td11)
        invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn1() normal @success unwind @failure
        br @success
      block success
        raw_store param2 integer<Int>(0x00000000) align 4
        ret integer<UInt>(0x00000000)
      block failure
        (t0, t1) = landingpad : (exception_record, ptr<raw>)
        t2 = begin_catch t1 : ptr<managed>
        call managed-direct-target0 sp<managed-call:1> live=[t2:ptr<managed>@0] t3 = sig=direct0 (ptr<managed>) -> ptr<managed> runtime @scoop_rt_materialize_exception(t2)
        global_store global1, t3
        end_catch
        ret integer<UInt>(0x00000001)
      td td0 Point @scoop$1$td$eb205ad260a812589e9f030260657692c3e8a971a60e730337a3c28f28bc6cc9 type-id=1930812111026443540 shape=FixedObject minimum-size=32 align=8 parent=none vtable=[] itables=[]
      td td2 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td3 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td4 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td5 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td6 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td7 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td8 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td9 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td10 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
      td td11 task-context @scoop$1$td$db9fdace23f2040d3622172122120f4e46c6786180d6495caf23e609df021eac type-id=11462109518987149384 shape=FixedObject minimum-size=24 align=8 parent=none vtable=[] itables=[]
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
      layout task-context value size=8 align=8 refs=[0]
      layout task-context size=24 align=8 refs=[16]
      layout Point value size=8 align=8 refs=[0]
      layout Point size=32 align=8 refs=[24]
      layout String value size=8 align=8 refs=[0]
      output executable @scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca
    "#);
}

#[test]
fn field_set_lowers_to_a_heap_store() {
    // `p.y = 3`: MIR FieldSet index 1 → byte offset 20 after the
    // 16-byte header and the first Int field.
    let mut b = Builder::new();
    let c = b.class("C", None, &[("x", INT), ("y", INT)], empty_vtable(), vec![]);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let main = b.main(
        locals,
        vec![stmt(mir::StatementKind::FieldSet {
            object: local_expr(p, mir::Type::Class(c)),
            index: 1,
            value: int_expr(3),
        })],
    );
    let module = lower(b.finish(main));

    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::HeapStore {
        object,
        offset: 20,
        value,
    } = instructions[0]
    else {
        panic!("a FieldSet must lower to a HeapStore")
    };
    assert!(matches!(object, lir::Value::Local(_)));
    assert!(matches!(
        value,
        lir::Value::IntegerConst(lir::LirIntegerConstant::Signed32(3))
    ));
}

#[test]
fn machine_state_fields_use_the_closed_heap_instruction_family() {
    let mut b = Builder::new();
    let kind = mir::MachineScalarKind::CoroutineFrameState;
    let state_ty = mir::Type::MachineScalar(kind);
    let frame = b.class(
        "Frame",
        None,
        &[("state", state_ty.clone())],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let object = locals.alloc(local("frame", mir::Type::Class(frame)));
    let observed = locals.alloc(local("observed", state_ty.clone()));
    let main = b.main(
        locals,
        vec![
            stmt(mir::StatementKind::FieldSet {
                object: local_expr(object, mir::Type::Class(frame)),
                index: 0,
                value: mir::Expr::machine_scalar(mir::MachineScalarValue::CoroutineFrameState(
                    mir::CoroutineFrameState::Initial,
                )),
            }),
            val_decl(
                observed,
                expr(
                    state_ty,
                    mir::ExprKind::FieldAccess {
                        receiver: Box::new(local_expr(object, mir::Type::Class(frame))),
                        index: 0,
                    },
                ),
            ),
        ],
    );
    let module = lower(b.finish(main));

    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::MachineHeapStore {
        kind: stored_kind,
        object: stored_object,
        offset: 16,
        value,
    } = instructions[0]
    else {
        panic!("a machine state field write must not use generic HeapStore")
    };
    assert_eq!(*stored_kind, lir::MachineScalarKind::CoroutineFrameState);
    assert!(matches!(stored_object, lir::Value::Local(_)));
    assert!(matches!(
        value,
        lir::Value::MachineScalar(lir::MachineScalarValue::CoroutineFrameState(
            lir::CoroutineFrameState::Initial
        ))
    ));

    let lir::Instruction::MachineHeapLoad {
        out,
        kind: loaded_kind,
        object: loaded_object,
        offset: 16,
    } = instructions[1]
    else {
        panic!("a machine state field read must not use generic HeapLoad")
    };
    assert_eq!(*loaded_kind, lir::MachineScalarKind::CoroutineFrameState);
    assert!(matches!(loaded_object, lir::Value::Local(_)));
    assert_eq!(
        function.temps[*out].ty,
        lir::LirType::MachineScalar(lir::MachineScalarKind::CoroutineFrameState)
    );
}

#[test]
#[should_panic(expected = "Retype operand must be a managed reference")]
fn retype_rejects_machine_scalar_to_source_integer() {
    let mut b = Builder::new();
    let machine_ty = mir::Type::MachineScalar(mir::MachineScalarKind::EnumTag);
    let mut locals = Arena::new();
    let machine = locals.alloc(local("machine", machine_ty.clone()));
    let integer = locals.alloc(local("integer", INT));
    let main = b.main(
        locals,
        vec![val_decl(
            integer,
            expr(
                INT,
                mir::ExprKind::Retype {
                    operand: Box::new(local_expr(machine, machine_ty)),
                    ty: Box::new(INT),
                },
            ),
        )],
    );

    let _ = lower(b.finish(main));
}

#[test]
fn a_trap_only_body_seals_the_function() {
    // mir-lower's abstract-method stub is a single trap terminator: the
    // block is sealed by the trap branch, so the "non-Unit
    // functions must end with `return`" check must not fire (it
    // applies to hir-lower-produced bodies that fall off the end,
    // not to noreturn bodies like this one).
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Any));
    let mut body = mir::Body::unreachable(locals);
    body.blocks[body.entry].terminator = mir::Terminator::Trap {
        message: String::from("call to abstract method `Base.id`"),
    };
    let _stub = b.user_fn_body(
        "Base.id",
        vec![param("this", mir::Type::Any, this)],
        INT,
        body,
    );
    let main = b.main(Arena::new(), vec![]);
    let module = lower(b.finish(main));

    let function = &module.functions[0];
    assert!(matches!(
        function.blocks[function.entry].terminator,
        lir::Terminator::Br(_)
    ));
}
