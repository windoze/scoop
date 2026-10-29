use super::*;
use scoop_process::CommandExt;
use scoop_toolchain::{
    FinalLinkOptions, ValidatedRuntimeBuildProfile, WindowsFinalLinkProfile,
    resolve_windows_c_toolchain,
};
use std::path::{Path, PathBuf};
use std::process::Command;

fn tool(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} is required for Windows runtime validation"))
}

#[test]
fn windows_runtime_sources_define_and_link_the_complete_runtime_abi() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let clang = tool("SCOOP_TEST_CLANG");
    let msvcrt = tool("SCOOP_TEST_MINGW_MSVCRT");
    let ucrt = tool("SCOOP_TEST_MINGW_UCRT");
    for (target, prefix, unwind) in [
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
    ] {
        let invocation = resolve_windows_c_toolchain(target, &clang, prefix).unwrap();
        let profile = ValidatedRuntimeBuildProfile::for_target(target).unwrap();
        let directory = tempfile::Builder::new()
            .prefix("scoop runtime 全体 ")
            .tempdir()
            .unwrap();
        let unwind = tool(unwind);
        let unwind_include = scoop_toolchain::runtime_unwind_include(target, Some(&unwind))
            .unwrap()
            .unwrap();
        let mut objects = Vec::new();
        for (index, source) in profile.runtime_sources().into_iter().enumerate() {
            let object = directory.path().join(format!("runtime-{index}.obj"));
            let input = source
                .strip_prefix("vendor/")
                .map(|source| workspace.join("sysroot/vendor").join(source))
                .unwrap_or_else(|| workspace.join(source));
            let mut command = invocation.object_compilation_command(&input, &object);
            command
                .args(profile.runtime_c_flags())
                .args([
                    "-O2",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSCOOP_ENABLE_GC_STRESS=1",
                    "-DSCOOP_ENABLE_STARTUP_STATS=1",
                ])
                .arg("-I")
                .arg(&unwind_include)
                .arg("-I")
                .arg(workspace.join("sysroot/vendor/ryu"));
            for include in profile.include_directories() {
                command
                    .arg("-I")
                    .arg(workspace.join("runtime").join(include));
            }
            if input.extension().is_some_and(|extension| extension == "S") {
                command.arg("-Wno-unused-command-line-argument");
            }
            checked(&mut command);
            objects.push(object);
        }
        // The production object boundary checks every runtime ABI definition.
        let runtime = RuntimeObjectSet::from_paths(
            target,
            invocation.profile(),
            RuntimeBuildConfiguration {
                enable_gc_stress: true,
                enable_startup_stats: true,
                input_key: sha256(b"Windows complete runtime inputs"),
                compiler_inputs: sha256(b"selected Windows runtime compiler"),
                flags: profile
                    .runtime_c_flags()
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            },
            objects,
        )
        .unwrap();
        assert!(
            runtime
                .symbols()
                .definitions
                .contains_key("scoop_rt_run_program_utf16")
        );
        assert!(!runtime.symbols().definitions.contains_key("wmain"));
        let duplicate_source = directory.path().join("duplicate.c");
        let duplicate_object = directory.path().join("duplicate.obj");
        std::fs::write(
            &duplicate_source,
            "int scoop_rt_program_argc(void) { return 0; }\n",
        )
        .unwrap();
        checked(&mut invocation.object_compilation_command(&duplicate_source, &duplicate_object));
        let mut duplicate_paths: Vec<_> = runtime
            .objects()
            .iter()
            .map(|object| object.path().to_owned())
            .collect();
        duplicate_paths.push(duplicate_object);
        assert!(
            RuntimeObjectSet::from_paths(
                target,
                invocation.profile(),
                runtime.configuration().clone(),
                duplicate_paths
            )
            .unwrap_err()
            .0
            .contains("multiple definitions of scoop_rt_program_argc")
        );
        let index = runtime
            .write_index(&directory.path().join("index"))
            .unwrap();
        let restored = RuntimeObjectSet::read_index(&index, target, invocation.profile()).unwrap();
        assert_eq!(restored.fingerprint(), runtime.fingerprint());
        assert_eq!(restored.symbols(), runtime.symbols());
        let other = if target == scoop_target_data::WINDOWS_X86_64_MSVC_UCRT {
            scoop_target_data::WINDOWS_X86_64_MINGW_UCRT
        } else {
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT
        };
        assert!(RuntimeObjectSet::read_index(&index, other, invocation.profile()).is_err());

        // Link every real runtime object to close native dependencies. This C
        // consumer does not substitute for the managed startup/GC tests.
        let source = directory.path().join("native entry.c");
        let entry = directory.path().join("native entry.obj");
        std::fs::write(&source, "#include \"scoop_rt.h\"\nconst ScoopTypeDescriptor scoop_td_String = {0};\nint wmain(void) { scoop_rt_stdout_write((const uint8_t *)\"runtime linked\", 14); return 0; }\n").unwrap();
        checked(
            invocation
                .object_compilation_command(&source, &entry)
                .arg("-I")
                .arg(workspace.join("runtime/include")),
        );
        let link = WindowsFinalLinkProfile::resolve(
            invocation,
            &FinalLinkOptions {
                sysroot: Some(directory.path().to_owned()),
                unwind_prefix: Some(unwind),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let binary = directory.path().join("runtime.exe");
        let mut command = link.command(directory.path(), &binary).unwrap();
        command
            .arg(entry)
            .args(restored.objects().iter().map(|object| object.path()));
        link.append_system_libraries(&mut command);
        checked(&mut command);
        let output = Command::new(binary).scoop_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"runtime linked");
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
