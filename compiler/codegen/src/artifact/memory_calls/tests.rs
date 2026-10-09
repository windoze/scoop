use super::*;
use inkwell::{context::Context, memory_buffer::MemoryBuffer, targets::FileType};
use scoop_lir::{OptimizationMode, TargetProfileId, ValidatedLirTargetSelection};

#[test]
fn copy_and_zero_libcalls_are_distinct_from_an_ordinary_external_call() {
    let source = br#"
        declare ptr @memcpy(ptr, ptr, i64)
        declare ptr @memset(ptr, i32, i64)
        declare void @ordinary()
        define void @memory_helpers(ptr %out, ptr %source, i64 %size) {
            call ptr @memcpy(ptr %out, ptr %source, i64 %size)
            call ptr @memset(ptr %out, i32 0, i64 %size)
            call void @ordinary()
            ret void
        }
    "#;
    for target in [
        TargetProfileId::DarwinAarch64,
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = crate::ValidatedBackendProfile::from_selection(
                ValidatedLirTargetSelection::from_id(target),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let buffer = MemoryBuffer::create_from_memory_range_copy(source, "memory_helpers");
            let module = context.create_module_from_ir(buffer).unwrap();
            module.set_triple(&machine.get_triple());
            module.set_data_layout(&machine.get_target_data().get_data_layout());
            let bytes = machine
                .write_to_memory_buffer(&module, FileType::Object)
                .unwrap();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            for helper in ["memcpy", "memset", "ordinary"] {
                let name = if target == TargetProfileId::DarwinAarch64 {
                    format!("_{helper}")
                } else {
                    helper.into()
                };
                assert!(file.symbols().any(|symbol| {
                    symbol.is_undefined() && symbol.name().ok() == Some(name.as_str())
                }));
            }
            let calls = file
                .sections()
                .filter(|section| section.kind() == object::SectionKind::Text)
                .map(|section| non_unwinding_calls(&file, &section).unwrap().len())
                .sum::<usize>();
            assert_eq!(calls, 2, "{target:?}/{mode:?}");
        }
    }
}
