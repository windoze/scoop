use std::path::Path;
use std::sync::OnceLock;

use crate::load_cone_manifest;
use scoop_manifest::ManifestRootLocator;
use scoop_process::CommandExt;

use super::*;
use crate::ResolvedTargetProfile;

#[cfg(not(target_env = "musl"))]
mod cxx;
mod objects;
mod target_include;
#[cfg(windows)]
mod windows;

fn target() -> &'static ResolvedTargetProfile {
    static TARGET: OnceLock<ResolvedTargetProfile> = OnceLock::new();
    TARGET.get_or_init(|| ResolvedTargetProfile::resolve_host().unwrap())
}

fn manifest(root: &Path, source: &str) -> LoadedConeManifest {
    std::fs::create_dir_all(root.join("include")).unwrap();
    // clang-cl uses the Microsoft warning levels rather than GNU -Wall.
    let warnings = if cfg!(windows) { "/W4" } else { "-Wall" };
    std::fs::write(
        root.join("Cone.toml"),
        format!(
            r#"schema = 1
[cone]
group = "test"
name = "native"
version = "1.0.0"
kind = "library"
[native]
include = ["include"]
c_flags = ["-D", "EXTRA=1", "-g", "{warnings}", "-Wextra", "-Werror"]
[[native.sources]]
path = "answer.c"
"#
        ),
    )
    .unwrap();
    std::fs::write(root.join("answer.c"), source).unwrap();
    load_cone_manifest(&ManifestRootLocator::cone_directory(root)).unwrap()
}

fn prepare(manifest: &LoadedConeManifest) -> PreparedNativeInputs {
    prepare_native_inputs(
        manifest,
        target().id(),
        target().c_bridge_toolchain(),
        OptimizationMode::Debug,
        &crate::development_runtime_root(),
    )
    .unwrap()
}

#[test]
fn nested_headers_and_flags_track_real_input_changes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cone");
    let manifest = manifest(
        &root,
        "#include <scoop_rt.h>\n#include <first.h>\nint answer(void) { return VALUE + EXTRA; }\n",
    );
    std::fs::write(root.join("include/first.h"), "#include \"second.h\"\n").unwrap();
    std::fs::write(root.join("include/second.h"), "#define VALUE 6\n").unwrap();
    let prepared = prepare(&manifest);
    let original_key = prepared.units().to_vec();
    std::fs::write(root.join("unrelated.h"), "#error not included\n").unwrap();
    assert_eq!(original_key, prepare(&manifest).units().to_vec());
    let object = directory.path().join("answer.o");
    compile_native_source(
        &prepared.units()[0],
        manifest.parsed().semantic().native(),
        &NativeToolchain::resolve(target().c_bridge_toolchain(), &manifest).unwrap(),
        OptimizationMode::Debug,
        &object,
    )
    .unwrap();
    let main = directory.path().join("main.c");
    let executable = directory.path().join("check");
    std::fs::write(
        &main,
        "int answer(void); int main(void) { return answer() != 7; }\n",
    )
    .unwrap();
    let output = target()
        .c_bridge_toolchain()
        .driver_command()
        .arg(&main)
        .arg(&object)
        .arg("-o")
        .arg(&executable)
        .scoop_output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(executable)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(root.join("include/second.h"), "#define VALUE 9\n").unwrap();
    assert_ne!(original_key, prepare(&manifest).units().to_vec());
}

#[test]
fn relocation_preserves_preprocessed_bytes_and_object_content() {
    let directory = tempfile::tempdir().unwrap();
    let mut results = Vec::new();
    for name in ["left", "right"] {
        let manifest = manifest(
            &directory.path().join(name),
            "#include <stdint.h>\nconst char *source(void) { return __FILE__; }\nint32_t answer(void) { return 42; }\n",
        );
        let prepared = prepare(&manifest);
        let object = directory.path().join(format!("{name}.o"));
        compile_native_source(
            &prepared.units()[0],
            manifest.parsed().semantic().native(),
            &NativeToolchain::resolve(target().c_bridge_toolchain(), &manifest).unwrap(),
            OptimizationMode::Debug,
            &object,
        )
        .unwrap();
        results.push((prepared.units().to_vec(), std::fs::read(object).unwrap()));
    }
    assert_eq!(results[0].0, results[1].0, "preprocessed inputs differ");
    assert_eq!(results[0].1, results[1].1, "native objects differ");
}

#[test]
fn native_headers_use_the_actual_include_path() {
    let directory = tempfile::tempdir().unwrap();
    let outside = directory.path().join("outside.h");
    std::fs::write(&outside, "#define VALUE 1\n").unwrap();
    let manifest = manifest(
        &directory.path().join("cone"),
        "#include \"../outside.h\"\nint answer(void) { return VALUE; }\n",
    );
    let prepared = prepare(&manifest);
    assert!(
        prepared
            .observed_paths
            .iter()
            .any(|p| p.canonicalize().ok() == outside.canonicalize().ok())
    );
    assert!(String::from_utf8_lossy(prepared.units()[0].preprocessed()).contains("return 1"));
}

#[test]
fn public_runtime_header_changes_invalidate_native_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = manifest(
        &directory.path().join("cone"),
        "#include <scoop_rt.h>\nint answer(void) { return M33_PUBLIC_VALUE; }\n",
    );
    let headers = directory.path().join("include");
    std::fs::create_dir(&headers).unwrap();
    for name in ["scoop_rt.h", "scoop_runtime_metadata_v1.h"] {
        std::fs::copy(
            crate::development_runtime_root().join("include").join(name),
            headers.join(name),
        )
        .unwrap();
    }
    let header = std::fs::read_to_string(headers.join("scoop_rt.h")).unwrap();
    let prepare = |value| {
        std::fs::write(
            headers.join("scoop_rt.h"),
            format!("{header}\n#define M33_PUBLIC_VALUE {value}\n"),
        )
        .unwrap();
        prepare_native_inputs(
            &manifest,
            target().id(),
            target().c_bridge_toolchain(),
            OptimizationMode::Debug,
            directory.path(),
        )
        .unwrap()
    };
    let first = prepare(1);
    let second = prepare(2);
    assert_ne!(first.units().to_vec(), second.units().to_vec());
    assert_ne!(
        first.units()[0].preprocessed(),
        second.units()[0].preprocessed()
    );
}

#[cfg(unix)]
#[test]
fn distinct_native_source_paths_remain_distinct_translation_units() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = manifest(directory.path(), "int answer(void) { return 42; }\n");
    std::fs::hard_link(
        directory.path().join("answer.c"),
        directory.path().join("alias.c"),
    )
    .unwrap();
    std::fs::write(
        manifest.manifest_path(),
        format!(
            "{}\n[[native.sources]]\npath = 'alias.c'\n",
            manifest.source_text()
        ),
    )
    .unwrap();
    let manifest =
        load_cone_manifest(&ManifestRootLocator::cone_directory(directory.path())).unwrap();
    let prepared = prepare_native_inputs(
        &manifest,
        target().id(),
        target().c_bridge_toolchain(),
        OptimizationMode::Debug,
        &crate::development_runtime_root(),
    )
    .unwrap();
    assert_eq!(prepared.units().len(), 2);
}
