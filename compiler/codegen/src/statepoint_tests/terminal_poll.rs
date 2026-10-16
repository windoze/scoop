use super::*;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn terminal_call_after_conditional_poll_keeps_one_machine_site() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::from_selection(
                scoop_target_data::selection(target)
                    .expect("implemented target selected by this consumer"),
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

            assert_eq!(plan.root_count(7), Some(0));
            assert_eq!(plan.root_count(8), Some(0));
            let output = directory
                .path()
                .join(format!("{}-{mode:?}.o", target.canonical_name()));
            machine
                .write_to_file(&llvm, FileType::Object, &output)
                .unwrap();
            assert_machine_site_count(&output, plan.site_count());
        }
    }
}
