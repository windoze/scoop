use super::*;
use scoop_lir::OptimizationMode;

#[test]
fn branch_loads_share_a_root_only_without_an_intervening_write() {
    let directory = tempfile::tempdir().unwrap();
    let original = include_str!("branch_loads.ll");
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for writes in [false, true] {
            let source = if writes {
                original.to_owned()
            } else {
                original.replace(
                    "  store ptr addrspace(1) %replacement, ptr %source, align 8\n",
                    "",
                )
            };
            for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
                let profile = ValidatedBackendProfile::for_selection(
                    scoop_target_data::selection(target).unwrap(),
                )
                .unwrap()
                .with_optimization(mode);
                let machine = profile.create_target_machine().unwrap();
                let context = Context::create();
                let llvm = parse(&context, &source);
                llvm.set_triple(&machine.get_triple());
                llvm.set_data_layout(&machine.get_target_data().get_data_layout());
                let roots = [0, 1].map(|index| ExpectedRoot {
                    source: CallerRootSource::Local(la_arena::Idx::from_raw(
                        la_arena::RawIdx::from_u32(index),
                    )),
                    byte_offset: 0,
                });
                let plan = crate::statepoint::rewrite(
                    &llvm,
                    &machine,
                    &manifest(Some((
                        7,
                        ExpectedStatepoint::Relocating(roots.to_vec().into()),
                    ))),
                    profile,
                )
                .unwrap();
                let output = directory.path().join("branch-loads.o");
                machine
                    .write_to_file(&llvm, inkwell::targets::FileType::Object, &output)
                    .unwrap();
                assert_machine_root_counts(&output, &plan);
                let expected = if !writes && mode == OptimizationMode::Release {
                    1
                } else {
                    2
                };
                assert_eq!(plan.root_count(7), Some(expected), "{target:?}/{mode:?}");
            }
        }
    }
}
