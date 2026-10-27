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

fn run_windows_runtime_test(test: &str, compile_sources: &[&str], linked_sources: &[&str]) {
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
        let objects: Vec<_> = linked_sources
            .iter()
            .copied()
            .chain([test])
            .enumerate()
            .map(|(part, source)| {
                let object = directory.path().join(format!("{index}-{part}.obj"));
                checked(
                    startup
                        .object_compilation_command(&runtime.join(source), &object)
                        .args(["-Wall", "-Wextra", "-Werror", "-O2"])
                        .arg("-I")
                        .arg(runtime.join("include"))
                        .arg("-I")
                        .arg(runtime.join("src")),
                );
                object
            })
            .collect();
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
        let binary = directory.path().join(format!("runtime-{index}.exe"));
        let mut command = profile.command(directory.path(), &binary).unwrap();
        checked(command.args(objects));
        checked(&mut Command::new(binary));
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
