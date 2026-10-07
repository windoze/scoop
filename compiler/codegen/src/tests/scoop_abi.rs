use super::*;

mod zst;

fn aggregate_type() -> LirType {
    LirType::Aggregate(vec![LirType::I64, LirType::I64, LirType::I64])
}

fn aggregate_value() -> scoop_lir::AbiValue {
    abi_value_with_layout(aggregate_type(), 24, 8, RefScan::None)
}

fn aggregate_signature() -> scoop_lir::ScoopAbiSignature {
    let value = aggregate_value();
    scoop_lir::ScoopAbiSignature::new(
        vec![scoop_lir::AbiArgument::Indirect(value.clone())],
        scoop_lir::AbiReturn::Indirect(value),
        scoop_lir::CallingConvention::Cdecl,
    )
}

fn aggregate_identity(symbol: &str, gc_effect: GcEffect) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    Function {
        callable_body: callable_body(symbol),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect,
        signature: aggregate_signature(),
        call_targets: CallTargets::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

fn aggregate_call(
    targets: &mut CallTargets,
    protocol: TestCallProtocol,
    argument: scoop_lir::LocalId,
    result: scoop_lir::LocalId,
    locals: &Arena<Local>,
) -> CallSite {
    let value = aggregate_value();
    let signature =
        targets
            .indirect_result_signatures
            .alloc(IndirectResultCallSignature::scoop_sret(
                vec![scoop_lir::AbiArgument::Indirect(value.clone())],
                value.clone(),
                scoop_lir::CallingConvention::Cdecl,
            ));
    let storage = scoop_lir::AbiArgumentStorage::new(argument, locals[argument].ty(), &value)
        .expect("test aggregate argument storage has its exact ABI type");
    protocol_site(
        targets,
        protocol,
        TestTypedCall::IndirectResult {
            signature,
            storage: result,
            args: vec![scoop_lir::AbiCallArgument::Indirect(storage)],
        },
    )
}

fn aggregate_call_storage() -> (Arena<Local>, scoop_lir::LocalId, scoop_lir::LocalId) {
    let mut locals = Arena::new();
    let argument = locals.alloc(test_local("aggregate_argument", aggregate_type()));
    let result = locals.alloc(test_local("aggregate_result", aggregate_type()));
    (locals, argument, result)
}

fn aggregate_literal(temps: &mut Arena<Temp>) -> (TempId, Instruction) {
    let value = temps.alloc(Temp {
        ty: aggregate_type(),
    });
    (
        value,
        Instruction::MakeAggregate {
            out: value,
            elements: vec![signed64(1), signed64(2), signed64(3)],
        },
    )
}

fn ordinary_caller(symbol: &str, gc_effect: GcEffect, protocol: TestCallProtocol) -> Function {
    let (locals, argument, result) = aggregate_call_storage();
    let mut temps = Arena::new();
    let (value, make_value) = aggregate_literal(&mut temps);
    let mut call_targets = CallTargets::default();
    let site = aggregate_call(&mut call_targets, protocol, argument, result, &locals);
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            make_value,
            Instruction::Store {
                local: argument,
                value: Value::Temp(value),
            },
            Instruction::Call { site },
        ],
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body: callable_body(symbol),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect,
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
        call_targets,
        locals,
        temps,
        blocks,
        entry,
    }
}

fn invoke_caller() -> Function {
    let (locals, argument, result) = aggregate_call_storage();
    let mut temps = Arena::new();
    let (value, make_value) = aggregate_literal(&mut temps);
    let record = temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let raw = temps.alloc(Temp { ty: RAW_PTR });
    let caught = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut call_targets = CallTargets::default();
    let site = aggregate_call(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_local(2),
        },
        argument,
        result,
        &locals,
    );
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Unreachable,
    });
    let normal = blocks.alloc(BasicBlock {
        name: "normal".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    let unwind = blocks.alloc(BasicBlock {
        name: "unwind".to_string(),
        instructions: vec![
            Instruction::LandingPad { record, raw },
            Instruction::BeginCatch {
                out: caught,
                raw: Value::Temp(raw),
            },
            Instruction::EndCatch,
        ],
        terminator: Terminator::Return { value: None },
    });
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            make_value,
            Instruction::Store {
                local: argument,
                value: Value::Temp(value),
            },
            Instruction::Invoke {
                site: managed_invoke(site, normal, unwind),
            },
        ],
        terminator: Terminator::Br(normal),
    };
    Function {
        callable_body: callable_body("scoop.aggregate_invoke_caller"),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
        call_targets,
        locals,
        temps,
        blocks,
        entry,
    }
}

