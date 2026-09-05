use super::*;

#[test]
fn trap_calls_branch_to_a_shared_trap_block() {
    // fun f(o: Option<Int>): Int { return o!! + o!! } — in the
    // mir-lower shape: each `o!!` is `if (tag == Some) { val $uw =
    // field0 } else { trap(msg) }`.
    let mut b = Builder::new();
    let option_i = b.option_enum("Option$I", mir::Type::Int);
    let option_ty = mir::Type::Enum(option_i, vec![mir::Type::Int]);
    let message = b.string("unwrap on None (function f)");
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_ty.clone()));
    let uw1 = locals.alloc(local("$uw.1", mir::Type::Int));
    let uw2 = locals.alloc(local("$uw.2", mir::Type::Int));
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
                mir::Type::Int,
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
            value: Some(binary(
                mir::BinOp::IntAdd,
                local_expr(uw1, mir::Type::Int),
                local_expr(uw2, mir::Type::Int),
                mir::Type::Int,
            )),
        },
        None,
    );
    let f = b.user_fn_body(
        "f",
        "scoop.f",
        vec![param("o", option_ty, o)],
        mir::Type::Int,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), Vec::new());
    let module = lower(&b.finish(main));

    // Both `!!` share the one trap block of the function.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "unwrap on None (function f)"
  global @scoop.cstr.0 = c"unwrap on None (function f)"
  enum Option$I tagged size=16 align=8 variants=(i64)@8+8 ()@8+0
  fun @scoop.f(enum0) -> i64
    local %0 $uw.1: i64
    local %1 $uw.2: i64
  block entry
    poll managed-void-target0 sp1 live=[]
    t0 = enum_tag e0 param0 : machine<enum-tag>
    t1 = MachineEq(EnumTag) t0, machine<enum-tag>(EnumTag(0)) : i1
    cbr t1 then @if.then.1 else @if.else.2
  block if.then.1
    t2 = enum_field e0 v0 f0 param0 : i64
    store t2 -> local0
    br @if.merge.3
  block if.else.2
    br @unwrap.trap.1
  block if.merge.3
    t3 = enum_tag e0 param0 : machine<enum-tag>
    t4 = MachineEq(EnumTag) t3, machine<enum-tag>(EnumTag(0)) : i1
    cbr t4 then @if.then.5 else @if.else.6
  block if.then.5
    t5 = enum_field e0 v0 f0 param0 : i64
    store t5 -> local1
    br @if.merge.7
  block if.else.6
    br @unwrap.trap.1
  block if.merge.7
    t6 = Add local0, local1 : i64
    ret t6
  block unwrap.trap.1
    call no-gc-void-target0 sig=void0 (ptr<raw>) runtime @scoop_rt_trap(global1)
    unreachable
  fun @scoop_main() -> void
  block entry
    poll managed-void-target0 sp2 live=[]
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$I size=16 align=8 enum-scan=none
  entry @scoop_main
"###);
}
