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
        "scoop.init.Point.$c0",
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
    let module = lower(&b.finish(main));

    // `scoop_rt_alloc(td, size)` with the class layout size (16
    // header + Int @16 + String @24 = 32), then the fields at
    // those byte offsets.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "x"
  fun @scoop.init.Point.$c0(ptr<managed>, i32, ptr<managed>) -> void
  block entry
    poll managed-void-target0 sp3 live=[param0:ptr<managed>@0, param2:ptr<managed>@0]
    heap_store param0 +16 param1
    heap_store param0 +24 param2
    ret
  fun @scoop_main() -> void
    local %0 p: ptr<managed>
  block entry
    poll managed-void-target1 sp4 live=[]
    call managed-direct-target0 sp1 live=[] t0 = sig=direct0 (ptr<metadata>, machine<byte-size>) -> ptr<managed> runtime @scoop_rt_alloc(td0, machine<byte-size>(ByteSize(32)))
    store t0 -> local0
    call managed-void-target0 sp2 live=[local0:ptr<managed>@0] sig=void0 (ptr<managed>, i32, ptr<managed>) local-fn0(local0, integer<Int>(0x00000001), global0)
    t1 = aggregate () : {}
    ret
  td td0 Point @scoop_td_C5_PointX type-id=2 size=32 parent=none vtable=[] itables=[]
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
  layout Point size=32 align=8 refs=[24]
  entry @scoop_main
"###);
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
    let module = lower(&b.finish(main));

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
    let module = lower(&b.finish(main));

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

    let _ = lower(&b.finish(main));
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
        INT,
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
