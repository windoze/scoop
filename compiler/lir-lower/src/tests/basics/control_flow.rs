use super::*;

fn managed_poll_block_names(function: &lir::Function) -> Vec<&str> {
    function
        .blocks
        .values()
        .filter(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, lir::Instruction::ManagedPoll { .. }))
        })
        .map(|block| block.name.as_str())
        .collect()
}

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
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals: Arena::new(),
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    );
    let module = lower(b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$io$628de209327518e6dd1b8cb671b0800d34d8c4a09fd4dafae1ff244dfb49e582 = "ok"
  global @scoop$1$io$6389e5e8389d22f0e2baac5ee54d46413239a4323769000ee665c277f1d369ec = "ng"
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  extern ef0 write @scoop_rt_write(ptr<managed>) -> void <scoop managed nounwind>
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    br @if.then.1
  block if.then.1
    call native-borrowed-void-target0 sp<native-borrowed:0> roots=[] sig=void0 (ptr<managed>) extern0(global0)
    t0 = aggregate () : {}
    br @if.merge.3
  block if.merge.3
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
    global_store global2, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td0 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
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
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: vec![mir::LoopHeaderPollTarget::new(cond)],
        },
    );
    let module = lower(b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 n: i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    store integer<Int>(0x00000000) -> local0
    br @while.cond.1
  block while.cond.1
    poll managed-void-target0 sp<managed-poll:1> live=[]
    t0 = integer_compare_Less<Int> local0, integer<Int>(0x00000003) : i1
    cbr t0 then @while.body.2 else @while.exit.3
  block while.body.2
    t1 = integer_Add<Int> local0, integer<Int>(0x00000001) : i32
    store t1 -> local0
    br @while.cond.1
  block while.exit.3
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
  td td0 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
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
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    );
    let module = lower(b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$io$628de209327518e6dd1b8cb671b0800d34d8c4a09fd4dafae1ff244dfb49e582 = "a"
  global @scoop$1$io$6389e5e8389d22f0e2baac5ee54d46413239a4323769000ee665c277f1d369ec = "b"
  global @scoop$1$io$c65f5ca6a0fa7e88edfdc899205b08b99869e7e0201f6abfc7beb0a4272e7367 = "c"
  global @scoop$1$io$9b97af2e165f9e07e3561b053a25d214a03b3373b11117b29503110b70fb23ea = "d"
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  extern ef0 coreStringEquals @scoop_rt_string_eq(ptr<managed>, ptr<managed>) -> i1 <scoop managed nounwind>
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 $call.1: i1
    local %1 $call.2: i1
    local %2 b: i1
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    call native-borrowed-direct-target0 sp<native-borrowed:0> roots=[] t0 = sig=direct0 (ptr<managed>, ptr<managed>) -> i1 extern0(global0, global1)
    store t0 -> local0
    cbr local0 then @logic.rhs.1 else @logic.short.2
  block logic.rhs.1
    call native-borrowed-direct-target1 sp<native-borrowed:1> roots=[] t1 = sig=direct1 (ptr<managed>, ptr<managed>) -> i1 extern0(global2, global3)
    store t1 -> local1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store false -> local2
    br @logic.merge.3
  block logic.merge.3
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
    global_store global4, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td0 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
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
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    );
    let module = lower(b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 x: i1
    local %1 y: i1
    local %2 b: i1
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
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
  td td0 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}

