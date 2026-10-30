use super::*;

pub(super) fn source() -> String {
    super::source()
        .replace(
            "  %slot = alloca",
            "  %first = alloca [2 x ptr addrspace(1)], align 8\n\
             %second = alloca ptr addrspace(1), align 8\n\
             %slot = alloca",
        )
        .replace(
            "store ptr addrspace(1) %array, ptr addrspace(1) %element, align 8",
            "  %view_field = getelementptr [2 x ptr addrspace(1)], ptr %first, i64 0, i64 1\n\
             store ptr addrspace(1) %array, ptr %view_field, align 8\n\
             store ptr addrspace(1) %array, ptr %second, align 8\n\
             %choose = icmp eq i32 %i, 3\n\
             %selected = select i1 %choose, ptr %view_field, ptr %second\n\
             %read = load volatile ptr addrspace(1), ptr %selected, align 8\n\
             store ptr addrspace(1) %read, ptr %view_field, align 8\n\
             %read_again = load volatile ptr addrspace(1), ptr %selected, align 8\n\
             store ptr addrspace(1) %read_again, ptr addrspace(1) %element, align 8",
        )
}

pub(super) fn assert_retained(function: inkwell::values::FunctionValue<'_>, expected: usize) {
    let mut reads = Vec::new();
    for block in function.get_basic_blocks() {
        for instruction in block.get_instructions() {
            if instruction.get_opcode() == InstructionOpcode::Load
                && instruction.get_volatile() == Ok(true)
                && matches!(
                    instruction.get_name().map(|name| name.to_bytes()),
                    Some(b"read" | b"read_again")
                )
            {
                reads.push(instruction.get_name().unwrap().to_str().unwrap().to_owned());
                assert!(!matches!(
                    block.get_name().to_str().unwrap(),
                    "chunk" | "card.preheader"
                ));
            }
        }
    }
    assert_eq!(reads.len(), expected);
    if expected == 2 {
        assert_eq!(reads, ["read", "read_again"]);
    }
}

#[test]
fn only_stack_reference_reads_allow_independent_mapping_queries_to_move() {
    let original = source();
    let phi = original
        .replace(
            "%selected = select i1 %choose, ptr %view_field, ptr %second",
            "br i1 %choose, label %select_left, label %select_right\n\
             select_left:\n  br label %select_merge\n\
             select_right:\n  br label %select_merge\n\
             select_merge:\n\
             %selected = phi ptr [%view_field, %select_left], [%second, %select_right]",
        )
        .replace(", %inner]", ", %select_merge]");
    let cases = [
        (original.clone(), true),
        (phi, true),
        (
            original.replace(
                "ptr %selected, align 8",
                "ptr addrspace(1) %element, align 8",
            ),
            false,
        ),
        (
            original.replace("ptr %view_field, ptr %second", "ptr @external, ptr %second"),
            false,
        ),
        (
            original.replace("ptr %selected, align 8", "ptr %external, align 8"),
            false,
        ),
        (
            original.replace("i64 0, i64 1", "i64 0, i64 %dynamic"),
            false,
        ),
        (
            original.replace(
                "store ptr addrspace(1) %read, ptr %view_field",
                "store volatile ptr addrspace(1) %read, ptr %view_field",
            ),
            false,
        ),
        (
            original.replace(
                "load volatile ptr addrspace(1), ptr %selected, align 8",
                "load atomic volatile ptr addrspace(1), ptr %selected acquire, align 8",
            ),
            false,
        ),
        (
            original.replace(
                "%address = ptrtoint ptr addrspace(1) %array",
                "%address = ptrtoint ptr addrspace(1) %read_again",
            ),
            false,
        ),
    ];
    for (source, outside) in cases {
        let source = format!("@external = external global ptr addrspace(1)\n{source}")
            .replace("i32 %count)", "i32 %count, ptr %external)")
            .replace(
                "%view_field = getelementptr",
                "%dynamic = and i64 %wide, 1\n  %view_field = getelementptr",
            );
        let context = Context::create();
        let llvm = parse(&context, &source);
        let machine = ValidatedBackendProfile::darwin_aarch64_for_test()
            .create_target_machine()
            .unwrap();
        crate::statepoint::bounded_loops::optimize(
            llvm.get_function("f").unwrap(),
            &machine,
            ValidatedBackendProfile::darwin_aarch64_for_test().tuning(),
        )
        .unwrap();
        llvm.verify().unwrap();
        assert_queries(llvm.get_function("f").unwrap(), outside);
        assert_retained(llvm.get_function("f").unwrap(), 2);
    }
}
