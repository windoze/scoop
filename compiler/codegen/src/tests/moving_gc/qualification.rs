use super::*;

fn stackmap_qualification_module() -> Module {
    fn poll_function(
        symbol: &str,
        safepoint: u64,
        params: Vec<LirType>,
        return_ty: LirType,
        live: scoop_lir::StatepointLiveSet,
        return_value: Option<Value>,
    ) -> Function {
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
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![Instruction::ManagedPoll {
                site: scoop_lir::ManagedPollSite {
                    target,
                    safepoint: test_safepoint(safepoint),
                    live,
                },
            }],
            terminator: Terminator::Return {
                value: return_value,
            },
        });
        Function {
            callable_body: callable_body(symbol),
            safepoints: test_safepoints(symbol, &blocks, entry),
            gc_effect: GcEffect::Managed,
            symbol: symbol.to_string(),
            signature: plain_scoop_signature(params, return_ty),
            call_targets,
            locals: Arena::default(),
            temps: Arena::default(),
            blocks,
            entry,
        }
    }

    let pair = LirType::Aggregate(vec![MANAGED_PTR, MANAGED_PTR]);
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
        functions: vec![
            poll_function(
                "scoop_qualification_zero",
                1,
                Vec::new(),
                LirType::Void,
                scoop_lir::StatepointLiveSet::default(),
                None,
            ),
            poll_function(
                "scoop_qualification_one",
                2,
                vec![MANAGED_PTR],
                MANAGED_PTR,
                statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Param(0),
                    MANAGED_PTR,
                    &[0],
                )]),
                Some(Value::Param(0)),
            ),
            poll_function(
                "scoop_qualification_many",
                3,
                vec![pair.clone()],
                pair.clone(),
                statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Param(0),
                    pair,
                    &[0, 8],
                )]),
                Some(Value::Param(0)),
            ),
        ],
        entry_symbol: "scoop_qualification_zero".to_string(),
        meta: string_metadata(),
    }
}

fn assert_aarch64_frame_disassembly(object: &std::path::Path, optimization: &str) {
    let output = std::process::Command::new("otool")
        .arg("-tvV")
        .arg(object)
        .output()
        .expect("disassemble qualification object");
    assert!(
        output.status.success(),
        "{optimization}: otool failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let disassembly = String::from_utf8_lossy(&output.stdout);
    for symbol in [
        "_scoop_qualification_zero:",
        "_scoop_qualification_one:",
        "_scoop_qualification_many:",
    ] {
        let start = disassembly
            .find(symbol)
            .unwrap_or_else(|| panic!("{optimization}: missing {symbol}:\n{disassembly}"));
        let body = &disassembly[start..];
        let end = body[1..]
            .find("\n_")
            .map_or(body.len(), |offset| offset + 1);
        let body = &body[..end];
        assert!(
            body.contains("stp\tx29, x30, [sp"),
            "{optimization}: {symbol} does not save the frame record:\n{body}"
        );
        assert!(
            body.contains("add\tx29, sp") || body.contains("mov\tx29, sp"),
            "{optimization}: {symbol} does not establish the x29 frame chain:\n{body}"
        );
        assert!(
            body.lines().any(|line| line.contains("\tbl\t")),
            "{optimization}: {symbol} has no managed call instruction:\n{body}"
        );
    }
}

#[test]
fn aarch64_statepoint_artifacts_are_qualified_at_o0_and_o2() {
    let module = stackmap_qualification_module();
    let expected = statepoint::expectations(&module).expect("complete safepoint manifest");
    let expected_eh = artifact::eh_expectations(&module).expect("complete EH manifest");
    let runtime_ids = module
        .functions
        .iter()
        .map(|function| {
            function
                .safepoints
                .iter()
                .next()
                .expect("qualification function has one poll")
                .runtime_id()
                .get()
        })
        .collect::<Vec<_>>();
    assert_eq!(expected.root_count(runtime_ids[0]), Some(0));
    assert_eq!(expected.root_count(runtime_ids[1]), Some(1));
    assert_eq!(expected.root_count(runtime_ids[2]), Some(2));
    let profile = host_profile();
    for (name, optimization) in [
        ("o0", OptimizationLevel::None),
        ("o2", OptimizationLevel::Default),
    ] {
        let machine = profile
            .create_qualification_target_machine(optimization)
            .expect("qualified target machine");
        let context = Context::create();
        let llvm = emit_llvm_module(&context, &module, &machine, profile).expect("emit module");
        llvm.verify().expect("valid LLVM module");
        statepoint::rewrite(&llvm, &machine).expect("rewrite-statepoints-for-gc pass");
        llvm.verify().expect("valid post-RS4GC module");
        statepoint::verify_rewritten(&llvm, &expected, profile)
            .expect("statepoint manifest matches");
        let ir = llvm.print_to_string().to_string();
        assert!(
            ir.contains("gc.statepoint"),
            "{name}: statepoint intrinsics missing after rewrite-statepoints-for-gc:\n{ir}"
        );

        let output = std::env::temp_dir().join(format!(
            "scoop_codegen_statepoint_{name}_{}.o",
            std::process::id()
        ));
        machine
            .write_to_file(&llvm, FileType::Object, &output)
            .expect("write object");
        profile
            .verify_object(&output, &expected, &expected_eh)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_aarch64_frame_disassembly(&output, name);
        std::fs::remove_file(&output).ok();
    }
}
