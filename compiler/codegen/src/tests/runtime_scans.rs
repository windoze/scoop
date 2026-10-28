use super::*;

fn root_plan_test_module(
    functions: Vec<Function>,
    extern_functions: scoop_lir::ExternFunctions,
    entry_index: usize,
) -> Module {
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
        functions,
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(entry_index),
        },
        meta: string_metadata(),
    };
    refresh_module_safepoints(&mut module);
    module
}

#[test]
fn callable_runtime_scan_trees_are_emitted_as_closed_strong_atoms() {
    let mut module = managed_poll_test_module();
    let element = scoop_lir::NonEmptyRefScan::new(RefScan::References(vec![8])).unwrap();
    module.functions[0]
        .call_targets
        .root_scans
        .alloc(RefScan::Sequence(vec![
            RefScan::None,
            RefScan::References(vec![0, 16]),
            RefScan::Array {
                length_offset: 24,
                first_element_offset: 32,
                stride: std::num::NonZeroU64::new(8).unwrap(),
                element: Box::new(element),
            },
        ]));
    let plans = scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_module(&module).unwrap();
    let callable = plans
        .callable(module.functions[0].callable_body.id())
        .unwrap();
    assert_eq!(callable.atoms().len(), 4);

    let foundation = scoop_lir::ConeLirFoundation::from_module(&module).unwrap();
    let surface = scoop_lir::ObjectSymbolSurfaceV1::from_foundation(&foundation).unwrap();
    let machine = host_target_machine().unwrap();
    let context = inkwell::context::Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap();
    let ir = llvm.print_to_string().to_string();
    for atom in callable.atoms() {
        let boundary = surface
            .plans()
            .iter()
            .flat_map(|plan| plan.atom_boundaries())
            .find(|boundary| boundary.atom() == atom.atom())
            .unwrap();
        assert!(
            ir.contains(boundary.start().symbol().as_str()),
            "missing runtime-scan start boundary for {}",
            atom.atom()
        );
        assert!(
            ir.contains(boundary.end().symbol().as_str()),
            "missing runtime-scan end boundary for {}",
            atom.atom()
        );
    }
    assert!(!ir.contains(".root_scan."), "{ir}");
    assert!(!ir.contains(".native."), "{ir}");
    assert!(!ir.contains(".invoke."), "{ir}");
}

fn managed_poll_test_module() -> Module {
    let mut call_targets = CallTargets::default();
    let signature = call_targets.void_signatures.alloc(VoidCallSignature::new(
        Vec::new(),
        scoop_lir::CallingConvention::Cdecl,
    ));
    let target = call_targets
        .managed_targets
        .void
        .alloc(scoop_lir::CallTarget {
            destination: scoop_lir::ManagedCallDestination::runtime(
                scoop_lir::ManagedRuntimeFunction::Safepoint,
            ),
            signature,
        });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::ManagedPoll {
            site: scoop_lir::ManagedPollSite {
                kind: scoop_lir::ManagedPollKind::Full,
                target,
                safepoint: test_safepoint(703),
                live: scoop_lir::StatepointLiveSet::default(),
            },
        }],
        terminator: Terminator::Return { value: None },
    });
    let function = Function {
        debug_source: None,
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
        call_targets,
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    root_plan_test_module(vec![function], scoop_lir::ExternFunctions::default(), 0)
}
