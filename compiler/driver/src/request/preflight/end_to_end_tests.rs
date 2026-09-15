use std::path::Path;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::ManifestRootLocator;
use scoop_slib::{
    ArtifactDistributionClassV1, ConeKind, ConeSourceForm, DecodedSlibEnvelope,
    SingleConeProductionOutputV1,
};
use scoop_wire::DecodeLimits;

use super::*;

#[test]
fn formal_pipeline_publishes_manifest_library_and_executable_artifacts() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    assert_graph_dependencies(&core, &target, &[]);

    let library_root = sysroot.path().join("library");
    write_manifest_cone(
        &library_root,
        "dev.example",
        "stage3.library",
        "library",
        "fun answer(): Long = 42\n",
    );
    let library = build_manifest(
        sysroot.path(),
        &target,
        &library_root,
        &sysroot.path().join("output/library.slib"),
    );
    let library_validation = library.artifact().validation();
    assert_eq!(
        library_validation.coordinate(),
        &ConeCoordinate::new("dev.example", "stage3.library", "0.1.0").unwrap()
    );
    assert_eq!(library_validation.kind(), ConeKind::Library);
    assert_eq!(library_validation.source_form(), ConeSourceForm::Manifest);
    assert_eq!(
        library_validation.link_summary().distribution(),
        ArtifactDistributionClassV1::DistributableCone
    );
    assert_eq!(
        library_validation.link_summary().output(),
        &SingleConeProductionOutputV1::Library
    );
    assert!(library_validation.link_summary().link_object_count() > 0);
    assert_graph_dependencies(&library, &target, &[ConeIdentity::CORE]);

    let executable_root = sysroot.path().join("executable");
    write_manifest_cone(
        &executable_root,
        "dev.example",
        "stage3.executable",
        "executable",
        "fun main() {\n    val message = \"stage3\"\n    val answer = stage3CoreAnswer()\n}\n",
    );
    let executable = build_manifest(
        sysroot.path(),
        &target,
        &executable_root,
        &sysroot.path().join("output/executable.slib"),
    );
    let executable_validation = executable.artifact().validation();
    assert_eq!(executable_validation.kind(), ConeKind::Executable);
    assert_eq!(
        executable_validation.source_form(),
        ConeSourceForm::Manifest
    );
    assert_eq!(
        executable_validation.link_summary().distribution(),
        ArtifactDistributionClassV1::DistributableCone
    );
    assert!(matches!(
        executable_validation.link_summary().output(),
        SingleConeProductionOutputV1::Executable(_)
    ));
    assert!(executable_validation.link_summary().link_object_count() > 0);
    assert_graph_dependencies(&executable, &target, &[ConeIdentity::CORE]);
}

#[test]
fn formal_pipeline_is_byte_reproducible_across_absolute_source_and_output_paths() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);

    let first_root = sysroot.path().join("checkout-a/library");
    let second_root = sysroot.path().join("checkout-b/library");
    for root in [&first_root, &second_root] {
        write_manifest_cone(
            root,
            "dev.example",
            "stage3.reproducible",
            "library",
            "fun answer(): Long = 42\n",
        );
    }

    let first = build_manifest(
        sysroot.path(),
        &target,
        &first_root,
        &sysroot.path().join("output-a/result.slib"),
    );
    let second = build_manifest(
        sysroot.path(),
        &target,
        &second_root,
        &sysroot.path().join("output-b/result.slib"),
    );

    let first_bytes = std::fs::read(first.artifact().path()).unwrap();
    let second_bytes = std::fs::read(second.artifact().path()).unwrap();
    assert_eq!(first_bytes, second_bytes);
    assert_eq!(
        first.artifact().validation().artifact_fingerprint(),
        second.artifact().validation().artifact_fingerprint()
    );
}

fn resolved_target() -> Option<scoop_codegen::ResolvedTargetProfile> {
    // The target resolver owns host Apple-toolchain qualification. A machine
    // blocked by the Xcode license gate cannot enter object production.
    scoop_codegen::ResolvedTargetProfile::resolve_host().ok()
}

fn bootstrap_core(
    sysroot: &Path,
    target: &scoop_codegen::ResolvedTargetProfile,
) -> SingleConeProductionSuccess {
    copy_trusted_core_sources(sysroot);
    let slot =
        crate::trusted_core::resolve_trusted_core_slot_at(sysroot, target.lir_target_selection())
            .unwrap();
    let artifact_path = slot.artifact().path().to_path_buf();
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    let (bootstrap, artifact_slot) = slot.into_bootstrap_parts();
    SingleConeBuildRequest::new(
        CurrentConeInput::TrustedCoreBootstrap {
            input: Box::new(bootstrap),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::BootstrapSelf { artifact_slot },
        target.clone(),
        SlibOutputDestination::new(&artifact_path).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish(DecodeLimits::default())
    .unwrap()
}

fn build_manifest(
    sysroot: &Path,
    target: &scoop_codegen::ResolvedTargetProfile,
    root: &Path,
    output: &Path,
) -> SingleConeProductionSuccess {
    build_ordinary(
        sysroot,
        target,
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(root),
        },
        output,
    )
}

fn build_ordinary(
    sysroot: &Path,
    target: &scoop_codegen::ResolvedTargetProfile,
    current: CurrentConeInput,
    output: &Path,
) -> SingleConeProductionSuccess {
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let core_slot =
        crate::trusted_core::resolve_trusted_core_slot_at(sysroot, target.lir_target_selection())
            .unwrap();
    SingleConeBuildRequest::new(
        current,
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::Artifact(core_slot.existing_artifact_input().unwrap()),
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish(DecodeLimits::default())
    .unwrap()
}

fn assert_graph_dependencies(
    artifact: &SingleConeProductionSuccess,
    target: &scoop_codegen::ResolvedTargetProfile,
    expected: &[ConeIdentity],
) {
    let bytes = std::fs::read(artifact.artifact().path()).unwrap();
    let graph = DecodedSlibEnvelope::open(
        &bytes,
        DecodeLimits::default(),
        target.lir_target_selection(),
    )
    .unwrap()
    .validate_graph()
    .unwrap();
    assert_eq!(
        graph
            .direct_dependencies()
            .iter()
            .map(scoop_slib::DependencyRecord::identity)
            .collect::<Vec<_>>(),
        expected
    );
}

fn write_manifest_cone(root: &Path, group: &str, name: &str, kind: &str, source: &str) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        format!(
            "schema = 1\n[cone]\ngroup = \"{group}\"\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"{kind}\"\n"
        ),
    )
    .unwrap();
    std::fs::write(root.join("src/main.scoop"), source).unwrap();
}

fn copy_trusted_core_sources(sysroot: &Path) {
    let source = crate::workspace_root().join("sysroot/lib/scoop.core");
    let destination = sysroot.join("lib/scoop.core");
    std::fs::create_dir_all(destination.join("src")).unwrap();
    std::fs::copy(source.join("Cone.toml"), destination.join("Cone.toml")).unwrap();
    for entry in std::fs::read_dir(source.join("src")).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        std::fs::copy(
            entry.path(),
            destination.join("src").join(entry.file_name()),
        )
        .unwrap();
    }
    std::fs::write(
        destination.join("src/stage3_test.scoop"),
        "public fun stage3CoreAnswer(): Long = 42\n",
    )
    .unwrap();
}
