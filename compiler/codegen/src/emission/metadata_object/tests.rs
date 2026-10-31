use super::*;
use crate::target::ValidatedBackendProfile;
use inkwell::context::Context;
use inkwell::targets::FileType;

#[test]
fn data_emission_matches_full_pipeline_on_every_target() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::DARWIN_X86_64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID,
    ] {
        for optimization in [
            scoop_lir::OptimizationMode::Debug,
            scoop_lir::OptimizationMode::Release,
        ] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(optimization);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let mut source = include_str!("data.ll").to_owned();
            if profile.lir_target_profile().native_object_format()
                != scoop_lir::NativeObjectFormat::MachO64
            {
                source.push_str(
                    "$shared = comdat any\n@shared = linkonce_odr constant i64 47, comdat\n",
                );
            }
            let module = context
                .create_module_from_ir(MemoryBuffer::create_from_memory_range_copy(
                    source.as_bytes(),
                    "metadata",
                ))
                .unwrap();
            module.set_triple(&machine.get_triple());
            module.set_data_layout(&machine.get_target_data().get_data_layout());
            module.verify().unwrap();
            let path = directory.path().join("metadata.o");
            // Exercise the data path before the C API initializes codegen passes.
            write(&machine, &module, &path).unwrap();
            let full = machine
                .write_to_memory_buffer(&module.clone(), FileType::Object)
                .unwrap();
            assert_eq!(
                std::fs::read(path).unwrap(),
                full.as_slice(),
                "{target:?}/{optimization:?}"
            );
        }
    }
}
