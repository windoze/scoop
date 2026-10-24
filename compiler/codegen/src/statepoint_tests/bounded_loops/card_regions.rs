use super::*;
use inkwell::AtomicOrdering;
use inkwell::values::InstructionOpcode;

#[path = "card_regions/stack_reads.rs"]
mod stack_reads;

fn query() -> String {
    let mut text = String::from(
        "  %array = load ptr addrspace(1), ptr %slot, align 8\n\
         %wide = sext i32 %i to i64\n\
         %element = getelementptr ptr addrspace(1), ptr addrspace(1) %array, i64 %wide\n\
         store ptr addrspace(1) %array, ptr addrspace(1) %element, align 8\n\
         %address = ptrtoint ptr addrspace(1) %array to i64\n",
    );
    let mut previous = "@scoop_gc_page_map".to_string();
    for (level, shift) in [52, 40, 28, 16].into_iter().enumerate() {
        text.push_str(&format!(
            "  %shift{level} = lshr i64 %address, {shift}\n\
             %index{level} = and i64 %shift{level}, 4095\n\
             %slot{level} = getelementptr ptr, ptr {previous}, i64 %index{level}\n\
             %page{level} = load atomic ptr, ptr %slot{level} acquire, align 8\n"
        ));
        previous = format!("%page{level}");
    }
    text.push_str(
        "  %base = load i64, ptr %page3, align 8\n\
         %cards_slot = getelementptr i8, ptr %page3, i64 16\n\
         %cards = load ptr, ptr %cards_slot, align 8\n\
         %written = ptrtoint ptr addrspace(1) %element to i64\n\
         %offset = sub i64 %written, %base\n\
         %card_index = lshr i64 %offset, 9\n\
         %card = getelementptr i8, ptr %cards, i64 %card_index\n\
         store atomic i8 1, ptr %card monotonic, align 1\n",
    );
    text
}

fn source() -> String {
    format!(
        "@scoop_gc_page_map = external global [4096 x ptr]\n{}",
        include_str!("../bounded_loops.ll").replace("  %scaled =", &(query() + "  %scaled ="))
    )
}

fn conditional(always: bool) -> String {
    let body = if always {
        format!(
            "  %choice = icmp eq i32 %i, 3\n\
             br i1 %choice, label %negative, label %positive\n\
             negative:\n  br label %latch\n\
             positive:\n  br label %latch\n\
             latch:\n{}",
            query()
        )
    } else {
        format!(
            "  %choice = icmp eq i32 %i, 3\n\
             br i1 %choice, label %write, label %latch\n\
             write:\n{}  br label %latch\n\
             latch:\n",
            query()
        )
    };
    source()
        .replace(&query(), &body)
        .replace(", %inner]", ", %latch]")
}

fn assert_queries(function: inkwell::values::FunctionValue<'_>, outside: bool) {
    let mut loads = 0;
    let mut stores = 0;
    for block in function.get_basic_blocks() {
        for instruction in block.get_instructions() {
            if instruction.get_opcode() == InstructionOpcode::Load
                && instruction.get_atomic_ordering() == Ok(AtomicOrdering::Acquire)
                && instruction.get_type().is_pointer_type()
                && instruction
                    .get_type()
                    .into_pointer_type()
                    .get_address_space()
                    == inkwell::AddressSpace::default()
            {
                loads += 1;
                let name = block.get_name().to_str().unwrap();
                assert_eq!(
                    matches!(name, "chunk" | "card.preheader"),
                    outside,
                    "{name}"
                );
            }
            if instruction.get_opcode() == InstructionOpcode::Store
                && instruction.get_atomic_ordering() == Ok(AtomicOrdering::Monotonic)
            {
                stores += 1;
                assert_ne!(block.get_name().to_str().unwrap(), "card.preheader");
                assert_ne!(block.get_name().to_str().unwrap(), "chunk");
            }
        }
    }
    assert_eq!(loads, 4);
    assert_eq!(stores, 1);
}

#[test]
fn page_queries_move_to_the_taken_entry_edge_and_keep_moving_roots() {
    for selection in [
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_MUSL_LLVM_22_1,
    ] {
        for (source, reloads) in [(source(), 0), (stack_reads::source(), 2)] {
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
            assert_queries(
                llvm.get_function("f").unwrap(),
                reloads == 0 || selection == scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
            );
            stack_reads::assert_retained(llvm.get_function("f").unwrap(), reloads);
            let ir = llvm.print_to_string().to_string();
            assert_eq!(
                ir.matches("store volatile ptr addrspace(1)").count(),
                2,
                "{ir}"
            );
            assert!(!ir.contains("llvm.loop.vectorize.enable"), "{ir}");
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("cards.o");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            assert_machine_site_count(&path, 2);
        }
    }
}

#[test]
fn only_unconditional_stable_queries_move_without_calls_or_synchronization() {
    let original = source();
    let cases = [
        (original.clone(), true),
        (conditional(true), true),
        (conditional(false), false),
        (original.replace("[64, %chunk]", "[65, %chunk]"), false),
        (original.replace("[64, %chunk]", "[%count, %chunk]"), false),
        (original.replace("%array = load ptr", "%array = load volatile ptr"), false),
        (original.replace("  %scaled =", "  call void @tick()\n  %scaled ="), false),
        (original.replace("  %scaled =", "  store atomic ptr addrspace(1) %root, ptr addrspace(1) %element release, align 8\n  %scaled ="), false),
        (original.replace("  %scaled =", "  store ptr addrspace(1) %root, ptr %slot, align 8\n  %scaled ="), false),
        (original.replace("%address = ptrtoint ptr addrspace(1) %array", "%address = ptrtoint ptr addrspace(1) %element"), false),
    ];
    for (source, outside) in cases {
        let context = Context::create();
        let llvm = parse(&context, &source);
        let machine = ValidatedBackendProfile::darwin_aarch64_for_test()
            .create_target_machine()
            .unwrap();
        crate::statepoint::bounded_loops::optimize(llvm.get_function("f").unwrap(), &machine)
            .unwrap();
        llvm.verify().unwrap();
        assert_queries(llvm.get_function("f").unwrap(), outside);
    }
}
