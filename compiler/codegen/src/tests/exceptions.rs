use super::*;

/// An M8-shaped module (runtime spec 5): Invoke / InvokeIndirect
/// sharing one catch-all landing pad, plus the cleanup-pad /
/// EndCatch / Resume shape used for exceptional handler exits.
pub(super) fn exceptions_module() -> Module {
    // fun @scoop.thrower(e: ptr) -> void: the rethrow shape — Throw
    // as the last instruction; the Unreachable terminator emits the
    // LLVM `unreachable` after the noreturn runtime call.
    let mut thrower_blocks = Arena::default();
    let thrower_entry = thrower_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Throw {
            exception: Value::Param(0),
        }],
        terminator: Terminator::Unreachable,
    });
    let thrower = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.thrower".to_string(),
        params: vec![MANAGED_PTR],
        return_ty: LirType::Void,
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: thrower_blocks,
        entry: thrower_entry,
    };

    let mut may_throw_blocks = Arena::default();
    let may_throw_entry = may_throw_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(signed64(1)),
        },
    });
    let may_throw = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.may_throw".to_string(),
        params: Vec::new(),
        return_ty: LirType::I64,
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: may_throw_blocks,
        entry: may_throw_entry,
    };

    // fun @scoop.eh_test(table: ptr) -> i64:
    //   entry:  t0 = invoke_indirect table[0]() normal @normal unwind @lpad
    //   normal: t1 = invoke @scoop.may_throw() normal @done unwind @lpad
    //   done:   t2 = t0 + t1; ret t2
    //   lpad:   t3 = landingpad; t5 = begin_catch(t3.raw)
    //           t8 = invoke @scoop.may_throw() normal @handler_done unwind @cleanup
    //   handler_done: end_catch; ret 0
    //   cleanup: t4 = cleanup_pad; end_catch; resume t4
    let mut temps = Arena::default();
    let t0 = temps.alloc(Temp { ty: LirType::I64 });
    let t1 = temps.alloc(Temp { ty: LirType::I64 });
    let t2 = temps.alloc(Temp { ty: LirType::I64 });
    let t3 = temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let t4 = temps.alloc(Temp { ty: RAW_PTR });
    let t5 = temps.alloc(Temp { ty: MANAGED_PTR });
    let t6 = temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let t7 = temps.alloc(Temp { ty: RAW_PTR });
    let t8 = temps.alloc(Temp { ty: LirType::I64 });
    let mut blocks = Arena::default();
    let placeholder = |blocks: &mut Arena<BasicBlock>, name: &str| {
        blocks.alloc(BasicBlock {
            name: name.to_string(),
            instructions: vec![],
            terminator: Terminator::Unreachable,
        })
    };
    let entry = placeholder(&mut blocks, "entry");
    let normal = placeholder(&mut blocks, "normal");
    let done = placeholder(&mut blocks, "done");
    let lpad = placeholder(&mut blocks, "lpad");
    let handler_done = placeholder(&mut blocks, "handler_done");
    let cleanup = placeholder(&mut blocks, "cleanup");
    let mut call_targets = CallTargets::default();
    let dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 0);
    let first_invoke = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: dispatch,
        },
        Vec::new(),
        (LirType::I64, RefScan::None),
        t0,
        Vec::new(),
    );
    let second_invoke = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 2,
            destination: managed_local(1),
        },
        Vec::new(),
        (LirType::I64, RefScan::None),
        t1,
        Vec::new(),
    );
    let handler_invoke = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 3,
            destination: managed_local(1),
        },
        Vec::new(),
        (LirType::I64, RefScan::None),
        t8,
        Vec::new(),
    );
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Invoke {
            site: managed_invoke(first_invoke, normal, lpad),
        }],
        terminator: Terminator::Br(normal),
    };
    blocks[normal] = BasicBlock {
        name: "normal".to_string(),
        instructions: vec![Instruction::Invoke {
            site: managed_invoke(second_invoke, done, lpad),
        }],
        terminator: Terminator::Br(done),
    };
    blocks[done] = BasicBlock {
        name: "done".to_string(),
        instructions: vec![Instruction::IntegerBinary {
            out: t2,
            kind: IntegerKind::SIGNED_64,
            operation: IntegerBinaryOperation::Add,
            lhs: Value::Temp(t0),
            rhs: Value::Temp(t1),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(t2)),
        },
    };
    blocks[lpad] = BasicBlock {
        name: "lpad".to_string(),
        instructions: vec![
            Instruction::LandingPad {
                record: t3,
                raw: t4,
            },
            Instruction::BeginCatch {
                out: t5,
                raw: Value::Temp(t4),
            },
            Instruction::Invoke {
                site: managed_invoke(handler_invoke, handler_done, cleanup),
            },
        ],
        terminator: Terminator::Br(handler_done),
    };
    blocks[handler_done] = BasicBlock {
        name: "handler_done".to_string(),
        instructions: vec![Instruction::EndCatch],
        terminator: Terminator::Return {
            value: Some(signed64(0)),
        },
    };
    blocks[cleanup] = BasicBlock {
        name: "cleanup".to_string(),
        instructions: vec![
            Instruction::CleanupPad {
                record: t6,
                raw: t7,
            },
            Instruction::EndCatch,
        ],
        terminator: Terminator::Resume {
            exception: Value::Temp(t6),
        },
    };
    let eh_test = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.eh_test".to_string(),
        params: vec![METADATA_PTR],
        return_ty: LirType::I64,
        call_targets,
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    };

    Module {
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![thrower, may_throw, eh_test],
        entry_symbol: "scoop.eh_test".to_string(),
        meta: string_metadata(),
    }
}

