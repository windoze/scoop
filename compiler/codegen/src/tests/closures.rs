use super::*;

/// An M11-shaped module with both ordinary and suspend closure calls.
/// Both return aggregates so the machine ABI has a leading result slot;
/// the closure remains the first source-level argument and the suspend
/// call carries its continuation immediately after it.
fn closure_abi_module() -> Module {
    let ordinary_result_ty = LirType::Aggregate(vec![LirType::I64, MANAGED_PTR]);
    let suspend_result_ty = LirType::Aggregate(vec![LirType::I64, LirType::I64]);
    let mut locals = Arena::default();
    let ordinary_result = locals.alloc(Local {
        name: "ordinary_result".to_string(),
        ty: ordinary_result_ty.clone(),
    });
    let suspend_result = locals.alloc(Local {
        name: "suspend_result".to_string(),
        ty: suspend_result_ty.clone(),
    });
    let mut call_targets = CallTargets::default();
    let ordinary_dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 2);
    let ordinary_site = indirect_result_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: ordinary_dispatch,
        },
        vec![MANAGED_PTR, LirType::I64],
        (ordinary_result_ty, RefScan::References(vec![8])),
        ordinary_result,
        vec![Value::Param(0), Value::Param(1)],
    );
    let suspend_dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 2);
    let suspend_site = indirect_result_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 2,
            destination: suspend_dispatch,
        },
        vec![MANAGED_PTR, MANAGED_PTR],
        (suspend_result_ty, RefScan::None),
        suspend_result,
        vec![Value::Param(0), Value::Param(2)],
    );
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Call {
                site: ordinary_site,
            },
            Instruction::Call { site: suspend_site },
        ],
        terminator: Terminator::Return { value: None },
    });

    Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop.closure_abi".to_string(),
            params: vec![MANAGED_PTR, LirType::I64, MANAGED_PTR],
            return_ty: LirType::Void,
            call_targets,
            locals,
            temps: Arena::default(),
            blocks,
            entry,
        }],
        entry_symbol: "scoop.closure_abi".to_string(),
        meta: string_metadata(),
    }
}

#[test]
fn closure_calls_preserve_hidden_abi_and_indirect_statepoints() {
    let module = closure_abi_module();
    let ir = ir_of(&module);
    assert_eq!(
        ir.matches("getelementptr ptr, ptr addrspace(1) %0, i32 2")
            .count(),
        2,
        "closure calls must load invoke from slot 2:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call void %dispatch_function")
                && line.contains("(ptr %ordinary_result, ptr addrspace(1) %0, i64 %1)")
        }),
        "ordinary closure ABI must be (result slot, closure, arguments):\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call void %dispatch_function")
                && line.contains("ptr %suspend_result, ptr addrspace(1) %0, ptr addrspace(1) %2")
        }),
        "suspend closure ABI must keep continuation after the closure:\n{ir}"
    );
    assert!(
        ir.contains("%ordinary_result = alloca { i64, ptr addrspace(1) }")
            && ir.contains("%suspend_result = alloca { i64, i64 }"),
        "aggregate closure results must use typed return storage:\n{ir}"
    );

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    statepoint::rewrite(&llvm, &machine).expect("rewrite-statepoints-for-gc pass");
    let rewritten = llvm.print_to_string().to_string();
    assert_eq!(
        rewritten
            .lines()
            .filter(|line| {
                line.contains("call token")
                    && line.contains("gc.statepoint")
                    && line.contains("%dispatch_function")
            })
            .count(),
        2,
        "both managed indirect closure calls must become statepoints:\n{rewritten}"
    );
}
