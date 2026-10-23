use super::*;
use inkwell::targets::FileType;

#[test]
fn folded_logical_null_keeps_the_physical_root_needed_by_shared_successors() {
    let directory = tempfile::tempdir().unwrap();
    let plain = include_str!("null_paths.ll");
    let merged = format!(
        "@immortal = addrspace(1) constant i64 0\n{}",
        plain
            .replace("ptr addrspace(1) %root, ptr %fields", "ptr addrspace(1) %input, ptr %fields, i1 %known")
            .replace("entry:\n", "entry:\n  %root = select i1 %known, ptr addrspace(1) @immortal, ptr addrspace(1) %input\n")
    );
    for source in [plain, &merged] {
        for target in [
            scoop_target_data::DARWIN_AARCH64_ID,
            scoop_target_data::LINUX_X86_64_GNU_ID,
            scoop_target_data::LINUX_X86_64_MUSL_ID,
        ] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(scoop_lir::OptimizationMode::Release);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            // Path-sensitive folding already made the restore null, while the
            // shared successor still mentions the original nullable SSA value.
            let llvm = parse(&context, source);
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            crate::statepoint::policy::finish_managed_optimization(
                llvm.get_function("f").unwrap(),
                &machine,
            )
            .unwrap();
            crate::statepoint::finalization::finalize(
                &llvm,
                &manifest(Some((7, one_root()))),
                profile,
            )
            .unwrap();
            let plan = crate::statepoint::lower(&llvm, &machine).unwrap();
            assert_eq!(plan.root_count(7), Some(1));
            llvm.verify().unwrap();
            let output = directory
                .path()
                .join(format!("{}.o", target.canonical_name()));
            machine
                .write_to_file(&llvm, FileType::Object, &output)
                .unwrap();
            assert_machine_root_counts(&output, &plan);
        }
    }
}
