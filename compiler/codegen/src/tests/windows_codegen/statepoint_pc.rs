use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::targets::FileType;
use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget};

#[test]
fn windows_statepoints_record_the_return_pc_before_seh_padding() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_LLVM_22_1,
    ] {
        for invoke in [false, true] {
            let operation = if invoke { "invoke" } else { "call" };
            let continuation = if invoke {
                "to label %done unwind label %landing\n\
                 done:\nret void\nlanding:\n\
                 %exception = landingpad { ptr, i32 } cleanup\n\
                 resume { ptr, i32 } %exception"
            } else {
                "\nret void"
            };
            let source = format!(
                "declare void @leaf()\n\
                 declare i32 @scoop_eh_personality_seh(...)\n\
                 declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)\n\
                 define void @caller() \"frame-pointer\"=\"all\" gc \"statepoint-example\" personality ptr @scoop_eh_personality_seh {{\n\
                 %site = {operation} token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 17, i32 0, ptr elementtype(void ()) @leaf, i32 0, i32 0, i32 0, i32 0) {continuation}\n}}"
            );
            let context = Context::create();
            let llvm = context
                .create_module_from_ir(MemoryBuffer::create_from_memory_range_copy(
                    source.as_bytes(),
                    "windows-return-pc",
                ))
                .unwrap();
            llvm.verify().unwrap();
            let machine = crate::ValidatedBackendProfile::for_selection(target)
                .unwrap()
                .create_target_machine()
                .unwrap();
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let path = directory.path().join("return-pc.obj");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            let bytes = std::fs::read(path).unwrap();
            let object = object::File::parse(bytes.as_slice()).unwrap();
            let caller = object.symbol_by_name("caller").unwrap();
            let code = object
                .section_by_index(caller.section_index().unwrap())
                .unwrap();
            let call_relocations = code
                .relocations()
                .filter_map(|(offset, relocation)| {
                    let RelocationTarget::Symbol(symbol) = relocation.target() else {
                        return None;
                    };
                    (object.symbol_by_index(symbol).unwrap().name().unwrap() == "leaf")
                        .then_some(offset)
                })
                .collect::<Vec<_>>();
            assert_eq!(call_relocations.len(), 1);
            let operand = call_relocations[0] as usize;
            let code_bytes = code.data().unwrap();
            assert_eq!(code_bytes[operand - 1], 0xe8); // CALL rel32.
            let return_pc = operand + 4;
            if !invoke {
                assert_eq!(code_bytes[return_pc], 0x90, "WinEH padding must remain");
            }

            let stackmaps = object
                .section_by_name(".llvm_stackmaps")
                .unwrap()
                .data()
                .unwrap();
            let word =
                |offset| u32::from_le_bytes(stackmaps[offset..offset + 4].try_into().unwrap());
            assert_eq!(word(4), 1); // One function.
            assert_eq!(word(12), 1); // One statepoint.
            let record = 16 + 24 + word(8) as usize * 8;
            let instruction_offset = word(record + 8) as usize;
            assert_eq!(
                instruction_offset + caller.address() as usize,
                return_pc,
                "Windows statepoint must precede WinEH NOP: {}, invoke={invoke}",
                target.target().id().canonical_name()
            );
        }
    }
}
