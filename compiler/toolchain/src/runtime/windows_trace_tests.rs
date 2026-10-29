use super::windows_tests::{checked, tool};
use crate::{FinalLinkOptions, WindowsFinalLinkProfile, resolve_windows_c_toolchain};
use std::path::Path;
use std::process::Command;

#[test]
fn windows_traces_keep_pdb_lines_unicode_paths_and_historical_module_addresses() {
    let directory = tempfile::Builder::new()
        .prefix("scoop trace 中文 ")
        .tempdir()
        .unwrap();
    let clang = tool("SCOOP_TEST_CLANG");
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime");
    let test = directory.path().join("检查 frames.c");
    std::fs::copy(runtime.join("tests/windows_trace_test.c"), &test).unwrap();
    let library_source = directory.path().join("library.c");
    std::fs::write(&library_source, "void trace_library_marker(void) {}\n").unwrap();
    let library_object = directory.path().join("library.obj");
    checked(
        Command::new(&clang)
            .args(["--target=x86_64-pc-windows-msvc", "-c"])
            .arg(&library_source)
            .arg("-o")
            .arg(&library_object),
    );
    let library = directory.path().join("trace library.dll");
    checked(
        Command::new(clang.parent().unwrap().join("lld-link.exe"))
            .args(["/dll", "/noentry", "/export:trace_library_marker"])
            .arg(format!("/out:{}", library.display()))
            .arg(library_object),
    );
    for (index, (target, mingw, unwind)) in [
        (
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
            None,
            "SCOOP_TEST_UNWIND_MSVC",
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT,
            Some("SCOOP_TEST_MINGW_MSVCRT"),
            "SCOOP_TEST_UNWIND_MSVCRT",
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_UCRT,
            Some("SCOOP_TEST_MINGW_UCRT"),
            "SCOOP_TEST_UNWIND_UCRT",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let prefix = mingw.map(|name| tool(name).parent().unwrap().parent().unwrap().to_owned());
        let startup = resolve_windows_c_toolchain(target, &clang, prefix.as_deref()).unwrap();
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
        for optimization in ["-O0", "-O2"] {
            let objects: Vec<_> = ["capture.c", "render.c", "windows.c", "windows_symbols.c"]
                .map(|name| runtime.join("src/stack_trace").join(name))
                .into_iter()
                .chain([test.clone()])
                .enumerate()
                .map(|(part, source)| {
                    let object = directory
                        .path()
                        .join(format!("{index}-{optimization}-{part}.obj"));
                    checked(
                        profile
                            .startup_toolchain()
                            .object_compilation_command(&source, &object)
                            .args([
                                "-Wall",
                                "-Wextra",
                                "-Werror",
                                "-g",
                                "-gcodeview",
                                "-D__SEH__",
                                "-D_CRT_SECURE_NO_WARNINGS",
                                "-fno-omit-frame-pointer",
                                "-fno-optimize-sibling-calls",
                                optimization,
                            ])
                            .arg("-I")
                            .arg(runtime.join("src"))
                            .arg("-I")
                            .arg(tool(unwind).join("include")),
                    );
                    object
                })
                .collect();
            for debug in [true, false] {
                let binary = directory
                    .path()
                    .join(format!("trace-{index}-{optimization}-{debug}.exe"));
                let pdb = binary.with_extension("pdb");
                let mut command = profile.command(directory.path(), &binary).unwrap();
                command.args(&objects);
                profile.append_system_libraries(&mut command);
                profile.configure_outputs(&mut command, debug.then_some(pdb.as_path()), false);
                checked(&mut command);
                checked(Command::new(binary).arg(&library).arg(&test).arg(if debug {
                    "debug"
                } else {
                    "addresses"
                }));
            }
        }
    }
}
