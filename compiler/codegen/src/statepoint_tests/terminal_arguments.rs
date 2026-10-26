use super::*;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn terminal_argument_roots_agree_with_instruction_selection() {
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
            let llvm = parse(
                &context,
                r#"
declare void @consume(ptr addrspace(1))
define void @f(ptr addrspace(1) %argument) gc "statepoint-example" {
entry:
  call void @consume(ptr addrspace(1) %argument) #0
  unreachable
}
attributes #0 = { noduplicate "statepoint-id"="7" }
"#,
            );
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let expected = manifest(Some((7, ExpectedStatepoint::Relocating(Vec::new().into()))));
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            assert_eq!(plan.root_count(7), Some(0));
            let directory = tempfile::tempdir().unwrap();
            let object = directory.path().join("terminal-argument.o");
            machine
                .write_to_file(&llvm, FileType::Object, &object)
                .unwrap();
            assert_machine_root_counts(&object, &plan);
        }
    }
}
