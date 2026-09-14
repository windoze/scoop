//! Finally execution on normal, catch, and early-return paths.

use super::*;

#[test]
fn finally_runs_on_the_normal_catch_and_rethrow_paths() {
    let (mut b, helper, handled, cleanup) = try_shell(true);
    let my_error = my_error(&mut b);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", mir::Type::Class(my_error)));
    let mut body = single_catch_body(
        locals,
        e,
        mir::Type::Class(my_error),
        vec![call_stmt(user_call(helper))],
        None,
        vec![call_stmt(user_call(handled))],
    );
    let try_body = cfg_block_named(&body, "try.body.8");
    let catch = cfg_block_named(&body, "try.catch.9");
    let next = cfg_block_named(&body, "try.next.10");
    let handler_cleanup = cfg_block_named(&body, "try.handler_cleanup.4");
    let exit_pad = cfg_block_named(&body, "try.exit_pad.5");
    let end = cfg_block_named(&body, "try.end.7");
    let normal_finally = cfg_block(&mut body.blocks, "scope.normal_finally");
    let catch_finally = cfg_block(&mut body.blocks, "scope.catch_finally");
    body.blocks[try_body].terminator = mir::Terminator::Goto(normal_finally);
    set_cfg_block(
        &mut body.blocks,
        normal_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Goto(end),
        None,
    );
    assert!(matches!(
        body.blocks[catch]
            .statements
            .pop()
            .map(|statement| statement.kind),
        Some(mir::StatementKind::Eh(mir::EhStatement::EndCatch))
    ));
    body.blocks[catch].terminator = mir::Terminator::Goto(catch_finally);
    set_cfg_block(
        &mut body.blocks,
        catch_finally,
        vec![
            stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            call_stmt(user_call(cleanup)),
        ],
        mir::Terminator::Goto(end),
        None,
    );
    body.blocks[next].statements = vec![call_stmt(user_call(cleanup))];
    body.blocks[next].unwind = Some(exit_pad);
    body.blocks[handler_cleanup]
        .statements
        .push(call_stmt(user_call(cleanup)));
    let main = b.user_fn_body("main", Vec::new(), mir::Type::Unit, body);
    let module = lower(b.finish(main));
    let main = module
        .executable_entry()
        .expect("test module is executable")
        .declaration()
        .into_u32() as usize;
    let dump = lir::dump(&module);

    // The finally body is inlined on normal completion, after the
    // catch body, on a catch-body exceptional exit, and before the
    // no-match rethrow.
    assert_eq!(dump.matches("local-fn2()").count(), 4);
    // The last copy is on the rethrow path, before the rethrow.
    let rethrow = dump
        .find("runtime @scoop_rt_rethrow()")
        .expect("a rethrow path");
    let last_cleanup = dump
        .rfind("local-fn2()")
        .expect("the rethrow path runs the finally");
    assert!(last_cleanup < rethrow);
    assert!(dump.contains("landingpad"));
    // A caught normal exit ends directly; catch-body exceptions
    // and no-match/rethrow exits have distinct cleanup pads.
    // Exactly one executes on each path.
    assert_eq!(
        module.functions[main]
            .blocks
            .iter()
            .flat_map(|(_, block)| &block.instructions)
            .filter(|instruction| matches!(instruction, lir::Instruction::EndCatch))
            .count(),
        3
    );
}

#[test]
fn return_inside_try_runs_finally_before_returning() {
    // fun f(): Int { try { return 1 } finally { cleanup() } }
    let (mut b, _, _, cleanup) = try_shell(true);
    let mut locals = Arena::new();
    let result = locals.alloc(local("$return.1", INT));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let unwind = cfg_block(&mut blocks, "try.unwind.1");
    let dispatch = cfg_block(&mut blocks, "try.dispatch.2");
    let exit_pad = cfg_block(&mut blocks, "try.exit_pad.3");
    let exit_cleanup = cfg_block(&mut blocks, "try.exit_cleanup.4");
    let end = cfg_block(&mut blocks, "try.end.5");
    let try_body = cfg_block(&mut blocks, "try.body.6");
    let return_finally = cfg_block(&mut blocks, "scope.7");
    let rethrow_finally = cfg_block(&mut blocks, "scope.8");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(try_body),
        None,
    );
    set_cfg_block(
        &mut blocks,
        unwind,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(dispatch),
        None,
    );
    set_cfg_block(
        &mut blocks,
        dispatch,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
        mir::Terminator::Goto(rethrow_finally),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        mir::Terminator::Goto(exit_cleanup),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Resume,
        None,
    );
    set_cfg_block(
        &mut blocks,
        end,
        Vec::new(),
        mir::Terminator::Unreachable,
        None,
    );
    set_cfg_block(
        &mut blocks,
        try_body,
        vec![val_decl(result, int_expr(1))],
        mir::Terminator::Goto(return_finally),
        Some(unwind),
    );
    set_cfg_block(
        &mut blocks,
        return_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Return {
            value: Some(local_expr(result, INT)),
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rethrow_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Rethrow {
            unwind: Some(exit_pad),
        },
        Some(exit_pad),
    );
    let f = b.user_fn_body(
        "f",
        Vec::new(),
        INT,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), vec![]);
    let module = lower(b.finish(main));

    // This body cannot enter the unwind path, so final LIR prunes that
    // detached copy together with the dead merge block. The reachable
    // finally copy still runs before the return.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
  fun @scoop$1$cb$92f24139c6f5bb3d64abf748dba9ff6099323c3e8df704588a886e027f85e4ee() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
  fun @scoop$1$cb$bf6cfaf71a9a7fbc42690a582257c2f479b94a120f839f3af0395e5a8990d370() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
  fun @scoop$1$cb$f944e169dc6c5f9c8e5ca489206ad78fabde6b6926cb0c8408d7e15d16eaa0bd() -> i32
    local %0 $return.1: i32
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr<raw>
    local %3 $sc.3: ptr<managed>
  block entry
    poll managed-void-target2 sp<managed-poll:0> live=[]
    br @try.body.6
  block try.body.6
    store integer<Int>(0x00000001) -> local0
    br @scope.7
  block scope.7
    call managed-void-target0 sp<managed-call:0> live=[] sig=void0 () local-fn2()
    t5 = aggregate () : {}
    ret local0
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn4() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td1 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
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
  layout String value size=8 align=8 refs=[0]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}
