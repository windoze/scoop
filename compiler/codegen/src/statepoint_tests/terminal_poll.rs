use super::*;
use inkwell::targets::FileType;
use scoop_lir::{OptimizationMode, TargetProfileId, ValidatedLirTargetSelection};

#[test]
fn terminal_call_after_conditional_poll_keeps_one_machine_site() {
    let directory = tempfile::tempdir().unwrap();
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
            let llvm = parse(&context, include_str!("terminal_poll.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            crate::statepoint::configure_function(
                &context,
                llvm.get_function("f").unwrap(),
                GcEffect::Managed,
                profile,
            );
            let expected = ExpectedSafepoints {
                sites: [(7, "poll.slow"), (8, "poll.continue")]
                    .into_iter()
                    .map(|(id, block)| {
                        (
                            id,
                            ExpectedSite {
                                function: "f".into(),
                                block: block.into(),
                                statepoint: ExpectedStatepoint::Relocating(Vec::new().into()),
                            },
                        )
                    })
                    .collect(),
                functions: BTreeMap::from([("f".into(), GcEffect::Managed)]),
            };
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            verify_rewritten_with_profile(&llvm, &plan, profile).unwrap();
            assert_eq!(plan.root_count(7), Some(0));
            assert_eq!(plan.root_count(8), Some(0));
            let output = directory.path().join(format!("{target:?}-{mode:?}.o"));
            machine
                .write_to_file(&llvm, FileType::Object, &output)
                .unwrap();
            profile
                .verify_object(&output, &plan, &crate::artifact::ExpectedEh::default())
                .unwrap_or_else(|error| panic!("{target:?}/{mode:?}: {error}"));
        }
    }
}
