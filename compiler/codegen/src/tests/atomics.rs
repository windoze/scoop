use super::*;
use scoop_lir::{
    AtomicCompareExchangeOrder, AtomicCompareExchangeResult, AtomicLoadOrder, AtomicLocation,
    AtomicMemoryOrder, AtomicRmwOperation, AtomicStoreOrder, AtomicValueKind, OptimizationMode,
    TargetProfileId, ValidatedLirTargetSelection,
};

mod fixture;

#[test]
fn source_atomic_instructions_preserve_orders_widths_and_pointer_values() {
    for target in [
        TargetProfileId::DarwinAarch64,
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let module = fixture::module(target);
            let profile = ValidatedBackendProfile::from_selection(
                ValidatedLirTargetSelection::from_id(target),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
            llvm.verify().unwrap();
            let original = llvm.print_to_string().to_string();
            assert_eq!(original.matches("load atomic").count(), 12);
            assert_eq!(original.matches("store atomic").count(), 12);
            assert_eq!(original.matches("cmpxchg ").count(), 72);
            for (opcode, orders) in [
                ("load atomic", &["monotonic", "acquire", "seq_cst"][..]),
                ("store atomic", &["monotonic", "release", "seq_cst"][..]),
            ] {
                for order in orders {
                    assert_eq!(
                        original
                            .lines()
                            .filter(|line| {
                                line.contains(opcode) && line.contains(&format!(" {order}, align"))
                            })
                            .count(),
                        4
                    );
                }
            }
            for pair in [
                "monotonic monotonic",
                "acquire monotonic",
                "acquire acquire",
                "release monotonic",
                "acq_rel monotonic",
                "acq_rel acquire",
                "seq_cst monotonic",
                "seq_cst acquire",
                "seq_cst seq_cst",
            ] {
                assert_eq!(
                    original
                        .lines()
                        .filter(|line| {
                            line.contains("cmpxchg ") && line.contains(&format!(" {pair}, align"))
                        })
                        .count(),
                    8
                );
            }
            assert!(!original.contains("atomic i1,"), "{original}");
            assert!(!original.contains("cmpxchg weak"), "{original}");
            assert_eq!(
                original
                    .lines()
                    .filter(|line| {
                        line.contains("atomicrmw xchg") && line.contains(", ptr addrspace(1)")
                    })
                    .count(),
                5
            );
            let expected = statepoint::expectations(&module).unwrap();
            let expected = statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            statepoint::verify_rewritten(&llvm, &expected, profile).unwrap();
            llvm.verify().unwrap();
            let optimized = llvm.print_to_string().to_string();
            assert_eq!(optimized.matches("cmpxchg ").count(), 72);
            assert!(!optimized.contains("cmpxchg weak"), "{optimized}");
            assert!(optimized.contains("atomicrmw add"));
            assert!(optimized.contains("atomicrmw sub"));
            let object = machine
                .write_to_memory_buffer(&llvm, FileType::Object)
                .unwrap();
            assert!(!object.as_slice().is_empty());
        }
    }
}

#[test]
fn source_atomic_fields_reject_invalid_locations_and_result_types() {
    for invalid in [0, 1, 2] {
        let mut module = fixture::module(TargetProfileId::DarwinAarch64);
        let function = &mut module.functions[0];
        let Instruction::AtomicLoad { location, out, .. } =
            &mut function.blocks[function.entry].instructions[0]
        else {
            unreachable!()
        };
        match invalid {
            0 => location.offset = 8,
            1 => location.offset = 17,
            2 => function.temps[*out].ty = LirType::I64,
            _ => unreachable!(),
        }
        let context = Context::create();
        let machine = host_target_machine().unwrap();
        let error = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap_err();
        assert!(error.to_string().contains("atomic"), "{error}");
    }
}
