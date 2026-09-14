use super::*;

#[test]
fn reachable_structural_function_descriptor_reports_strong_capability_error() {
    let mut b = Builder::new();
    let function_type = b.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    let mut locals = Arena::new();
    let callable = locals.alloc(local("callable", mir::Type::Any));
    let main = b.main(
        locals,
        vec![call_stmt(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::FunctionBridge { function_type },
                callee: mir::Callee::FunctionBridge(function_type),
            },
            args: vec![local_expr(callable, mir::Type::Any)],
            pending: mir::CoroutinePendingContext::Root,
        })],
    );
    let mut source = b.finish(main);
    register_test_source_exact_type(&mut source, mir::Type::Function(function_type));

    let error = match try_lower(source) {
        Err(error) => error,
        Ok(_) => panic!("reachable function bridge must fail strong LIR capability validation"),
    };
    assert!(
        error
            .to_string()
            .starts_with(StrongLirCapabilityError::CODE)
    );
    match error {
        StrongLirLoweringError::Capability(error) => {
            assert_eq!(error.function(), main);
            assert_eq!(
                error.requirement(),
                &StrongLirMaterializationRequirement::TypeDescriptor(mir::Type::Function(
                    function_type
                ))
            );
        }
        StrongLirLoweringError::Foundation(_) => {
            panic!("capability validation must run before LIR foundation projection")
        }
    }
}

#[test]
fn function_signatures_params_and_calls() {
    let mut b = Builder::new();
    // fun add(x: Int, y: Int): Int { return x + y }
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", INT));
    let y = locals.alloc(local("y", INT));
    let add = b.user_fn_body(
        "add",
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
                pending: mir::CoroutinePendingContext::Root,
            },
        )],
    );
    let module = lower(b.finish(main));

    // Parameters are SSA values (`Value::Param`), not stack slots;
    // the add body has no locals at all.
    let add_fn = &module.functions[0];
    assert_eq!(
        add_fn
            .signature
            .arguments()
            .iter()
            .map(lir::AbiArgument::logical_storage_type)
            .collect::<Vec<_>>(),
        [&lir::LirType::I32, &lir::LirType::I32]
    );
    assert_eq!(
        add_fn.signature.result().logical_storage_type(),
        Some(&lir::LirType::I32)
    );
    assert_eq!(add_fn.locals.len(), 0);

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e(i32, i32) -> i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    t0 = integer_Add<Int> param0, param1 : i32
    ret t0
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 r: i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    call managed-direct-target0 sp<managed-call:0> live=[] t0 = sig=direct0 (i32, i32) -> i32 local-fn0(integer<Int>(0x00000028), integer<Int>(0x00000002))
    store t0 -> local0
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn1() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
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
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
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
    let module = lower(b.finish(main));

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
        [lir::AbiCallArgument::Direct(lir::Value::CArgumentStorage(
            lir::CArgumentStorage::address_of(local)
        ))]
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
        vec![param("x", INT, x)],
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

    // The constant branch is folded and its unreachable merge path is
    // removed; the `return` seals the remaining then block.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e(i32) -> i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    br @if.then.1
  block if.then.1
    ret param0
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn1() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
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
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}
