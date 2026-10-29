use std::path::{Path, PathBuf};
use std::process::Command;

fn checked(command: &mut Command) {
    let output = command.output().expect("run the native trace tool");
    assert!(
        output.status.success(),
        "{command:?}:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn trace_symbols_preserve_source_names_paths_and_reject_changed_loader_indices() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let work = tempfile::tempdir().unwrap();
    let target = if cfg!(target_os = "macos") {
        scoop_target_data::DARWIN_AARCH64
    } else {
        scoop_target_data::LINUX_X86_64_GNU
    };
    let profile = scoop_toolchain::ValidatedRuntimeBuildProfile::for_target(target).unwrap();
    let mut sources = profile
        .runtime_sources()
        .into_iter()
        .filter(|source| source.starts_with("vendor/libbacktrace/"))
        .map(|source| workspace.join("sysroot").join(source))
        .collect::<Vec<PathBuf>>();
    let test_source = workspace.join("runtime/tests/stack_trace_symbols_test.c");
    sources.extend([
        workspace.join("runtime/src/stack_trace/capture.c"),
        workspace.join("runtime/src/stack_trace/symbolize.c"),
        workspace.join(super::platform_support::native_trace_module_source()),
        test_source.clone(),
    ]);
    let mut objects = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let object = work.path().join(format!("trace-{index}.o"));
        let mut command = Command::new("cc");
        command
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-O0",
                "-g",
                "-gdwarf-5",
            ])
            .arg("-I")
            .arg(workspace.join("runtime/src"))
            .arg("-I")
            .arg(workspace.join("sysroot/vendor/libbacktrace"))
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(&object);
        checked(&mut command);
        objects.push(object);
    }
    let binary = work.path().join("trace-symbols");
    let mut link = Command::new("cc");
    link.args(&objects).arg("-pthread");
    if cfg!(target_os = "linux") {
        link.arg("-ldl");
    }
    checked(link.arg("-o").arg(&binary));
    if cfg!(target_os = "macos") {
        checked(Command::new("dsymutil").arg(&binary));
    }
    let library_source = work.path().join("library.c");
    std::fs::write(
        &library_source,
        "int trace_library_marker(void) { return 1; }\n",
    )
    .unwrap();
    let library = work.path().join(if cfg!(target_os = "macos") {
        "library.dylib"
    } else {
        "library.so"
    });
    checked(
        Command::new("cc")
            .arg(if cfg!(target_os = "macos") {
                "-dynamiclib"
            } else {
                "-shared"
            })
            .arg("-fPIC")
            .arg(&library_source)
            .arg("-o")
            .arg(&library),
    );
    checked(Command::new(&binary).arg(&library).arg(&test_source));
}