#[test]
fn explicit_loop_header_poll_survives_coroutine_like_multi_entry_cfg_and_pruning() {
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let choice = locals.alloc(local("choice", mir::Type::Boolean));
    let params = vec![mir::Param {
        name: "choice".to_string(),
        ty: mir::Type::Boolean,
        local: choice,
    }];
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let detached = cfg_block(&mut blocks, "detached");
    let dispatch = cfg_block(&mut blocks, "coroutine.dispatch");
    let header = cfg_block(&mut blocks, "while.cond");
    let body = cfg_block(&mut blocks, "while.body");
    let resume = cfg_block(&mut blocks, "coroutine.resume");
    let post = cfg_block(&mut blocks, "coroutine.post");
    let exit = cfg_block(&mut blocks, "while.exit");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(dispatch),
        None,
    );
    set_cfg_block(
        &mut blocks,
        dispatch,
        Vec::new(),
        mir::Terminator::Branch {
            cond: local_expr(choice, mir::Type::Boolean),
            then_block: header,
            else_block: resume,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        header,
        Vec::new(),
        mir::Terminator::Goto(body),
        None,
    );
    set_cfg_block(
        &mut blocks,
        body,
        Vec::new(),
        mir::Terminator::Goto(post),
        None,
    );
    set_cfg_block(
        &mut blocks,
        resume,
        Vec::new(),
        mir::Terminator::Goto(post),
        None,
    );
    set_cfg_block(
        &mut blocks,
        post,
        Vec::new(),
        mir::Terminator::Branch {
            cond: local_expr(choice, mir::Type::Boolean),
            then_block: header,
            else_block: exit,
        },
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
        params,
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: vec![
                mir::LoopHeaderPollTarget::new(entry),
                mir::LoopHeaderPollTarget::new(detached),
                mir::LoopHeaderPollTarget::new(header),
            ],
        },
    );

    let module = lower(b.finish(main));
    assert_eq!(
        managed_poll_block_names(&module.functions[0]),
        vec!["entry", "while.cond"]
    );
}

#[test]
fn loop_header_poll_carries_live_managed_root() {
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let keep_looping = locals.alloc(local("keepLooping", mir::Type::Boolean));
    let root = locals.alloc(local("root", mir::Type::String));
    let params = vec![
        mir::Param {
            name: "keepLooping".to_string(),
            ty: mir::Type::Boolean,
            local: keep_looping,
        },
        mir::Param {
            name: "root".to_string(),
            ty: mir::Type::String,
            local: root,
        },
    ];
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let header = cfg_block(&mut blocks, "while.cond");
    let body = cfg_block(&mut blocks, "while.body");
    let exit = cfg_block(&mut blocks, "while.exit");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(header),
        None,
    );
    set_cfg_block(
        &mut blocks,
        header,
        Vec::new(),
        mir::Terminator::Branch {
            cond: local_expr(keep_looping, mir::Type::Boolean),
            then_block: body,
            else_block: exit,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        body,
        Vec::new(),
        mir::Terminator::Goto(header),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(local_expr(root, mir::Type::String)),
        },
        None,
    );
    let function = b.user_fn_body(
        "loopLiveRoot",
        params,
        mir::Type::String,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: vec![mir::LoopHeaderPollTarget::new(header)],
        },
    );

    let module = lower(b.finish(function));
    lir::CanonicalLirFoundation::from_module(&module)
        .expect("lowered callable and safepoint identities must project canonically");
    let function = &module.functions[0];
    let header = function
        .blocks
        .values()
        .find(|block| block.name == "while.cond")
        .expect("loop header");
    let site = header
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::ManagedPoll { site } => Some(site),
            _ => None,
        })
        .expect("loop header poll");
    let live = site.live.as_slice();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].source, lir::CallerRootSource::Param(1));
    assert_eq!(live[0].ty, lir::MANAGED_PTR);
    assert_eq!(
        live[0].leaves.as_slice(),
        &[lir::ManagedLeafPath { byte_offset: 0 }]
    );
}

#[test]
fn unmarked_cycle_does_not_synthesize_a_loop_header_poll() {
    let mut b = Builder::new();
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let cycle = cfg_block(&mut blocks, "cycle");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(cycle),
        None,
    );
    set_cfg_block(
        &mut blocks,
        cycle,
        Vec::new(),
        mir::Terminator::Goto(cycle),
        None,
    );
    let main = b.user_fn_body(
        "main",
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals: Arena::new(),
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    );

    let module = lower(b.finish(main));
    assert_eq!(
        managed_poll_block_names(&module.functions[0]),
        vec!["entry"]
    );
}

#[test]
fn no_gc_function_ignores_explicit_loop_header_poll_targets() {
    let mut b = Builder::new();
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let cycle = cfg_block(&mut blocks, "cycle");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(cycle),
        None,
    );
    set_cfg_block(
        &mut blocks,
        cycle,
        Vec::new(),
        mir::Terminator::Goto(cycle),
        None,
    );
    let main = b.user_fn_body(
        "main",
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals: Arena::new(),
            blocks,
            entry,
            loop_header_polls: vec![mir::LoopHeaderPollTarget::new(cycle)],
        },
    );
    b.functions[main].gc_effect = mir::GcEffect::NoGc;

    let module = lower(b.finish(main));
    assert!(managed_poll_block_names(&module.functions[0]).is_empty());
}