#[test]
fn emits_m8_exceptions() {
    let module = exceptions_module();
    let ir = ir_of(&module);
    assert!(ir.contains("declare ptr addrspace(1) @scoop_rt_begin_catch(ptr)"));
    assert!(ir.contains("@scoop_rt_end_catch"));
    assert!(!ir.contains("@__cxa_begin_catch"));
    assert!(!ir.contains("@__cxa_end_catch"));
    assert!(ir.contains("personality ptr @scoop_eh_personality"));
    assert!(ir.contains("catch ptr null"));
    assert!(ir.contains("cleanup"));
    assert!(ir.contains("resume { ptr, i32 }"));
    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m8_test_{}.o", std::process::id()));
    // `emit_object` verifies the LLVM module before writing, so a
    // successful return means `module.verify()` passed. M9: this
    // also proves invoke / landingpad and the GC strategy coexist
    // — every function carries `gc "statepoint-example"` and the
    // module goes through `rewrite-statepoints-for-gc`.
    emit_object(&module, &output, host_profile()).expect("emit object");
    let bytes = std::fs::read(&output).expect("read object");
    assert!(!bytes.is_empty(), "object file is empty");
    std::fs::remove_file(&output).ok();
}

#[test]
fn darwin_aarch64_eh_artifacts_are_qualified_at_o0_and_o2() {
    let module = exceptions_module();
    let expected_safepoints =
        statepoint::expectations(&module).expect("complete safepoint manifest");
    let expected_eh = artifact::eh_expectations(&module).expect("complete EH manifest");
    assert_eq!(expected_eh.function_count(), 1);
    assert_eq!(
        expected_eh.action_count("scoop.eh_test", artifact::EhActionKind::CatchAll),
        2
    );
    assert_eq!(
        expected_eh.action_count("scoop.eh_test", artifact::EhActionKind::Cleanup),
        1
    );

    let profile = host_profile();
    for (name, optimization) in [
        ("o0", OptimizationLevel::None),
        ("o2", OptimizationLevel::Default),
    ] {
        let machine = profile
            .create_qualification_target_machine(optimization)
            .expect("qualified target machine");
        let context = Context::create();
        let llvm =
            emit_llvm_module(&context, &module, &machine, profile).expect("emit LLVM module");
        llvm.verify().expect("valid LLVM module");
        statepoint::rewrite(&llvm, &machine).expect("rewrite-statepoints-for-gc pass");
        llvm.verify().expect("valid post-RS4GC module");
        statepoint::verify_rewritten(&llvm, &expected_safepoints, profile)
            .expect("statepoint manifest matches");

        let output = std::env::temp_dir().join(format!(
            "scoop_codegen_eh_qualification_{name}_{}.o",
            std::process::id()
        ));
        machine
            .write_to_file(&llvm, FileType::Object, &output)
            .expect("write object");
        profile
            .verify_object(&output, &expected_safepoints, &expected_eh)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        std::fs::remove_file(&output).ok();
    }
}
