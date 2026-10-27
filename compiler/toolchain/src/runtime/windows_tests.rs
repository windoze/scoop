use crate::{FinalLinkOptions, WindowsFinalLinkProfile, resolve_windows_c_toolchain};
use scoop_process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tool(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} is required"))
}

#[test]
fn windows_runtime_primitives_support_real_parallel_workers() {
    run_windows_runtime_test(
        "tests/platform_primitives_test.c",
        &[
            "src/thread.c",
            "src/gc/mutator.c",
            "src/gc/mark_pool.c",
            "src/gc/mark_queue.c",
            "src/gc/evacuation_plan.c",
            "src/gc/old_slots.c",
            "src/thread/collection.c",
            "src/thread/transitions.c",
            "src/gc/time.c",
            "src/gc/heap.c",
            "src/gc_binding/roots.c",
            "src/initialization.c",
            "src/rt.c",
            "src/image/type_relations.c",
            "src/callback.c",
            "src/eh.c",
            "src/startup.c",
        ],
        &["src/platform/primitives/windows.c"],
    );
}

#[test]
fn windows_vm_keeps_alignment_reservations_and_reusable_pages() {
    run_windows_runtime_test(
        "tests/windows_vm_test.c",
        &["src/gc/regions.c", "src/gc/memory_return.c"],
        &[
            "src/platform/primitives/windows.c",
            "src/platform/os/windows.c",
        ],
    );
}

#[test]
fn windows_numeric_parsing_rounds_directly_and_restores_native_locales() {
    run_windows_runtime_test(
        "tests/windows_numeric_test.c",
        &[],
        &[
            "src/floating_parse.c",
            "src/platform/primitives/windows.c",
            "src/platform/numeric/windows.c",
            "src/platform/arch/x86_64_floating.c",
            "src/platform/arch/x86_64_float_environment.c",
            "../sysroot/vendor/gdtoa/strtodg.c",
            "../sysroot/vendor/gdtoa/strtof.c",
            "../sysroot/vendor/gdtoa/strtodnrp.c",
            "../sysroot/vendor/gdtoa/misc.c",
            "../sysroot/vendor/gdtoa/dmisc.c",
            "../sysroot/vendor/gdtoa/smisc.c",
            "../sysroot/vendor/gdtoa/gmisc.c",
            "../sysroot/vendor/gdtoa/sum.c",
            "../sysroot/vendor/gdtoa/ulp.c",
            "../sysroot/vendor/gdtoa/gethex.c",
            "../sysroot/vendor/gdtoa/hexnan.c",
            "../sysroot/vendor/gdtoa/hd_init.c",
        ],
    );
}

#[test]
fn windows_seh_frames_relocate_real_roots_through_entry_anchors() {
    run_windows_runtime_test_with_optimizations(
        "tests/windows_frames_test.c",
        &[],
        &[
            "src/platform/profiles/windows_x86_64.c",
            "src/platform/arch/windows_x86_64.c",
            "src/platform/arch/windows_x86_64_anchor.S",
            "src/platform/primitives/windows.c",
            "src/platform/os/windows.c",
            "src/platform/image/pe.c",
            "src/image/ranges.c",
            "src/gc_binding/stackmap.c",
            "src/gc_binding/stackmap/parser.c",
            "src/gc_binding/stackmap/records.c",
            "src/gc_binding/stackmap/payload.c",
            "tests/windows_frames_test.ll",
        ],
        &["-O0", "-O2"],
    );
}

fn run_windows_runtime_test(test: &str, compile_sources: &[&str], linked_sources: &[&str]) {
    run_windows_runtime_test_with_optimizations(test, compile_sources, linked_sources, &["-O2"]);
}

fn run_windows_runtime_test_with_optimizations(
    test: &str,
    compile_sources: &[&str],
    linked_sources: &[&str],
    optimizations: &[&str],
) {
    let directory = tempfile::Builder::new()
        .prefix("scoop runtime 同步 ")
        .tempdir()
        .unwrap();
    let clang = tool("SCOOP_TEST_CLANG");
    let msvcrt = tool("SCOOP_TEST_MINGW_MSVCRT");
    let ucrt = tool("SCOOP_TEST_MINGW_UCRT");
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime");
    for (index, (target, prefix, unwind)) in [
        (
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
            None,
            "SCOOP_TEST_UNWIND_MSVC",
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT,
            msvcrt.parent().and_then(Path::parent),
            "SCOOP_TEST_UNWIND_MSVCRT",
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_UCRT,
            ucrt.parent().and_then(Path::parent),
            "SCOOP_TEST_UNWIND_UCRT",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let startup = resolve_windows_c_toolchain(target, &clang, prefix).unwrap();
        for (part, source) in compile_sources.iter().enumerate() {
            checked(
                startup
                    .object_compilation_command(
                        &runtime.join(source),
                        &directory.path().join(format!("runtime-{index}-{part}.obj")),
                    )
                    .args([
                        "-Wall",
                        "-Wextra",
                        "-Werror",
                        "-O2",
                        "-D__SEH__",
                        "-D_CRT_SECURE_NO_WARNINGS",
                        "-DSCOOP_ENABLE_STARTUP_STATS=1",
                    ])
                    .arg("-I")
                    .arg(runtime.join("include"))
                    .arg("-I")
                    .arg(runtime.join("src"))
                    .arg("-I")
                    .arg(tool(unwind).join("include")),
            );
        }
        let profile = WindowsFinalLinkProfile::resolve(
            startup,
            &FinalLinkOptions {
                sysroot: Some(directory.path().to_owned()),
                unwind_prefix: Some(tool(unwind)),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let startup = profile.startup_toolchain();
        for optimization in optimizations {
            let objects: Vec<_> = linked_sources
                .iter()
                .copied()
                .chain([test])
                .enumerate()
                .map(|(part, source)| {
                    let object = directory
                        .path()
                        .join(format!("{index}-{optimization}-{part}.obj"));
                    let source = runtime.join(source);
                    let mut command =
                        if source.extension().is_some_and(|extension| extension == "c") {
                            let mut command = startup.object_compilation_command(&source, &object);
                            command
                                .args([
                                    "-Wall",
                                    "-Wextra",
                                    "-Werror",
                                    "-fno-omit-frame-pointer",
                                    "-fno-optimize-sibling-calls",
                                    "-D_CRT_SECURE_NO_WARNINGS",
                                ])
                                .arg("-I")
                                .arg(runtime.join("include"))
                                .arg("-I")
                                .arg(runtime.join("src"));
                            command
                        } else {
                            let mut command = startup.driver_command();
                            command
                                .args([
                                    "-c",
                                    "-Werror",
                                    "-Wno-override-module",
                                    "-Wno-unused-command-line-argument",
                                ])
                                .arg(&source)
                                .arg("-o")
                                .arg(&object);
                            command
                        };
                    checked(command.arg(optimization));
                    object
                })
                .collect();
            let binary = directory
                .path()
                .join(format!("runtime-{index}-{optimization}.exe"));
            let mut command = profile.command(directory.path(), &binary).unwrap();
            for argument in ["/merge:.llvm_stackmaps=.smap", "/section:.smap,R"] {
                profile.linker_arg(
                    &mut command,
                    if profile.microsoft() {
                        argument.to_owned()
                    } else {
                        format!("--Xlink={argument}")
                    },
                );
            }
            checked(command.args(objects));
            checked(&mut Command::new(binary));
        }
    }
}

fn checked(command: &mut Command) {
    let output = command.scoop_output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
