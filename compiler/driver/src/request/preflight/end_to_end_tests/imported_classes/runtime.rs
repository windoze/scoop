use super::*;
use std::path::PathBuf;
use std::process::Command;

mod execution;
mod runner;
pub(in super::super) use execution::execute;

pub(in super::super) fn build(
    target: &scoop_toolchain::ResolvedTargetProfile,
    directory: &Path,
) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let workspace = crate::workspace_root();
    let profile = target.runtime_build();
    let mut objects = Vec::new();
    for (index, source) in profile.runtime_sources().iter().enumerate() {
        let object = directory.join(format!("{index}.o"));
        let output = Command::new(target.c_bridge_toolchain().compiler_driver())
            .arg("-isysroot")
            .arg(target.c_bridge_toolchain().sdk_root())
            .args(profile.runtime_c_flags())
            .arg("-I")
            .arg(workspace.join("runtime/include"))
            .arg("-c")
            .arg(workspace.join(source))
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        objects.push(object);
    }
    let archive = directory.join("runtime.a");
    archive_objects(&archive, &objects);
    archive
}

pub(in super::super) fn read(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess],
) -> scoop_slib::LinkSymbolsReplayedCrossConeLayoutClosure {
    let bytes = artifacts
        .iter()
        .map(|artifact| std::fs::read(artifact.artifact().path()).unwrap())
        .collect::<Vec<_>>();
    let identities = artifacts
        .iter()
        .map(|artifact| {
            artifact
                .artifact()
                .summary()
                .coordinate()
                .identity()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let current = artifacts.len().checked_sub(1).expect("current artifact");
    let mut direct = artifacts[current]
        .artifact()
        .summary()
        .direct_dependencies()
        .iter()
        .map(scoop_slib::DependencyRecord::identity)
        .collect::<Vec<_>>();
    direct.sort_unstable();
    scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            identities[current],
            target.lir_target_selection(),
            direct,
            bytes[..current].iter().map(Vec::as_slice).collect(),
            &bytes[current],
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap_or_else(|error| {
        panic!(
            "reading runtime closure for {:?}: {error:?}",
            artifacts[current].artifact().summary().coordinate()
        )
    })
}

pub(in super::super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess],
    runtime: &Path,
    fixtures: &Path,
    directory: &Path,
    case: &str,
) -> scoop_slib::LinkSymbolsReplayedCrossConeLayoutClosure {
    let closure = read(target, artifacts);
    let template = std::fs::read_to_string(fixtures.join("runtime.c")).unwrap();
    execute(
        target, artifacts, &closure, runtime, &template, directory, case,
    );
    closure
}

fn archive_objects(archive: &Path, objects: &[PathBuf]) {
    let output = Command::new("ar")
        .arg("rcs")
        .arg(archive)
        .args(objects)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "archiving actual objects: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
