use super::*;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn short_circuit_null_guards_are_simplified_before_site_planning() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(&context, include_str!("guarded_unwrap.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            let expected = ExpectedSafepoints {
                sites: [(7, "f"), (8, "g")]
                    .into_iter()
                    .map(|(id, function)| {
                        (
                            id,
                            ExpectedSite {
                                function: function.into(),
                                block: "failure".into(),
                                statepoint: ExpectedStatepoint::Relocating(Vec::new().into()),
                            },
                        )
                    })
                    .collect(),
                functions: ["f", "g"]
                    .into_iter()
                    .map(|function| (function.into(), GcEffect::Managed))
                    .collect(),
            };
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            let guarded = (mode == OptimizationMode::Debug).then_some(0);
            assert_eq!(plan.root_count(7), guarded, "{target:?}/{mode:?}");
            assert_eq!(plan.root_count(8), Some(0), "{target:?}/{mode:?}");
            let output = directory
                .path()
                .join(format!("{}-{mode:?}.o", target.canonical_name()));
            machine
                .write_to_file(&llvm, FileType::Object, &output)
                .unwrap();
            assert_machine_root_counts(&output, &plan);
        }
    }
}