fn aggregate_dispatch_caller() -> Function {
    let (locals, argument, result) = aggregate_call_storage();
    let mut temps = Arena::new();
    let (value, make_value) = aggregate_literal(&mut temps);
    let mut call_targets = CallTargets::default();
    let dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 0);
    let site = aggregate_call(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 3,
            destination: dispatch,
        },
        argument,
        result,
        &locals,
    );
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            make_value,
            Instruction::Store {
                local: argument,
                value: Value::Temp(value),
            },
            Instruction::Call { site },
        ],
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body: callable_body("scoop.aggregate_dispatch_caller"),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![METADATA_PTR], LirType::Void),
        call_targets,
        locals,
        temps,
        blocks,
        entry,
    }
}

fn aggregate_abi_module() -> Module {
    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals: Arena::new(),
        initialization_units: Arena::new(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions: Default::default(),
        native_globals: Arena::new(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        functions: vec![
            aggregate_identity("scoop.aggregate_no_gc", GcEffect::NoGc),
            ordinary_caller(
                "scoop.aggregate_no_gc_caller",
                GcEffect::NoGc,
                TestCallProtocol::NoGc {
                    destination: no_gc_local(0),
                },
            ),
            aggregate_identity("scoop.aggregate_managed", GcEffect::Managed),
            ordinary_caller(
                "scoop.aggregate_managed_caller",
                GcEffect::Managed,
                TestCallProtocol::Managed {
                    safepoint: 2,
                    destination: managed_local(2),
                },
            ),
            invoke_caller(),
            aggregate_dispatch_caller(),
        ],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(4),
        },
        meta: string_metadata(),
    };
    refresh_module_safepoints(&mut module);
    module
}

fn has_ordered_aggregate_attributes(line: &str) -> bool {
    let Some(sret) = line.find("sret({ i64, i64, i64 }) align 8") else {
        return false;
    };
    let Some(byval) = line.find("byval({ i64, i64, i64 }) align 8") else {
        return false;
    };
    sret < byval
}

fn managed_aggregate_type() -> LirType {
    LirType::Aggregate(vec![MANAGED_PTR, LirType::I64, LirType::I64])
}

fn managed_aggregate_value() -> scoop_lir::AbiValue {
    abi_value_with_layout(
        managed_aggregate_type(),
        24,
        8,
        RefScan::References(vec![0]),
    )
}

fn managed_aggregate_signature() -> scoop_lir::ScoopAbiSignature {
    let value = managed_aggregate_value();
    scoop_lir::ScoopAbiSignature::new(
        vec![scoop_lir::AbiArgument::Indirect(value.clone())],
        scoop_lir::AbiReturn::Indirect(value),
        scoop_lir::CallingConvention::Cdecl,
    )
}

