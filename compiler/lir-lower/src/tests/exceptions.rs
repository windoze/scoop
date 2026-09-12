use super::*;

/// `try { throw e } catch (e: MyError) { handled() }` minus the
/// throw — the shared shell of the M8 tests: `helper()` in the
/// body, `handled()` in the catch, `cleanup()` in the finally.
fn try_shell(finally: bool) -> (Builder, mir::FunctionId, mir::FunctionId, mir::FunctionId) {
    let mut b = Builder::new();
    let helper = b.user_fn("helper", "scoop.helper", Arena::new(), vec![]);
    let handled = b.user_fn("handled", "scoop.handled", Arena::new(), vec![]);
    let cleanup = if finally {
        b.user_fn("cleanup", "scoop.cleanup", Arena::new(), vec![])
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
        mir::ENTRY_SYMBOL,
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
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
  fun @scoop$1$cb$92f24139c6f5bb3d64abf748dba9ff6099323c3e8df704588a886e027f85e4ee() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
  fun @scoop$1$cb$bf6cfaf71a9a7fbc42690a582257c2f479b94a120f839f3af0395e5a8990d370() -> void
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
  td td0 MyError @scoop$1$td$eb205ad260a812589e9f030260657692c3e8a971a60e730337a3c28f28bc6cc9 type-id=1930812111026443540 size=16 parent=none vtable=[] itables=[]
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
  layout MyError size=16 align=8 refs=[]
  entry @scoop$1$cb$bf6cfaf71a9a7fbc42690a582257c2f479b94a120f839f3af0395e5a8990d370
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
        "scoop.callee",
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
        mir::ENTRY_SYMBOL,
        vec![
            param("both", reference_ty.clone(), live_on_both_edges),
            param("argumentOnly", reference_ty, argument_only),
        ],
        mir::Type::Unit,
        body,
    );
    let module = lower(&b.finish(main));
    let main = &module.functions[module.entry.declaration().into_u32() as usize];
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
