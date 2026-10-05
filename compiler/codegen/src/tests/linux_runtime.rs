use std::path::{Path, PathBuf};
use std::process::Command;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetTriple,
};

mod images;
mod sha256;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

fn checked(command: &mut Command) -> std::process::Output {
    let output = command.output().expect("execute Linux runtime probe");
    assert!(
        output.status.success(),
        "{command:?}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn emit_frame_probe(workspace: &Path, object: &Path, triple: &str, opt: OptimizationLevel) {
    Target::initialize_x86(&InitializationConfig::default());
    let target_triple = TargetTriple::create(triple);
    let machine = Target::from_triple(&target_triple)
        .unwrap()
        .create_target_machine(
            &target_triple,
            "x86-64",
            "",
            opt,
            RelocMode::PIC,
            CodeModel::Small,
        )
        .unwrap();
    let context = Context::create();
    let source =
        MemoryBuffer::create_from_file(&workspace.join("runtime/tests/linux_frames_test.ll"))
            .unwrap();
    let module = context.create_module_from_ir(source).unwrap();
    module.set_triple(&target_triple);
    module.set_data_layout(&machine.get_target_data().get_data_layout());
    module.verify().unwrap();
    machine
        .write_to_file(&module, FileType::Object, object)
        .unwrap();
    // Function addresses require loader relocations in PIE. This test retains
    // the stackmap blob in writable data; image-permission tests use RELRO.
    checked(
        Command::new("objcopy")
            .args([
                "--set-section-flags",
                ".llvm_stackmaps=alloc,load,data,contents",
            ])
            .arg(object),
    );
}

#[test]
fn linux_amd64_anchors_relocate_real_llvm_roots_and_preserve_sret() {
    let workspace = workspace();
    let root = workspace.join("target/linux-runtime-tests");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let script = directory.path().join("stackmaps.ld");
    std::fs::write(
        &script,
        "SECTIONS { .test_stackmaps : ALIGN(8) { test_stackmaps_start = .; KEEP(*(.llvm_stackmaps)) test_stackmaps_end = .; } } INSERT AFTER .data;\n",
    )
    .unwrap();
    for (libc, compiler, mode, flags) in [
        ("gnu", "gcc", "dynamic", ["-fPIE", "-pie"]),
        ("musl", "musl-gcc", "static", ["-static", "-no-pie"]),
        ("musl", "musl-gcc", "dynamic", ["-fPIE", "-pie"]),
    ] {
        for (level, optimization) in [
            ("0", OptimizationLevel::None),
            ("2", OptimizationLevel::Default),
        ] {
            let object = directory
                .path()
                .join(format!("frames-{libc}-{mode}-{level}.o"));
            emit_frame_probe(
                &workspace,
                &object,
                &format!("x86_64-unknown-linux-{libc}"),
                optimization,
            );
            let binary = object.with_extension("exe");
            let mut command = Command::new(compiler);
            command
                .env("TMPDIR", directory.path())
                .args([
                    "-std=c11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-pthread",
                    "-ffunction-sections",
                    "-fdata-sections",
                    "-fno-omit-frame-pointer",
                    "-fno-optimize-sibling-calls",
                    "-Wl,--gc-sections",
                    "-Wl,-z,noexecstack",
                ])
                .arg(format!("-O{level}"))
                .args(flags)
                .arg("-I")
                .arg(workspace.join("runtime/include"))
                .arg("-Wl,-T")
                .arg(&script)
                .arg(&object);
            for source in [
                "runtime/tests/linux_frames_test.c",
                "runtime/src/platform/arch/x86_64.c",
                "runtime/src/platform/arch/x86_64_anchor.S",
                "runtime/src/platform/arch/x86_64_strings.c",
                "runtime/src/platform/os/linux.c",
                "runtime/src/strings.c",
                "runtime/src/utf8.c",
                "runtime/src/gc/stackmap.c",
                "runtime/src/gc/stackmap/parser.c",
                "runtime/src/gc/stackmap/records.c",
            ] {
                command.arg(workspace.join(source));
            }
            checked(command.arg("-o").arg(&binary));
            let output = checked(&mut Command::new(&binary));
            assert_eq!(
                output.stdout,
                b"amd64 anchors, relocated roots, String sret and Linux VM passed\n"
            );
        }
    }
}

#[test]
fn linux_thread_publications_refresh_growing_main_stack_bounds() {
    let workspace = workspace();
    let root = workspace.join("target/linux-runtime-tests");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    for (compiler, flags) in [
        ("gcc", ["-fPIE", "-pie"]),
        ("musl-gcc", ["-static", "-no-pie"]),
        ("musl-gcc", ["-fPIE", "-pie"]),
    ] {
        for optimization in ["-O0", "-O2"] {
            let binary = directory.path().join("growth");
            let mut command = Command::new(compiler);
            command
                .env("TMPDIR", directory.path())
                .args([
                    "-std=c11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-pthread",
                    "-fno-omit-frame-pointer",
                    "-fno-optimize-sibling-calls",
                    optimization,
                ])
                .args(flags)
                .arg("-I")
                .arg(workspace.join("runtime/include"));
            for source in [
                "runtime/tests/linux_stack_growth_test.c",
                "runtime/src/platform/os/linux.c",
                "runtime/src/thread.c",
                "runtime/src/thread/collection.c",
                "runtime/src/thread/transitions.c",
                "runtime/src/thread/roots.c",
            ] {
                command.arg(workspace.join(source));
            }
            checked(command.arg("-o").arg(&binary));
            let output = checked(&mut Command::new(binary));
            assert_eq!(
                output.stdout,
                b"Linux stack growth across managed, native and callback entries passed\n"
            );
        }
    }
}

#[test]
fn shared_stackmap_decoder_keeps_darwin_frame_alignment_in_its_adapter() {
    let workspace = workspace();
    let root = workspace.join("target/linux-runtime-tests");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let binary = directory.path().join("stackmap");
    let mut command = Command::new("gcc");
    command
        .env("TMPDIR", directory.path())
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"]);
    for source in [
        "runtime/tests/stackmap_test.c",
        "runtime/src/platform/arch/aarch64.c",
        "runtime/src/gc/stackmap.c",
        "runtime/src/gc/stackmap/parser.c",
        "runtime/src/gc/stackmap/records.c",
    ] {
        command.arg(workspace.join(source));
    }
    checked(command.arg("-o").arg(&binary));
    checked(&mut Command::new(binary));
}
