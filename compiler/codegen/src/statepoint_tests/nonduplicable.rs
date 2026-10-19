use super::*;
use inkwell::targets::FileType;
use inkwell::values::{AsValueRef, CallSiteValue, InstructionOpcode};
use scoop_lir::OptimizationMode;

#[test]
fn conditional_site_and_leaf_restoration_remain_unique() {
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
            let llvm = parse(&context, include_str!("nonduplicable.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            for block in llvm.get_function("f").unwrap().get_basic_blocks() {
                for instruction in block.get_instructions() {
                    if instruction.get_opcode() != InstructionOpcode::Call {
                        continue;
                    }
                    // SAFETY: the opcode above establishes a CallBase.
                    let call = unsafe { CallSiteValue::new(instruction.as_value_ref()) };
                    let callee = call.get_called_fn_value().unwrap();
                    if matches!(callee.get_name().to_bytes(), b"callee" | b"llvm.fake.use") {
                        crate::statepoint::mark_nonduplicable(&context, call);
                    }
                }
            }
            let plan = crate::statepoint::rewrite(
                &llvm,
                &machine,
                &manifest(Some((7, one_root()))),
                profile,
            )
            .unwrap();
            assert_eq!(plan.site_count(), 1);
            assert_eq!(plan.root_count(7), Some(1));
            llvm.verify().unwrap();
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