fn native_aggregate_module() -> Module {
    let mut extern_functions = scoop_lir::ExternFunctions::default();
    let native = extern_functions.alloc_scoop(scoop_lir::ScoopExternFunction {
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "nativeAggregate".to_string(),
            native_symbol: "native_aggregate".to_string(),
            library: "fixture".to_string(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        gc_effect: GcEffect::Managed,
        signature: managed_aggregate_signature(),
    });

    let mut locals = Arena::new();
    let argument = locals.alloc(test_local("native_argument", managed_aggregate_type()));
    let result = locals.alloc(test_local("native_result", managed_aggregate_type()));
    let mut temps = Arena::new();
    let literal = temps.alloc(Temp {
        ty: managed_aggregate_type(),
    });
    let abi_value = managed_aggregate_value();
    let mut call_targets = CallTargets::default();
    let signature =
        call_targets
            .indirect_result_signatures
            .alloc(IndirectResultCallSignature::scoop_sret(
                vec![scoop_lir::AbiArgument::Indirect(abi_value.clone())],
                abi_value.clone(),
                scoop_lir::CallingConvention::Cdecl,
            ));
    let argument_storage =
        scoop_lir::AbiArgumentStorage::new(argument, locals[argument].ty(), &abi_value)
            .expect("native aggregate argument storage has its exact ABI type");
    let mut site = protocol_site(
        &mut call_targets,
        TestCallProtocol::NativeBorrowed {
            safepoint: 4,
            destination: scoop_lir::NativeBorrowedCallDestination::extern_function(native),
            result: NativeBorrowedResultRoot::Rooted { storage: result },
        },
        TestTypedCall::IndirectResult {
            signature,
            storage: result,
            args: vec![scoop_lir::AbiCallArgument::Indirect(argument_storage)],
        },
    );
    let CallSite::NativeBorrowed(native_site) = &mut site else {
        unreachable!("test protocol constructs a native-borrowed site")
    };
    native_site.roots = scoop_lir::NativeBorrowedRootSet::new(vec![scoop_lir::CallerRoot {
        source: scoop_lir::CallerRootSource::Local(argument),
        scan: scoop_lir::NonEmptyRefScan::new(RefScan::References(vec![0]))
            .expect("managed aggregate has one root leaf"),
    }]);

    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: literal,
                elements: vec![Value::Param(0), signed64(10), signed64(20)],
            },
            Instruction::Store {
                local: argument,
                value: Value::Temp(literal),
            },
            Instruction::Call { site },
        ],
        terminator: Terminator::Return { value: None },
    });
    let caller = Function {
        callable_body: callable_body("scoop.native_aggregate_caller"),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], LirType::Void),
        call_targets,
        locals,
        temps,
        blocks,
        entry,
    };

    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals: Arena::new(),
        initialization_units: Arena::new(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions,
        native_globals: Arena::new(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        functions: vec![caller],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
        meta: string_metadata(),
    };
    refresh_module_safepoints(&mut module);
    module
}

fn has_ordered_managed_aggregate_attributes(line: &str) -> bool {
    let Some(sret) = line.find("sret({ ptr addrspace(1), i64, i64 }) align 8") else {
        return false;
    };
    let Some(byval) = line.find("byval({ ptr addrspace(1), i64, i64 }) align 8") else {
        return false;
    };
    sret < byval
}

