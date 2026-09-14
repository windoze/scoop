use super::*;

/// `try { throw e } catch (e: MyError) { handled() }` minus the
/// throw — the shared shell of the M8 tests: `helper()` in the
/// body, `handled()` in the catch, `cleanup()` in the finally.
fn try_shell(finally: bool) -> (Builder, mir::FunctionId, mir::FunctionId, mir::FunctionId) {
    let mut b = Builder::new();
    let helper = b.user_fn("helper", Arena::new(), vec![]);
    let handled = b.user_fn("handled", Arena::new(), vec![]);
    let cleanup = if finally {
        b.user_fn("cleanup", Arena::new(), vec![])
    } else {
        helper
    };
    (b, helper, handled, cleanup)
}

fn my_error(b: &mut Builder) -> mir::ClassId {
    b.class("MyError", None, &[], vec![], vec![])
}

#[test]
fn try_catch_lowers_to_invoke_landingpad_and_rethrow() {
    let (mut b, helper, handled, _) = try_shell(false);
    let my_error = my_error(&mut b);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", mir::Type::Class(my_error)));
    let main = b.user_fn_body(
        "main",
        Vec::new(),
        mir::Type::Unit,
        single_catch_body(
            locals,
            e,
            mir::Type::Class(my_error),
            vec![call_stmt(user_call(helper))],
            None,
            vec![call_stmt(user_call(handled))],
        ),
    );
    let module = lower(b.finish(main));

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
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 e: ptr<managed>
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr<raw>
    local %3 $sc.3: ptr<managed>
  block entry
    poll managed-void-target2 sp<managed-poll:0> live=[]
    br @try.body.8
  block try.unwind.1
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    store t0 -> local1
    store t1 -> local2
    br @try.dispatch.2
  block try.dispatch.2
    t2 = begin_catch local2 : ptr<managed>
    store t2 -> local3
    call no-gc-direct-target0 t3 = sig=direct0 (ptr<managed>, ptr<metadata>) -> i1 runtime @scoop_rt_is_instance(local3, td0)
    cbr t3 then @try.catch.9 else @try.next.10
  block try.handler_pad.3
    (t4, t5) = cleanup_pad : (exception_record, ptr<raw>)
    store t4 -> local1
    store t5 -> local2
    br @try.handler_cleanup.4
  block try.handler_cleanup.4
    end_catch
    resume local1
  block try.exit_pad.5
    (t6, t7) = cleanup_pad : (exception_record, ptr<raw>)
    store t6 -> local1
    store t7 -> local2
    br @try.exit_cleanup.6
  block try.exit_cleanup.6
    end_catch
    resume local1
  block try.end.7
    ret
  block try.body.8
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn0() normal @invoke.normal.1 unwind @try.unwind.1
    br @invoke.normal.1
  block try.catch.9
    store local3 -> local0
    invoke managed-void-target1 sp<managed-invoke:1> roots=[] sig=void1 () local-fn1() normal @invoke.normal.2 unwind @try.handler_pad.3
    br @invoke.normal.2
  block try.next.10
    invoke no-gc-void-target0 sig=void2 () runtime @scoop_rt_rethrow() normal @rethrow.normal.3 unwind @try.exit_pad.5
    br @rethrow.normal.3
  block invoke.normal.1
    t8 = aggregate () : {}
    br @try.end.7
  block invoke.normal.2
    t9 = aggregate () : {}
    end_catch
    br @try.end.7
  block rethrow.normal.3
    unreachable
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn2() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td0 MyError @scoop$1$td$eb205ad260a812589e9f030260657692c3e8a971a60e730337a3c28f28bc6cc9 type-id=1930812111026443540 shape=FixedObject minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td2 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td3 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td10 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td11 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
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
  layout MyError value size=8 align=8 refs=[0]
  layout MyError size=16 align=8 refs=[]
  layout String value size=8 align=8 refs=[0]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}

#[test]
fn managed_invoke_roots_have_complete_edge_roles_and_argument_coverage() {
    let mut b = Builder::new();
    let error_class = my_error(&mut b);
    let reference_ty = mir::Type::Class(error_class);

    let mut callee_locals = Arena::new();
    let callee_first = callee_locals.alloc(local("first", reference_ty.clone()));
    let callee_second = callee_locals.alloc(local("second", reference_ty.clone()));
    let callee = b.user_fn_body(
        "callee",
        vec![
            param("first", reference_ty.clone(), callee_first),
            param("second", reference_ty.clone(), callee_second),
        ],
        mir::Type::Unit,
        body_with_terminator(
            callee_locals,
            Vec::new(),
            mir::Terminator::Return { value: None },
        ),
    );

    let mut locals = Arena::new();
    let live_on_both_edges = locals.alloc(local("both", reference_ty.clone()));
    let argument_only = locals.alloc(local("argumentOnly", reference_ty.clone()));
    let caught = locals.alloc(local("caught", reference_ty.clone()));
    let sink = locals.alloc(local("sink", reference_ty.clone()));
    let call = mir::Call {
        target: mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: mir::Callee::User(callee),
        },
        args: vec![
            local_expr(live_on_both_edges, reference_ty.clone()),
            local_expr(argument_only, reference_ty.clone()),
        ],
        pending: mir::CoroutinePendingContext::Root,
    };
    let body = single_catch_body(
        locals,
        caught,
        reference_ty.clone(),
        vec![
            call_stmt(call),
            assign(sink, local_expr(live_on_both_edges, reference_ty.clone())),
        ],
        None,
        vec![assign(
            sink,
            local_expr(live_on_both_edges, reference_ty.clone()),
        )],
    );
    let main = b.user_fn_body(
        "main",
        vec![
            param("both", reference_ty.clone(), live_on_both_edges),
            param("argumentOnly", reference_ty, argument_only),
        ],
        mir::Type::Unit,
        body,
    );
    let module = lower(b.finish(main));
    let main = module
        .functions
        .iter()
        .find(|function| {
            function.blocks.iter().any(|(_, block)| {
                block
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, lir::Instruction::Invoke { .. }))
            })
        })
        .expect("test module contains the invoking function");
    let roots = main
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.instructions)
        .find_map(|instruction| match instruction {
            lir::Instruction::Invoke {
                site: lir::InvokeSite::Managed(site),
            } => Some(site.roots.as_slice()),
            _ => None,
        })
        .expect("managed invoke");

    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].root.source, lir::CallerRootSource::Param(0));
    assert!(roots[0].normal_live);
    assert!(roots[0].unwind_live);
    assert_eq!(roots[1].root.source, lir::CallerRootSource::Param(1));
    assert!(!roots[1].normal_live);
    assert!(!roots[1].unwind_live);
}

mod finally;
mod unwind;
