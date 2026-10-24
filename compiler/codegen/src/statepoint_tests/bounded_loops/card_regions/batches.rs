use super::*;
use inkwell::values::{BasicValue, FunctionValue, PhiValue};

fn strided(source: String, stride: u64) -> String {
    source
        .replace(
            "inner:\n",
            "inner:\n  %position = phi i64 [0, %chunk], [%advance, %inner]\n",
        )
        .replace("%wide = sext i32 %i to i64", "%wide = add i64 %position, 0")
        .replace(
            "getelementptr ptr addrspace(1), ptr addrspace(1) %array",
            if stride == 16 {
                "getelementptr {ptr addrspace(1), ptr}, ptr addrspace(1) %array"
            } else {
                "getelementptr ptr addrspace(1), ptr addrspace(1) %array"
            },
        )
        .replace(
            "  %remaining =",
            &format!(
                "  %advance = add i64 %position, {}\n  %remaining =",
                u64::from(stride != 0)
            ),
        )
}

fn marks(function: FunctionValue<'_>, expected: usize, batched: bool) {
    let mut count = 0;
    for block in function.get_basic_blocks() {
        for instruction in block.get_instructions() {
            if instruction.get_opcode() == InstructionOpcode::Store
                && instruction.get_atomic_ordering() == Ok(AtomicOrdering::Monotonic)
            {
                count += 1;
                assert_eq!(
                    block.get_name().to_str().unwrap().starts_with("card.exit"),
                    batched
                );
                if batched {
                    let address = instruction
                        .get_operand(1)
                        .unwrap()
                        .value()
                        .unwrap()
                        .as_instruction_value()
                        .unwrap();
                    let index = address
                        .get_operand(1)
                        .unwrap()
                        .value()
                        .unwrap()
                        .into_int_value();
                    assert_eq!(index.get_type().get_bit_width(), 64);
                }
            }
            if instruction.get_opcode() == InstructionOpcode::Store
                && instruction
                    .get_operand(1)
                    .unwrap()
                    .value()
                    .unwrap()
                    .into_pointer_value()
                    .get_type()
                    .get_address_space()
                    == inkwell::AddressSpace::from(1)
            {
                assert!(!block.get_name().to_str().unwrap().starts_with("card."));
            }
        }
    }
    assert_eq!(count, expected);
}

#[test]
fn constant_strides_finish_on_the_normal_exit_before_the_outer_poll() {
    let source = strided(super::source(), 8);
    let two_streams = source.replace(
        "  %scaled =",
        "  %other = getelementptr i8, ptr addrspace(1) %element, i64 8\n\
         store ptr addrspace(1) %array, ptr addrspace(1) %other, align 8\n\
         %other_written = ptrtoint ptr addrspace(1) %other to i64\n\
         %other_offset = sub i64 %other_written, %base\n\
         %other_index = lshr i64 %other_offset, 9\n\
         %other_card = getelementptr i8, ptr %cards, i64 %other_index\n\
         store atomic i8 1, ptr %other_card monotonic, align 1\n\
         %scaled =",
    );
    let conditional = source
        .replace(
            "  %array =",
            "  %choice = icmp eq i32 %i, 3\n\
         br i1 %choice, label %negative, label %positive\n\
         negative:\n  br label %latch\n\
         positive:\n  br label %latch\n\
         latch:\n  %array =",
        )
        .replace(", %inner]", ", %latch]");
    let guarded = source.replace(
        "chunk:\n  br label %inner",
        "chunk:\n  %enter = icmp ne i32 %count, 1\n  br i1 %enter, label %inner, label %exit",
    );
    let outer_offset = strided(super::source(), 0)
        .replace(
            "outer:\n",
            "outer:\n  %offset = phi i64 [0, %entry], [%offset, %inner]\n",
        )
        .replace("%offset = sub i64 %written, %base\n", "");
    for (source, count) in [
        (strided(super::source(), 0), 1),
        (source.clone(), 2),
        (strided(super::source(), 16), 3),
        (source.replace("[64, %chunk]", "[2, %chunk]"), 2),
        (two_streams, 4),
        (conditional, 2),
        (guarded, 2),
        (outer_offset, 1),
    ] {
        let context = Context::create();
        let llvm = parse(&context, &source);
        let machine = ValidatedBackendProfile::darwin_aarch64_for_test()
            .create_target_machine()
            .unwrap();
        let function = llvm.get_function("f").unwrap();
        crate::statepoint::bounded_loops::optimize(function, &machine).unwrap();
        llvm.verify().unwrap();
        marks(function, count, true);
        let exit = function
            .get_basic_blocks()
            .into_iter()
            .find(|block| block.get_name().to_str().unwrap() == "card.exit")
            .unwrap();
        let successor = exit
            .get_terminator()
            .unwrap()
            .get_operand(0)
            .unwrap()
            .block()
            .unwrap();
        assert_eq!(successor.get_name().to_str().unwrap(), "outer");
        for phi in successor
            .get_instructions()
            .take_while(|instruction| instruction.get_opcode() == InstructionOpcode::Phi)
        {
            assert!(
                PhiValue::try_from(phi)
                    .unwrap()
                    .get_incomings()
                    .any(|(_, incoming)| incoming == exit)
            );
        }
        let ir = llvm.print_to_string().to_string();
        assert!(!ir.contains("llvm.loop.vectorize.enable"), "{ir}");
    }
}

