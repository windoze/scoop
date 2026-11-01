use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::targets::FileType;

#[cfg(windows)]
use std::path::Path;

use super::*;

#[test]
fn windows_mingw_address_pointers_join_callable_constants() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_LLVM_22_1,
    ] {
        for mixed in [false, true] {
            let source = format!(
                "@first = external global i64\n@second = external global i64\n\
                 define ptr @get_data(i32 %which) {{\n\
                 %pick = icmp ne i32 %which, 0\n\
                 %pointer = select i1 %pick, ptr @first, ptr @second\n\
                 ret ptr %pointer\n}}\n{}",
                if mixed {
                    "define double @add(double %value) {\n\
                     %sum = fadd double %value, 1.5\nret double %sum\n}\n"
                } else {
                    ""
                }
            );
            let context = Context::create();
            let llvm = context
                .create_module_from_ir(MemoryBuffer::create_from_memory_range_copy(
                    source.as_bytes(),
                    "refptr",
                ))
                .unwrap();
            let machine = crate::ValidatedBackendProfile::for_selection(target)
                .unwrap()
                .create_target_machine()
                .unwrap();
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let path = directory.path().join("refptr.obj");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            let mut bytes = std::fs::read(&path).unwrap();
            merge_callable_constants(&mut bytes).unwrap();
            let mut object = CoffObject::read(&bytes).unwrap();
            let constants = object.file.section_by_name(".rdata").unwrap();
            assert_eq!(constants.relocations().count(), 2);
            assert!(constants.size() >= 16 && constants.align() >= 8);
            let constant_index = constants.index();
            let pointers = object
                .file
                .symbols()
                .filter(|symbol| symbol.name().unwrap().starts_with(".refptr."))
                .collect::<Vec<_>>();
            assert_eq!(pointers.len(), 2);
            for pointer in pointers {
                assert!(pointer.is_local());
                assert_eq!(pointer.section_index(), Some(constant_index));
            }
            let (code, _) = object.extent("get_data").unwrap();
            object
                .associate(code, &[constant_index], LinkageClass::ConeStrong)
                .unwrap();
            std::fs::write(&path, object.finish().unwrap()).unwrap();

            #[cfg(windows)]
            check_native(&path, target, mixed);
        }
    }
}

#[cfg(windows)]
fn check_native(path: &Path, target: scoop_lir::ValidatedLirTargetSelection, mixed: bool) {
    link_and_run(
        path,
        target,
        &format!(
            "#include <stdint.h>\n\
             int64_t first = 21, second = 42;\n\
             extern int64_t *get_data(int);\nextern double add(double);\n\
             int main(void) {{ return get_data(1) != &first || get_data(0) != &second{}; }}\n",
            if mixed { " || add(0.5) != 2.0" } else { "" }
        ),
    );
}

#[test]
fn mingw_jump_table_and_address_pointer_survive_constant_merging() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_LLVM_22_1,
    ] {
        let source = "@first = external global i64\n\
            define i64 @pick(i32 %which) {\n\
            %value = load volatile i64, ptr @first\n\
            switch i32 %which, label %bad [i32 0, label %a\n\
                i32 1, label %b\ni32 2, label %c\ni32 3, label %d]\n\
            a: %sum = add i64 %value, 3\nret i64 %sum\n\
            b: %bits = xor i64 %value, 17\nret i64 %bits\n\
            c: %product = mul i64 %value, 7\nret i64 %product\n\
            d: %difference = sub i64 %value, 2\nret i64 %difference\n\
            bad: unreachable\n}\n";
        let context = Context::create();
        let llvm = context
            .create_module_from_ir(MemoryBuffer::create_from_memory_range_copy(
                source.as_bytes(),
                "refptr-switch",
            ))
            .unwrap();
        let comdat = llvm.get_or_insert_comdat("pick");
        comdat.set_selection_kind(inkwell::comdat::ComdatSelectionKind::NoDuplicates);
        llvm.get_function("pick")
            .unwrap()
            .as_global_value()
            .set_comdat(comdat);
        let machine = crate::ValidatedBackendProfile::for_selection(target)
            .unwrap()
            .with_optimization(scoop_lir::OptimizationMode::Release)
            .create_target_machine()
            .unwrap();
        llvm.set_triple(&machine.get_triple());
        llvm.set_data_layout(&machine.get_target_data().get_data_layout());
        let path = directory.path().join("refptr-switch.obj");
        machine
            .write_to_file(&llvm, FileType::Object, &path)
            .unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        merge_callable_constants(&mut bytes).unwrap();
        let mut object = CoffObject::read(&bytes).unwrap();
        let constants = object.file.section_by_name(".rdata").unwrap();
        assert_eq!(constants.relocations().count(), 5);
        let constant_index = constants.index();
        let (code, _) = object.extent("pick").unwrap();
        object
            .associate(code, &[constant_index], LinkageClass::ConeStrong)
            .unwrap();
        std::fs::write(&path, object.finish().unwrap()).unwrap();

        #[cfg(windows)]
        link_and_run(
            &path,
            target,
            "#include <stdint.h>\nint64_t first = 21;\nextern int64_t pick(int);\n\
             int main(void) {\n\
             const int64_t expected[] = {24, 4, 147, 19};\n\
             for (int i = 0; i < 4; ++i) if (pick(i) != expected[i]) return i + 1;\n\
             return 0;\n}\n",
        );
    }
}

#[cfg(windows)]
fn link_and_run(path: &Path, target: scoop_lir::ValidatedLirTargetSelection, source: &str) {
    let variable = if target == scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1 {
        "SCOOP_TEST_MINGW_MSVCRT"
    } else {
        "SCOOP_TEST_MINGW_UCRT"
    };
    let source_path = path.with_extension("c");
    let executable = path.with_extension("exe");
    std::fs::write(&source_path, source).unwrap();
    let linked = std::process::Command::new(std::env::var_os(variable).expect(variable))
        .arg(&source_path)
        .arg(path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(linked.status.success(), "{linked:?}");
    let ran = std::process::Command::new(executable).output().unwrap();
    assert!(ran.status.success(), "{ran:?}");
}
