use super::*;

fn boolean_function(symbol: &str, effect: GcEffect) -> Function {
    let mut temps = Arena::new();
    let result = temps.alloc(Temp { ty: LirType::I1 });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "entry".into(),
        instructions: vec![Instruction::UnaryOp {
            out: result,
            op: scoop_lir::UnOp::Not,
            operand: Value::Param(0),
        }]
        .into(),
        terminator: Terminator::Return {
            value: Some(Value::Temp(result)),
        },
    });
    Function {
        debug_name: Default::default(),
        debug_variables: Vec::new(),
        debug_signature: Default::default(),
        debug_origin: Default::default(),
        callable_body: callable_body(symbol),
        safepoints: Default::default(),
        gc_effect: effect,
        signature: plain_scoop_signature(vec![LirType::I1], LirType::I1),
        call_targets: Default::default(),
        locals: Arena::new(),
        temps,
        blocks,
        entry,
    }
}

fn boolean_caller(symbol: &str, effect: GcEffect, protocol: TestCallProtocol) -> Function {
    let mut function = boolean_function(symbol, effect);
    let argument = match function.blocks[function.entry].terminator {
        Terminator::Return { value: Some(value) } => value,
        _ => unreachable!("boolean function returns its negated argument"),
    };
    let result = function.temps.alloc(Temp { ty: LirType::I1 });
    let site = direct_site(
        &mut function.call_targets,
        protocol,
        vec![LirType::I1],
        (LirType::I1, RefScan::None),
        result,
        vec![argument],
    );
    function.blocks[function.entry]
        .instructions
        .push(Instruction::Call { site });
    function.blocks[function.entry].terminator = Terminator::Return {
        value: Some(Value::Temp(result)),
    };
    function
}

fn boolean_invoke() -> Function {
    let mut function = boolean_caller(
        "boolean_invoke",
        GcEffect::Managed,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_local(2),
        },
    );
    let Terminator::Return { value } = function.blocks[function.entry].terminator else {
        unreachable!("boolean caller returns its result");
    };
    let normal = function.blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "normal".into(),
        instructions: Vec::new().into(),
        terminator: Terminator::Return { value },
    });
    let record = function.temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let raw = function.temps.alloc(Temp { ty: RAW_PTR });
    let caught = function.temps.alloc(Temp { ty: MANAGED_PTR });
    let unwind = function.blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "unwind".into(),
        instructions: vec![
            Instruction::LandingPad { record, raw },
            Instruction::BeginCatch {
                out: caught,
                raw: Value::Temp(raw),
            },
            Instruction::EndCatch,
        ]
        .into(),
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let block = &mut function.blocks[function.entry];
    let Instruction::Call { site } = block.instructions.pop().unwrap() else {
        unreachable!("boolean caller ends with its call");
    };
    block.instructions.push(Instruction::Invoke {
        site: managed_invoke(site, normal, unwind),
    });
    block.terminator = Terminator::Br(normal);
    function
}

fn boolean_module() -> Module {
    let mut dispatch = boolean_function("boolean_dispatch", GcEffect::Managed);
    dispatch.signature = plain_scoop_signature(vec![LirType::I1, METADATA_PTR], LirType::I1);
    let destination = dispatch_destination(&mut dispatch.call_targets, Value::Param(1), 0);
    let result = dispatch.temps.alloc(Temp { ty: LirType::I1 });
    let site = direct_site(
        &mut dispatch.call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination,
        },
        vec![LirType::I1],
        (LirType::I1, RefScan::None),
        result,
        vec![Value::Param(0)],
    );
    dispatch.blocks[dispatch.entry].instructions = vec![Instruction::Call { site }].into();
    dispatch.blocks[dispatch.entry].terminator = Terminator::Return {
        value: Some(Value::Temp(result)),
    };

    let mut module = aggregate_abi_module();
    module.functions = vec![
        boolean_function("boolean_no_gc", GcEffect::NoGc),
        boolean_caller(
            "boolean_no_gc_caller",
            GcEffect::NoGc,
            TestCallProtocol::NoGc {
                destination: no_gc_local(0),
            },
        ),
        boolean_function("boolean_managed", GcEffect::Managed),
        boolean_caller(
            "boolean_managed_caller",
            GcEffect::Managed,
            TestCallProtocol::Managed {
                safepoint: 1,
                destination: managed_local(2),
            },
        ),
        boolean_invoke(),
        dispatch,
    ];
    refresh_module_safepoints(&mut module);
    module
}

fn assert_boolean_abi(ir: &str, rewritten: bool) {
    for line in ir.lines().filter(|line| line.starts_with("define ")) {
        assert!(
            line.contains("zeroext i1") && line.contains("i1 zeroext"),
            "{line}"
        );
    }
    let calls = ir
        .lines()
        .filter(|line| line.contains("call zeroext i1") && !line.contains("gc.result"))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), if rewritten { 1 } else { 3 }, "{ir}");
    for line in calls {
        assert!(line.contains("i1 zeroext"), "{line}");
    }
    let statepoints = ir
        .lines()
        .filter(|line| line.contains("= ") && line.contains("gc.statepoint"))
        .collect::<Vec<_>>();
    assert_eq!(statepoints.len(), if rewritten { 3 } else { 1 }, "{ir}");
    for line in statepoints {
        assert!(line.contains("i1 zeroext"), "{line}");
    }
    let results = ir
        .lines()
        .filter(|line| line.contains("= ") && line.contains("gc.result"))
        .collect::<Vec<_>>();
    assert_eq!(results.len(), if rewritten { 3 } else { 1 }, "{ir}");
    for line in results {
        assert!(line.contains("call zeroext i1"), "{line}");
    }
}

#[test]
fn scalar_boolean_calls_and_statepoints_keep_zero_extension_on_every_target() {
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        let profile =
            ValidatedBackendProfile::for_selection(scoop_target_data::selection(target).unwrap())
                .unwrap();
        let machine = profile.create_target_machine().unwrap();
        let mut module = boolean_module();
        module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
        let context = Context::create();
        let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
        llvm.verify().unwrap();
        assert_boolean_abi(&llvm.print_to_string().to_string(), false);
        let expected = statepoint::expectations(&module).unwrap();
        statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
        llvm.verify().unwrap();
        assert_boolean_abi(&llvm.print_to_string().to_string(), true);
        machine
            .write_to_memory_buffer(&llvm, FileType::Object)
            .unwrap();
    }
}
