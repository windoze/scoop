use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use inkwell::values::{AsValueRef, BasicValue, CallSiteValue, InstructionOpcode};
use scoop_lir::OptimizationMode;

#[test]
fn split_aggregate_merges_share_roots_only_for_the_same_incoming_values() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            for select in [false, true] {
                for equivalent in [true, false] {
                    let mut source = include_str!("aggregate_merge_roots.ll").to_owned();
                    if !equivalent {
                        source = source.replace(
                            "[%a.object, %left], [%b.object, %right]",
                            "[%b.object, %left], [%a.object, %right]",
                        );
                    }
                    if select {
                        let start = source.find("  br i1 %choose").unwrap();
                        let end = source.find("join:\n").unwrap() + "join:\n".len();
                        source.replace_range(start..end, "");
                        source = source
                            .replace(
                                "phi %I [%a, %left], [%b, %right]",
                                "select i1 %choose, %I %a, %I %b",
                            )
                            .replace(
                                "phi ptr addrspace(1) [%a.object, %left], [%b.object, %right]",
                                "select i1 %choose, ptr addrspace(1) %a.object, ptr addrspace(1) %b.object",
                            )
                            .replace(
                                "phi ptr addrspace(1) [%b.object, %left], [%a.object, %right]",
                                "select i1 %choose, ptr addrspace(1) %b.object, ptr addrspace(1) %a.object",
                            );
                    }
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
                    let expected = manifest(Some((
                        7,
                        ExpectedStatepoint::Relocating(
                            vec![ExpectedRoot {
                                source: CallerRootSource::Temp(la_arena::Idx::from_raw(0.into())),
                                byte_offset: 0,
                            }]
                            .into(),
                        ),
                    )));
                    let plan =
                        crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
                    llvm.run_passes(
                        "function(verify<safepoint-ir>),verify",
                        &machine,
                        PassBuilderOptions::create(),
                    )
                    .unwrap();
                    let function = llvm.get_function("f").unwrap();
                    let returned = function
                        .get_basic_blocks()
                        .into_iter()
                        .flat_map(|block| block.get_instructions())
                        .find(|instruction| instruction.get_opcode() == InstructionOpcode::Return)
                        .unwrap()
                        .get_operand(0)
                        .unwrap()
                        .value()
                        .unwrap();
                    let object = super::aggregate_uses::field(returned, 0);
                    let instruction = object.as_instruction_value().unwrap();
                    assert_eq!(instruction.get_opcode(), InstructionOpcode::Call);
                    // SAFETY: the opcode above establishes a CallBase.
                    let call = unsafe { CallSiteValue::new(instruction.as_value_ref()) };
                    assert!(
                        call.get_called_fn_value()
                            .unwrap()
                            .get_name()
                            .to_bytes()
                            .starts_with(b"llvm.experimental.gc.relocate")
                    );
                    let path = directory.path().join("aggregate-merge.o");
                    machine
                        .write_to_file(&llvm, FileType::Object, &path)
                        .unwrap();
                    assert_machine_root_counts(&path, &plan);
                    assert_eq!(
                        plan.root_count(7),
                        Some(if equivalent { 1 } else { 2 }),
                        "{target:?}/{mode:?}/select={select}/equivalent={equivalent}"
                    );
                }
            }
        }
    }
}

#[test]
fn split_aggregate_loop_roots_agree_with_machine_phi_coalescing() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Release, OptimizationMode::Debug] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(&context, include_str!("aggregate_loop_roots.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let expected = manifest(Some((
                7,
                ExpectedStatepoint::Relocating(
                    vec![ExpectedRoot {
                        source: CallerRootSource::Temp(la_arena::Idx::from_raw(0.into())),
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
            let returned = llvm
                .get_function("f")
                .unwrap()
                .get_basic_blocks()
                .into_iter()
                .flat_map(|block| block.get_instructions())
                .find(|instruction| instruction.get_opcode() == InstructionOpcode::Return)
                .unwrap()
                .get_operand(0)
                .unwrap()
                .value()
                .unwrap();
            assert!(super::aggregate_uses::has_relocation(
                super::aggregate_uses::field(returned, 0),
                &mut std::collections::HashSet::new(),
            ));
            let path = directory.path().join("aggregate-loop.o");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            assert_machine_root_counts(&path, &plan);
            assert_eq!(plan.root_count(7), Some(2), "{target:?}/{mode:?}");
        }
    }
}
