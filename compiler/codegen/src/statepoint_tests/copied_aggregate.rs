use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn copied_managed_aggregate_keeps_pointer_provenance_through_statepoints() {
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target)
                    .expect("implemented target selected by this consumer"),
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
