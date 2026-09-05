use super::*;

#[test]
fn function_signatures_params_and_calls() {
    let mut b = Builder::new();
    // fun add(x: Int, y: Int): Int { return x + y }
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", INT));
    let y = locals.alloc(local("y", INT));
    let add = b.user_fn_body(
        "add",
        "scoop.add",
        vec![param("x", INT, x), param("y", INT, y)],
        INT,
        returning_body(
            locals,
            integer_binary_expr(
                mir::IntegerKind::SIGNED_32,
                mir::IntegerBinaryOperator::Add,
                local_expr(x, INT),
                local_expr(y, INT),
            ),
        ),
    );
    // main: val r = add(40, 2)
    let mut main_locals = Arena::new();
    let r = main_locals.alloc(local("r", INT));
    let main = b.main(
        main_locals,
        vec![call_value(
            r,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(add),
                },
                args: vec![int_expr(40), int_expr(2)],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // Parameters are SSA values (`Value::Param`), not stack slots;
    // the add body has no locals at all.
    let add_fn = &module.functions[0];
    assert_eq!(add_fn.params, [lir::LirType::I32, lir::LirType::I32]);
    assert_eq!(add_fn.return_ty, lir::LirType::I32);
    assert_eq!(add_fn.locals.len(), 0);

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop.add(i32, i32) -> i32
  block entry
    poll managed-void-target0 sp2 live=[]
    t0 = integer_Add<Int> param0, param1 : i32
    ret t0
  fun @scoop_main() -> void
    local %0 r: i32
  block entry
    poll managed-void-target0 sp3 live=[]
    call managed-direct-target0 sp1 live=[] t0 = sig=direct0 (i32, i32) -> i32 local-fn0(integer<Int>(0x00000028), integer<Int>(0x00000002))
    store t0 -> local0
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
  entry @scoop_main
"###);
}

#[test]
fn c_extern_arguments_keep_their_exact_backing_storage_in_lir() {
    let mut b = Builder::new();
    let int8 = mir::Type::Integer(mir::IntegerKind::SIGNED_8);
    let consume = b.c_extern(
        "consumeInt8",
        "native_consume_int8",
        vec![int8.clone()],
        mir::Type::Unit,
    );
    let main = b.main(
        Arena::new(),
        vec![call_stmt(extern_call(
            consume,
            vec![integer_expr(mir::IntegerKind::SIGNED_8, 7)],
        ))],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let local = instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::Store { local, .. } => Some(*local),
            _ => None,
        })
        .expect("C argument is stored in one exact typed local");
    let arguments = instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::Call {
                site: lir::CallSite::NativeSafe(site),
            } => Some(site.call.args()),
            _ => None,
        })
        .expect("C extern uses the native-safe protocol");
    assert_eq!(
        arguments,
        [lir::Value::CArgumentStorage(
            lir::CArgumentStorage::address_of(local)
        )]
    );
    assert_eq!(function.locals[local].ty, lir::LirType::I8);
    assert!(lir::dump(&module).contains("extern0(c-arg-address(local0))"));
    assert!(
        !instructions
            .iter()
            .any(|instruction| matches!(instruction, lir::Instruction::LocalAddress { .. }))
    );
}

#[test]
fn return_inside_a_branch_seals_its_block() {
    // fun f(x: Int): Int { if (true) { return x }; return 0 }
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", INT));
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
            value: Some(local_expr(x, INT)),
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(int_expr(0)),
        },
        None,
    );
    let f = b.user_fn_body(
        "f",
        "scoop.f",
        vec![param("x", INT, x)],
        INT,
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
  fun @scoop.f(i32) -> i32
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
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}
