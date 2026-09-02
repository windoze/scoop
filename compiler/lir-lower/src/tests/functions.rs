use super::*;

#[test]
fn function_signatures_params_and_calls() {
    let mut b = Builder::new();
    // fun add(x: Int, y: Int): Int { return x + y }
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", mir::Type::Int));
    let y = locals.alloc(local("y", mir::Type::Int));
    let add = b.user_fn_body(
        "add",
        "scoop.add",
        vec![param("x", mir::Type::Int, x), param("y", mir::Type::Int, y)],
        mir::Type::Int,
        returning_body(
            locals,
            binary(
                mir::BinOp::IntAdd,
                local_expr(x, mir::Type::Int),
                local_expr(y, mir::Type::Int),
                mir::Type::Int,
            ),
        ),
    );
    // main: val r = add(40, 2)
    let mut main_locals = Arena::new();
    let r = main_locals.alloc(local("r", mir::Type::Int));
    let main = b.main(
        main_locals,
        vec![call_value(
            r,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(add),
                },
                args: vec![mir::Expr::int(40), mir::Expr::int(2)],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // Parameters are SSA values (`Value::Param`), not stack slots;
    // the add body has no locals at all.
    let add_fn = &module.functions[0];
    assert_eq!(add_fn.params, [lir::LirType::I64, lir::LirType::I64]);
    assert_eq!(add_fn.return_ty, lir::LirType::I64);
    assert_eq!(add_fn.locals.len(), 0);

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop.add(i64, i64) -> i64
  block entry
    poll managed-void-target0 sp2 live=[]
    t0 = Add param0, param1 : i64
    ret t0
  fun @scoop_main() -> void
    local %0 r: i64
  block entry
    poll managed-void-target0 sp3 live=[]
    call managed-direct-target0 sp1 live=[] t0 = sig=direct0 (i64, i64) -> i64 local-fn0(40, 2)
    store t0 -> local0
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn return_inside_a_branch_seals_its_block() {
    // fun f(x: Int): Int { if (true) { return x }; return 0 }
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", mir::Type::Int));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let then_block = cfg_block(&mut blocks, "if.then.1");
    let merge = cfg_block(&mut blocks, "if.merge.2");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Branch {
            cond: mir::Expr::bool(true),
            then_block,
            else_block: merge,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then_block,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(local_expr(x, mir::Type::Int)),
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(mir::Expr::int(0)),
        },
        None,
    );
    let f = b.user_fn_body(
        "f",
        "scoop.f",
        vec![param("x", mir::Type::Int, x)],
        mir::Type::Int,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    // The constant branch is folded and its unreachable merge path is
    // removed; the `return` seals the remaining then block.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop.f(i64) -> i64
  block entry
    poll managed-void-target0 sp1 live=[]
    br @if.then.1
  block if.then.1
    ret param0
  fun @scoop_main() -> void
  block entry
    poll managed-void-target0 sp2 live=[]
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}
