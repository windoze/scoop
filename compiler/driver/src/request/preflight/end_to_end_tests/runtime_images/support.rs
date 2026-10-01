use super::*;
use std::os::unix::process::ExitStatusExt;

pub(super) fn build_fixture(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    name: &str,
    kind: &str,
    dependencies: &[&SingleConeProductionSuccess],
    support: &[&SingleConeProductionSuccess],
) -> SingleConeProductionSuccess {
    let root = sysroot.join(name);
    let source = std::fs::read_to_string(
        crate::workspace_root().join(format!("tests/fixtures/m23-runtime-images/{name}.scoop")),
    )
    .unwrap();
    write_manifest_cone(&root, "dev.example", name, kind, &source);
    let mut manifest = std::fs::read_to_string(root.join("Cone.toml")).unwrap();
    if !dependencies.is_empty() {
        manifest.push_str("[dependencies]\n");
        for dependency in dependencies {
            let coordinate = dependency.artifact().summary().coordinate();
            manifest.push_str(&format!(
                "\"{}:{}\" = \"{}\"\n",
                coordinate.group(),
                coordinate.name(),
                coordinate.version()
            ));
        }
    }
    std::fs::write(root.join("Cone.toml"), manifest).unwrap();
    let result = build_manifest_request(
        sysroot,
        target,
        &root,
        &sysroot.join(format!("output/{name}.slib")),
        dependencies
            .iter()
            .map(|artifact| artifact.artifact().path().to_path_buf())
            .collect(),
        support
            .iter()
            .map(|artifact| artifact.artifact().path().to_path_buf())
            .collect(),
    )
    .build_and_publish()
    .unwrap();
    std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
    result
}

pub(super) fn run(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess],
    library: &Path,
    directory: &Path,
    expected: &str,
    failure: Option<&str>,
) {
    assert!(
        artifacts.len() >= 3,
        "the runtime receives a real multi-Cone closure"
    );
    let closure = runtime::read(target, artifacts);
    let template = std::fs::read_to_string(
        crate::workspace_root().join("tests/fixtures/m23-runtime-images/runtime.c"),
    )
    .unwrap();
    let executable = runtime::link_program(target, &closure, library, &template, directory);
    for stress in [false, true] {
        let mut command = std::process::Command::new(&executable);
        command.env_remove("SCOOP_GC_STRESS_MOVE");
        if stress {
            command.env("SCOOP_GC_STRESS_MOVE", "1");
        }
        let output = command.output().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected,
            "{directory:?} stress={stress}: {output:?}"
        );
        if let Some(name) = failure {
            assert_eq!(output.status.signal(), Some(6), "{directory:?}: {output:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(name),
                "{output:?}"
            );
        } else {
            assert!(output.status.success(), "{directory:?}: {output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
}
