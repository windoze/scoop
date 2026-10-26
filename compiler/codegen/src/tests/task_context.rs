use super::*;
use inkwell::values::AnyValue;
use scoop_lir::{NoGcRuntimeFunction as Leaf, OptimizationMode};

fn switched_context_module() -> Module {
    let mut module = values_module();
    module.output = scoop_lir::LirOutput::Library;
    let mut temps = Arena::default();
    let previous = temps.alloc(Temp { ty: MANAGED_PTR });
    let current = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut targets = CallTargets::default();
    let enter = direct_site(
        &mut targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(Leaf::ContextEnter),
        },
        vec![MANAGED_PTR],
        (MANAGED_PTR, RefScan::References(vec![0])),
        previous,
        vec![Value::Param(0)],
    );
    let mut collect = void_site(
        &mut targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::GcCollect),
        },
        vec![],
        vec![],
    );
    let CallSite::Managed(site) = &mut collect else {
        unreachable!()
    };
    site.live = statepoint_live(vec![statepoint_value(
        scoop_lir::CallerRootSource::Temp(previous),
        MANAGED_PTR,
        &[0],
    )]);
    let read = direct_site(
        &mut targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(Leaf::ContextCurrent),
        },
        vec![],
        (MANAGED_PTR, RefScan::References(vec![0])),
        current,
        vec![],
    );
    let leave = void_site(
        &mut targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(Leaf::ContextLeave),
        },
        vec![MANAGED_PTR],
        vec![Value::Temp(previous)],
    );
    let mut blocks = Arena::default();
    let active = blocks.alloc(BasicBlock {
        name: "active".into(),
        instructions: vec![
            Instruction::Call { site: enter },
            Instruction::Call { site: collect },
            Instruction::Call { site: read },
            Instruction::Call { site: leave },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(current)),
        },
    });
    let skip = blocks.alloc(BasicBlock {
        name: "skip".into(),
        instructions: vec![],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![],
        terminator: Terminator::CondBr {
            cond: Value::Param(1),
            then_block: active,
            else_block: skip,
        },
    });
    module.functions = vec![Function {
        callable_body: callable_body("switch_context"),
        safepoints: test_safepoints("switch_context", &blocks, entry),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR, LirType::I1], MANAGED_PTR),
        call_targets: targets,
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    }];
    module
}

#[test]
fn context_access_reads_and_writes_the_original_root_only_when_used() {
    let module = switched_context_module();
    let symbol = module.functions[0].symbol();
    for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
        let context = Context::create();
        let machine = host_target_machine().unwrap();
        let llvm = emit_llvm_module(
            &context,
            &module,
            &machine,
            host_profile().with_optimization(mode),
        )
        .unwrap();
        llvm.verify().unwrap();
        let function = llvm.get_function(symbol).unwrap();
        let ir = function.print_to_string().to_string();
        for name in ["current", "enter", "leave"] {
            assert_eq!(
                ir.contains(&format!("@scoop_rt_context_{name}(")),
                mode == OptimizationMode::Debug,
                "{ir}"
            );
        }
        if mode == OptimizationMode::Release {
            assert!(
                ir.contains("task_context_previous = load ptr addrspace(1)"),
                "{ir}"
            );
            assert!(ir.contains("@scoop_rt_context_slot"), "{ir}");
            let lazy = machine.get_target().get_name() == c"aarch64";
            assert_eq!(
                ir.contains("task_context_slot_cache = alloca ptr"),
                lazy,
                "{ir}"
            );
            for block in function.get_basic_blocks() {
                if matches!(block.get_name().to_str().unwrap(), "entry" | "skip") {
                    assert!(block.get_instructions().all(|instruction| {
                        !instruction
                            .print_to_string()
                            .to_string()
                            .contains("@scoop_rt_context_slot")
                    }));
                }
            }
        }
    }
}

#[test]
fn context_leave_stores_the_relocated_previous_context() {
    let module = switched_context_module();
    let context = Context::create();
    let machine = host_target_machine().unwrap();
    let profile = host_profile().with_optimization(OptimizationMode::Release);
    let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
    statepoint::rewrite(
        &llvm,
        &machine,
        &statepoint::expectations(&module).unwrap(),
        profile,
    )
    .unwrap();
    llvm.verify().unwrap();
    let ir = llvm
        .get_function(module.functions[0].symbol())
        .unwrap()
        .print_to_string()
        .to_string();
    assert!(
        ir.lines()
            .any(|line| line.contains("store ptr addrspace(1)")
                && line.contains(".relocated")
                && line.contains("task_context_slot")),
        "{ir}"
    );
    assert!(!ir.contains("@scoop_rt_context_leave("), "{ir}");
}
