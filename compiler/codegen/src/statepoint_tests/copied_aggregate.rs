use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use scoop_lir::{OptimizationMode, TargetProfileId, ValidatedLirTargetSelection};

#[test]
fn copied_managed_aggregate_keeps_pointer_provenance_through_statepoints() {
    for target in [
        TargetProfileId::DarwinAarch64,
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::from_selection(
                ValidatedLirTargetSelection::from_id(target),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(&context, include_str!("copied_aggregate.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            let expected = manifest(Some((
                7,
                ExpectedStatepoint::Relocating(
                    vec![ExpectedRoot {
                        source: CallerRootSource::Local(la_arena::Idx::from_raw(0.into())),
                        byte_offset: 0,
                    }]
                    .into(),
                ),
            )));
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            verify_rewritten_with_profile(&llvm, &plan, profile).unwrap();
            llvm.run_passes(
                "function(verify<safepoint-ir>),verify",
                &machine,
                PassBuilderOptions::create(),
            )
            .unwrap();
            let ir = llvm.print_to_string().to_string();
            assert!(!ir.contains("inttoptr"), "{target:?}/{mode:?}: {ir}");
            assert_eq!(plan.root_count(7), Some(1));
            assert!(ir.contains("gc.relocate.p1"), "{ir}");
            assert!(
                !machine
                    .write_to_memory_buffer(&llvm, FileType::Object)
                    .unwrap()
                    .as_slice()
                    .is_empty()
            );
        }
    }
}
