use super::*;

#[test]
fn if_else_becomes_basic_blocks() {
    let mut b = Builder::new();
    let ok = b.string("ok");
    let ng = b.string("ng");
    let write = b.managed_scoop_extern(
        "write",
        "scoop_rt_write",
        vec![mir::Type::String],
        mir::Type::Unit,
    );
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let then_block = cfg_block(&mut blocks, "if.then.1");
    let else_block = cfg_block(&mut blocks, "if.else.2");
    let merge = cfg_block(&mut blocks, "if.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Branch {
            cond: mir::Expr::bool(true),
            then_block,
            else_block,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then_block,
        vec![call_stmt(extern_call(write, vec![string_expr(ok)]))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        else_block,
        vec![call_stmt(extern_call(write, vec![string_expr(ng)]))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals: Arena::new(),
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "ok"
  global @scoop.str.1 = "ng"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> void <scoop managed nounwind>
  fun @scoop_main() -> void
  block entry
    poll managed-void-target0 sp3 live=[]
    br @if.then.1
  block if.then.1
    call native-borrowed-void-target0 sp1 roots=[] sig=void0 (ptr<managed>) extern0(global0)
    t0 = aggregate () : {}
    br @if.merge.3
  block if.merge.3
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
fn while_becomes_basic_blocks() {
    // var n = 0; while (n < 3) { n = n + 1 }
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let n = locals.alloc(var("n", INT));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let cond = cfg_block(&mut blocks, "while.cond.1");
    let body = cfg_block(&mut blocks, "while.body.2");
    let exit = cfg_block(&mut blocks, "while.exit.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![val_decl(n, int_expr(0))],
        mir::Terminator::Goto(cond),
        None,
    );
    set_cfg_block(
        &mut blocks,
        cond,
        Vec::new(),
        mir::Terminator::Branch {
            cond: integer_compare_expr(
                mir::IntegerKind::SIGNED_32,
                mir::IntegerComparisonOperator::LessThan,
                local_expr(n, INT),
                int_expr(3),
            ),
            then_block: body,
            else_block: exit,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        body,
        vec![assign(
            n,
            integer_binary_expr(
                mir::IntegerKind::SIGNED_32,
                mir::IntegerBinaryOperator::Add,
                local_expr(n, INT),
                int_expr(1),
            ),
        )],
        mir::Terminator::Goto(cond),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop_main() -> void
    local %0 n: i32
  block entry
    poll managed-void-target0 sp1 live=[]
    store integer<Int>(0x00000000) -> local0
    br @while.cond.1
  block while.cond.1
    poll managed-void-target0 sp2 live=[]
    t0 = integer_compare_Less<Int> local0, integer<Int>(0x00000003) : i1
    cbr t0 then @while.body.2 else @while.exit.3
  block while.body.2
    t1 = integer_Add<Int> local0, integer<Int>(0x00000001) : i32
    store t1 -> local0
    br @while.cond.1
  block while.exit.3
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
fn and_short_circuits_through_blocks() {
    // val b = eq(s0, s1) && eq(s2, s3): the second comparison call
    // sits in its own block, executed only when the first is true.
    let mut b = Builder::new();
    let s0 = b.string("a");
    let s1 = b.string("b");
    let s2 = b.string("c");
    let s3 = b.string("d");
    let string_eq = b.managed_scoop_extern(
        "coreStringEquals",
        "scoop_rt_string_eq",
        vec![mir::Type::String, mir::Type::String],
        mir::Type::Boolean,
    );
    let string_eq = |l, r| extern_call(string_eq, vec![string_expr(l), string_expr(r)]);
    let mut locals = Arena::new();
    let lhs = locals.alloc(local("$call.1", mir::Type::Boolean));
    let rhs = locals.alloc(local("$call.2", mir::Type::Boolean));
    let result = locals.alloc(local("b", mir::Type::Boolean));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let rhs_block = cfg_block(&mut blocks, "logic.rhs.1");
    let short_block = cfg_block(&mut blocks, "logic.short.2");
    let merge = cfg_block(&mut blocks, "logic.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![call_value(lhs, string_eq(s0, s1))],
        mir::Terminator::Branch {
            cond: local_expr(lhs, mir::Type::Boolean),
            then_block: rhs_block,
            else_block: short_block,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rhs_block,
        vec![
            call_value(rhs, string_eq(s2, s3)),
            assign(result, local_expr(rhs, mir::Type::Boolean)),
        ],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        short_block,
        vec![assign(result, mir::Expr::bool(false))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "a"
  global @scoop.str.1 = "b"
  global @scoop.str.2 = "c"
  global @scoop.str.3 = "d"
  extern ef0 coreStringEquals @scoop_rt_string_eq(ptr<managed>, ptr<managed>) -> i1 <scoop managed nounwind>
  fun @scoop_main() -> void
    local %0 $call.1: i1
    local %1 $call.2: i1
    local %2 b: i1
  block entry
    poll managed-void-target0 sp3 live=[]
    call native-borrowed-direct-target0 sp1 roots=[] t0 = sig=direct0 (ptr<managed>, ptr<managed>) -> i1 extern0(global0, global1)
    store t0 -> local0
    cbr local0 then @logic.rhs.1 else @logic.short.2
  block logic.rhs.1
    call native-borrowed-direct-target1 sp2 roots=[] t1 = sig=direct1 (ptr<managed>, ptr<managed>) -> i1 extern0(global2, global3)
    store t1 -> local1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store false -> local2
    br @logic.merge.3
  block logic.merge.3
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
fn or_short_circuits_through_blocks() {
    // val b = x || y: when x is true, y is never evaluated.
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", mir::Type::Boolean));
    let y = locals.alloc(local("y", mir::Type::Boolean));
    let result = locals.alloc(local("b", mir::Type::Boolean));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let rhs = cfg_block(&mut blocks, "logic.rhs.1");
    let short = cfg_block(&mut blocks, "logic.short.2");
    let merge = cfg_block(&mut blocks, "logic.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![
            val_decl(x, mir::Expr::bool(true)),
            val_decl(y, mir::Expr::bool(false)),
        ],
        mir::Terminator::Branch {
            cond: local_expr(x, mir::Type::Boolean),
            then_block: short,
            else_block: rhs,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rhs,
        vec![assign(result, local_expr(y, mir::Type::Boolean))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        short,
        vec![assign(result, mir::Expr::bool(true))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop_main() -> void
    local %0 x: i1
    local %1 y: i1
    local %2 b: i1
  block entry
    poll managed-void-target0 sp1 live=[]
    store true -> local0
    store false -> local1
    cbr local0 then @logic.short.2 else @logic.rhs.1
  block logic.rhs.1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store true -> local2
    br @logic.merge.3
  block logic.merge.3
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