#[test]
fn uncertain_addresses_and_incomplete_or_synchronizing_loops_keep_each_mark() {
    let source = strided(super::source(), 8);
    let conditional = source
        .replace(
            "  %array =",
            "  %choice = icmp eq i32 %i, 3\n\
         br i1 %choice, label %write, label %latch\n\
         write:\n  %array =",
        )
        .replace("  %scaled =", "  br label %latch\nlatch:\n  %scaled =")
        .replace(", %inner]", ", %latch]");
    for source in [
        super::source(),
        conditional,
        source.replace("[64, %chunk]", "[1, %chunk]"),
        source.replace("[64, %chunk]", "[65, %chunk]"),
        source.replace("[64, %chunk]", "[%count, %chunk]"),
        source.replace("%advance = add i64 %position, 1", "%advance = sub i64 %position, 1"),
        source.replace("%advance = add i64 %position, 1", "%advance = add i64 %position, 4"),
        source.replace("%wide = add i64 %position, 0", "%wide = and i64 %position, 3"),
        source.replace("%wide = add i64 %position, 0", "%wide = mul i64 %position, %position"),
        source.replace("%array = load ptr", "%array = load volatile ptr"),
        source.replace("  %scaled =", "  call void @tick()\n  %scaled ="),
        source.replace("  %scaled =", "  store atomic ptr addrspace(1) %root, ptr addrspace(1) %element release, align 8\n  %scaled ="),
        source.replace("  %scaled =", "  store ptr addrspace(1) %root, ptr %slot, align 8\n  %scaled ="),
    ] {
        let context = Context::create();
        let llvm = parse(&context, &source);
        let machine = ValidatedBackendProfile::darwin_aarch64_for_test().create_target_machine().unwrap();
        let function = llvm.get_function("f").unwrap();
        crate::statepoint::bounded_loops::optimize(function, &machine).unwrap();
        llvm.verify().unwrap();
        marks(function, 1, false);
    }
}

#[test]
fn batched_cards_keep_moving_roots_and_actual_stack_reloads_on_each_target() {
    for selection in [
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_MUSL_LLVM_22_1,
    ] {
        for (source, reloads) in [(super::source(), 0), (stack_reads::source(), 2)] {
            let source = strided(source, 16);
            let profile = ValidatedBackendProfile::for_selection(selection)
                .unwrap()
                .with_optimization(OptimizationMode::Release);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(&context, &source);
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let mut expected = manifest(Some((7, one_root())));
            expected.sites.insert(
                8,
                ExpectedSite {
                    function: "f".into(),
                    block: "outer".into(),
                    statepoint: one_root(),
                },
            );
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            assert_eq!(plan.site_count(), 2);
            assert_eq!(plan.root_count(7), Some(1));
            assert_eq!(plan.root_count(8), Some(1));
            llvm.run_passes(
                "function(verify<safepoint-ir>),verify",
                &machine,
                PassBuilderOptions::create(),
            )
            .unwrap();
            let batched = reloads == 0 || selection == scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
            marks(
                llvm.get_function("f").unwrap(),
                if batched { 3 } else { 1 },
                batched,
            );
            stack_reads::assert_retained(llvm.get_function("f").unwrap(), reloads);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("batches.o");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            assert_machine_site_count(&path, 2);
        }
    }
}