#[test]
fn aggregate_abi_attributes_survive_definitions_calls_and_statepoint_rewrite() {
    let module = aggregate_abi_module();
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect("emit aggregate ABI module");
    llvm.verify().expect("valid pre-RS4GC aggregate ABI module");
    let before = llvm.print_to_string().to_string();
    let no_gc = llvm_function_symbol(&module.functions[0]);
    let managed = llvm_function_symbol(&module.functions[2]);

    assert!(
        before.lines().any(|line| {
            line.contains(&format!("define void {no_gc}")) && has_ordered_aggregate_attributes(line)
        }),
        "callee definition lost the exact sret/byval/align ABI:\n{before}"
    );
    assert!(
        before.lines().any(|line| {
            line.contains(&format!("call void {no_gc}")) && has_ordered_aggregate_attributes(line)
        }),
        "ordinary call site lost the exact sret/byval/align ABI:\n{before}"
    );
    assert!(
        before.lines().any(|line| {
            line.contains("invoke token")
                && line.contains("gc.statepoint")
                && line.contains(&managed)
                && has_ordered_aggregate_attributes(line)
        }),
        "explicit managed statepoint did not offset the wrapped sret/byval attributes:\n{before}"
    );
    assert!(
        before.lines().any(|line| {
            line.contains(&format!("call void {managed}")) && has_ordered_aggregate_attributes(line)
        }),
        "ordinary managed call lost its pre-RS4GC sret/byval attributes:\n{before}"
    );
    assert!(
        before.lines().any(|line| {
            line.contains("call void %dispatch_function") && has_ordered_aggregate_attributes(line)
        }),
        "managed dispatch call lost its pre-RS4GC sret/byval attributes:\n{before}"
    );

    let expected = statepoint::expectations(&module).expect("complete safepoint manifest");
    let expected = statepoint::rewrite(&llvm, &machine, &expected, host_profile())
        .expect("rewrite aggregate statepoints");
    llvm.verify()
        .expect("valid post-RS4GC aggregate ABI module");
    statepoint::verify_rewritten(&llvm, &expected, host_profile())
        .expect("aggregate ABI statepoint manifest matches");
    let after = llvm.print_to_string().to_string();
    assert!(
        after.lines().any(|line| {
            line.contains("invoke token")
                && line.contains("gc.statepoint")
                && line.contains(&managed)
                && has_ordered_aggregate_attributes(line)
        }),
        "RS4GC dropped or reordered the wrapped sret/byval attributes:\n{after}"
    );
    assert!(
        after.lines().any(|line| {
            line.contains("call token")
                && line.contains("gc.statepoint")
                && line.contains(&managed)
                && has_ordered_aggregate_attributes(line)
        }),
        "RS4GC dropped or reordered ordinary managed sret/byval attributes:\n{after}"
    );
    assert!(
        after.lines().any(|line| {
            line.contains("call token")
                && line.contains("gc.statepoint")
                && line.contains("%dispatch_function")
                && has_ordered_aggregate_attributes(line)
        }),
        "RS4GC dropped or reordered managed dispatch sret/byval attributes:\n{after}"
    );
}

#[test]
fn scoop_extern_aggregate_abi_keeps_typed_storage_rooted_across_native_transition() {
    let module = native_aggregate_module();
    let ir = ir_of(&module);

    assert!(
        ir.lines().any(|line| {
            line.contains("declare void @native_aggregate")
                && has_ordered_managed_aggregate_attributes(line)
        }),
        "Scoop extern declaration lost its exact sret/byval ABI:\n{ir}"
    );
    let native_call = ir
        .lines()
        .find(|line| {
            line.contains("call void @native_aggregate")
                && has_ordered_managed_aggregate_attributes(line)
        })
        .expect("native Scoop call keeps exact sret/byval attributes");
    assert!(
        native_call.contains("%native_result") && native_call.contains("%native_argument"),
        "native call must use the published exact result and argument storage:\n{native_call}"
    );
    let call = ir
        .find("call void @native_aggregate")
        .expect("native aggregate call");
    let enter = ir[..call]
        .rfind("@scoop_rt_enter_native_borrowed")
        .expect("native-borrowed transition enters native state");
    let leave = call
        + ir[call..]
            .find("@scoop_rt_leave_native_borrowed")
            .expect("native-borrowed transition leaves native state");
    let root_reload = leave
        + ir[leave..]
            .find("published_root_reload = load")
            .expect("continuation roots reload after the leave handshake");
    let result_reload = root_reload
        + ir[root_reload..]
            .find("native_result_reload = load")
            .expect("rooted result reloads after the leave handshake");
    let pop = result_reload
        + ir[result_reload..]
            .find("@scoop_rt_pop_caller_roots")
            .expect("caller roots pop after all reloads");
    assert!(
        ir[..enter].contains("@scoop_rt_push_caller_roots")
            && enter < call
            && call < leave
            && leave < root_reload
            && root_reload < result_reload
            && result_reload < pop,
        "argument/result roots must remain published through the native call, leave handshake, and all continuation reloads:\n{ir}"
    );
    assert!(
        ir[pop..].contains(
            "store volatile { ptr addrspace(1), i64, i64 } %published_root_reload, ptr %native_argument"
        ),
        "the post-handshake caller root must be restored only after roots pop:\n{ir}"
    );
    assert!(
        ir[pop..].contains(
            "store { ptr addrspace(1), i64, i64 } %native_result_reload, ptr %native_result"
        ),
        "the post-handshake value must be preserved as the ordinary indirect result after roots pop:\n{ir}"
    );
}
