use super::*;

#[test]
fn trap_calls_branch_to_a_shared_trap_block() {
    // fun f(o: Option<Int>): Int { return o!! + o!! } — in the
    // mir-lower shape: each `o!!` is `if (tag == Some) { val $uw =
    // field0 } else { trap(msg) }`.
    let mut b = Builder::new();
    let option_i = b.option_enum("Option$I", INT);
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
        "scoop.f",
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
    let module = lower(&b.finish(main));

    // Both `!!` share the one trap block of the function.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$io$628de209327518e6dd1b8cb671b0800d34d8c4a09fd4dafae1ff244dfb49e582 = "unwrap on None (function f)"
  global @scoop.cstr.0 = c"unwrap on None (function f)"
  enum Option$I tagged size=16 align=8 variants=(i32)@8+4 ()@8+0
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
    call no-gc-void-target0 sig=void0 (ptr<raw>) runtime @scoop_rt_trap(global1)
    unreachable
  fun @scoop$1$cb$92f24139c6f5bb3d64abf748dba9ff6099323c3e8df704588a886e027f85e4ee() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
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
  layout Option$I size=16 align=8 enum-scan=none
  entry @scoop$1$cb$92f24139c6f5bb3d64abf748dba9ff6099323c3e8df704588a886e027f85e4ee
"###);
}
