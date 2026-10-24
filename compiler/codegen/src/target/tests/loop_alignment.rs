use super::*;
use inkwell::{context::Context, memory_buffer::MemoryBuffer, targets::FileType};
use object::{Object, ObjectSection};

#[test]
fn small_x86_inner_loops_keep_a_32_byte_entry_without_in_loop_padding() {
    // This prefix puts the default 16-byte loop entry halfway through a fetch
    // block. The actual vector load, reduction, and backedge must fit together.
    let source = r#"
declare i64 @llvm.vector.reduce.add.v2i64(<2 x i64>)
define i64 @f(ptr %values) {
entry:
  call void asm sideeffect ".p2align 4\0A.space 1, 0x90", ""()
  br label %loop
loop:
  %i = phi i64 [0, %entry], [%next, %loop]
  %sum = phi <2 x i64> [zeroinitializer, %entry], [%added, %loop]
  %address = getelementptr i64, ptr %values, i64 %i
  %value = load <2 x i64>, ptr %address, align 8
  %added = add <2 x i64> %value, %sum
  %next = add i64 %i, 2
  %more = icmp ne i64 %next, 64
  br i1 %more, label %loop, label %done
done:
  %total = call i64 @llvm.vector.reduce.add.v2i64(<2 x i64> %added)
  ret i64 %total
}
"#;
    for selection in [
        scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_MUSL_LLVM_22_1,
    ] {
        let profile = ValidatedBackendProfile::for_selection(selection)
            .unwrap()
            .with_optimization(scoop_lir::OptimizationMode::Release);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let llvm = context
            .create_module_from_ir(MemoryBuffer::create_from_memory_range_copy(
                source.as_bytes(),
                "loop-alignment",
            ))
            .unwrap();
        llvm.set_triple(&machine.get_triple());
        llvm.set_data_layout(&machine.get_target_data().get_data_layout());
        let buffer = machine
            .write_to_memory_buffer(&llvm, FileType::Object)
            .unwrap();
        let object = object::File::parse(buffer.as_slice()).unwrap();
        let text = object.section_by_name(".text").unwrap();
        assert!(text.align() >= 32);
        let bytes = text.data().unwrap();
        let entry = bytes
            .windows(3)
            .position(|instruction| instruction == [0xf3, 0x0f, 0x6f])
            .expect("the loop starts with movdqu");
        let length = bytes[entry..]
            .windows(2)
            .position(|branch| branch[0] == 0x75 && branch[1] >= 0x80)
            .expect("short backwards jne")
            + 2;
        assert_eq!(entry % 32, 0);
        assert!(length < 32, "short loop length {length}");
    }
}
