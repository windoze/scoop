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
            value: Some(Value::IntConst(1)),
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
    //   lpad:   t3 = landingpad : ptr
    //           end_catch
    //           ret 0
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
    let cleanup = placeholder(&mut blocks, "cleanup");
    let mut call_targets = CallTargets::default();
    let dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 0);
    let first_invoke = direct_site(
        &mut call_targets,
        dispatch,
        TestCallProtocol::Managed(1),
        Vec::new(),
        (LirType::I64, RefScan::None),
        t0,
        Vec::new(),
    );
    let second_invoke = direct_site(
        &mut call_targets,
        CallDestination::Local(scoop_lir::LocalFunctionId::from_u32(1)),
        TestCallProtocol::Managed(2),
        Vec::new(),
        (LirType::I64, RefScan::None),
        t1,
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
        instructions: vec![Instruction::BinOp {
            out: t2,
            op: BinOp::Add,
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
            Instruction::EndCatch,
        ],
        terminator: Terminator::Return {
            value: Some(Value::IntConst(0)),
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
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
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
    assert!(ir.contains("@__cxa_begin_catch"));
    assert!(ir.contains("@__cxa_end_catch"));
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
    // The landing pad function must carry an exception table.
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("gcc_except_tab"),
        "object file lacks exception tables"
    );
    assert!(
        text.contains("__llvm_stackmaps"),
        "object file lacks the __llvm_stackmaps section"
    );
    std::fs::remove_file(&output).ok();
}
