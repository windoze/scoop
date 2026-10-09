use super::*;
use inkwell::values::AnyValue;

#[test]
fn conditional_polls_materialize_roots_only_on_the_slow_edge() {
    let mut module = qualification::stackmap_qualification_module();
    for function in &mut module.functions {
        let Instruction::ManagedPoll { site } = &function.blocks[function.entry].instructions[0]
        else {
            unreachable!()
        };
        let second = Instruction::ManagedPoll {
            site: scoop_lir::ManagedPollSite {
                target: site.target,
                safepoint: test_safepoint(100),
                live: site.live.clone(),
            },
        };
        function.blocks[function.entry].instructions.push(second);
        function.safepoints = test_safepoints_for_owner(
            function.callable_body.id(),
            &function.blocks,
            function.entry,
        );
    }
    for target in [
        scoop_lir::TargetProfileId::DarwinAarch64,
        scoop_lir::TargetProfileId::LinuxX86_64Gnu,
        scoop_lir::TargetProfileId::LinuxX86_64Musl,
    ] {
        module.meta = string_metadata_for(scoop_lir::LirTargetProfile::from_id(target));
        let profile = ValidatedBackendProfile::from_selection(
            scoop_lir::ValidatedLirTargetSelection::from_id(target),
        )
        .unwrap();
        let machine = profile
            .create_qualification_target_machine(OptimizationLevel::None)
            .unwrap();
        let context = Context::create();
        let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
        for function in &module.functions {
            let emitted = llvm.get_function(function.symbol()).unwrap();
            let function_ir = emitted.print_to_string().to_string();
            assert_eq!(
                function_ir
                    .matches("load ptr, ptr @scoop_rt_poll_state")
                    .count(),
                1,
                "{function_ir}"
            );
            assert_eq!(
                function_ir.matches("load atomic").count(),
                8,
                "{function_ir}"
            );
            let entry = emitted.get_first_basic_block().unwrap();
            let entry_ir = entry
                .get_instructions()
                .map(|instruction| instruction.print_to_string().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(entry_ir.matches("load atomic").count(), 4, "{entry_ir}");
            assert_eq!(entry_ir.matches("acquire").count(), 4, "{entry_ir}");
            assert!(entry_ir.contains("icmp eq i32 %poll_mode, 1"), "{entry_ir}");
            assert!(!entry_ir.contains("_source"), "{entry_ir}");
            assert!(
                !entry_ir.contains("call void @scoop_rt_safepoint"),
                "{entry_ir}"
            );
            let slow = emitted
                .get_basic_blocks()
                .into_iter()
                .find(|block| block.get_name().to_bytes().starts_with(b"poll.slow."))
                .unwrap();
            let slow_ir = slow
                .get_instructions()
                .map(|instruction| instruction.print_to_string().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                slow_ir.contains("call void @scoop_rt_safepoint"),
                "{slow_ir}"
            );
        }
        let expected = statepoint::expectations(&module).unwrap();
        let expected = statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
        statepoint::verify_rewritten(&llvm, &expected, profile).unwrap();
    }
}
