use std::path::Path;
use std::sync::OnceLock;

use scoop_manifest::{ManifestRootLocator, load_cone_manifest};
use scoop_process::CommandExt;

use super::*;
use crate::ResolvedTargetProfile;

#[cfg(not(target_env = "musl"))]
mod cxx;

fn target() -> &'static ResolvedTargetProfile {
    static TARGET: OnceLock<ResolvedTargetProfile> = OnceLock::new();
    TARGET.get_or_init(|| ResolvedTargetProfile::resolve_host().unwrap())
}

fn manifest(root: &Path, source: &str) -> LoadedConeManifest {
    std::fs::create_dir_all(root.join("include")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        r#"schema = 1
[cone]
group = "test"
name = "native"
version = "1.0.0"
kind = "library"
[native]
include = ["include"]
c_flags = ["-D", "EXTRA=1", "-g", "-Wall", "-Wextra", "-Werror"]
[[native.sources]]
path = "answer.c"
"#,
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
        &crate::development_runtime_root().join("include"),
    )
    .unwrap()
}

#[test]
fn snapshot_compiles_old_headers_and_next_build_observes_changes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cone");
    let manifest = manifest(
        &root,
        "#include <scoop_rt.h>\n#include <first.h>\nint answer(void) { return VALUE + EXTRA; }\n",
    );
    std::fs::write(root.join("include/first.h"), "#include \"second.h\"\n").unwrap();
    std::fs::write(root.join("include/second.h"), "#define VALUE 6\n").unwrap();
    let prepared = prepare(&manifest);
    let original_key = scoop_wire::encode(&prepared).unwrap();
    std::fs::write(root.join("unrelated.h"), "#error not included\n").unwrap();
    assert_eq!(
        original_key,
        scoop_wire::encode(&prepare(&manifest)).unwrap()
    );
    std::fs::write(root.join("include/second.h"), "#define VALUE 9\n").unwrap();
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
    assert_ne!(
        original_key,
        scoop_wire::encode(&prepare(&manifest)).unwrap()
    );
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
        results.push((
            scoop_wire::encode(&prepared).unwrap(),
            std::fs::read(object).unwrap(),
        ));
    }
    assert_eq!(results[0].0, results[1].0, "preprocessed inputs differ");
    assert_eq!(results[0].1, results[1].1, "native objects differ");
}

#[test]
fn native_headers_cannot_escape_the_declared_input_roots() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("outside.h"), "#define VALUE 1\n").unwrap();
    let manifest = manifest(
        &directory.path().join("cone"),
        "#include \"../outside.h\"\nint answer(void) { return VALUE; }\n",
    );
    let error = prepare_native_inputs(
        &manifest,
        target().id(),
        target().c_bridge_toolchain(),
        OptimizationMode::Debug,
        &crate::development_runtime_root().join("include"),
    )
    .unwrap_err();
    assert!(error.0.contains("outside the Cone"), "{error}");
}

#[test]
fn public_runtime_header_changes_invalidate_native_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = manifest(
        &directory.path().join("cone"),
        "#include <scoop_rt.h>\nint answer(void) { return M33_PUBLIC_VALUE; }\n",
    );
    let headers = directory.path().join("public");
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
            &headers,
        )
        .unwrap()
    };
    let first = prepare(1);
    let second = prepare(2);
    assert_ne!(
        scoop_wire::encode(&first).unwrap(),
        scoop_wire::encode(&second).unwrap()
    );
    assert_ne!(
        first.units()[0].preprocessed(),
        second.units()[0].preprocessed()
    );
}

#[cfg(unix)]
#[test]
fn repeated_native_files_are_rejected_through_hard_links() {
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
    let error = prepare_native_inputs(
        &manifest,
        target().id(),
        target().c_bridge_toolchain(),
        OptimizationMode::Debug,
        &crate::development_runtime_root().join("include"),
    )
    .unwrap_err();
    assert!(error.0.contains("duplicate native source"), "{error}");
}
