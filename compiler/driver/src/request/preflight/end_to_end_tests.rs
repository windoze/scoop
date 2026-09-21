use std::path::Path;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::ManifestRootLocator;
use scoop_slib::{
    ArtifactDistributionClassV1, ConeKind, ConeSourceForm, DecodedSlibEnvelope,
    SingleConeProductionOutputV1,
};
use scoop_wire::DecodeLimits;

use super::*;
use crate::{ExplicitDependencyInputs, HostArtifactLocator};

mod cross_cone;

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
fn formal_pipeline_is_byte_reproducible_for_every_stage3_input_form() {
    let Some(target) = resolved_target() else {
        return;
    };
    let workspace = tempfile::tempdir().unwrap();
    let first_sysroot = workspace.path().join("checkout-a/sysroot");
    let second_sysroot = workspace.path().join("checkout-b/sysroot");
    let first_core = bootstrap_core(&first_sysroot, &target);
    let second_core = bootstrap_core(&second_sysroot, &target);
    assert_artifacts_equal(&first_core, &second_core);

    let first_root = first_sysroot.join("cones/library");
    let second_root = second_sysroot.join("other/cones/library");
    for root in [&first_root, &second_root] {
        write_manifest_cone(
            root,
            "dev.example",
            "stage3.reproducible",
            "library",
            "fun answer(): Long = 42\n",
        );
    }
    std::fs::write(first_root.join("src/z.scoop"), "fun last(): Long = 2\n").unwrap();
    std::fs::write(first_root.join("src/a.scoop"), "fun first(): Long = 1\n").unwrap();
    std::fs::write(second_root.join("src/a.scoop"), "fun first(): Long = 1\n").unwrap();
    std::fs::write(second_root.join("src/z.scoop"), "fun last(): Long = 2\n").unwrap();

    let first = build_manifest(
        &first_sysroot,
        &target,
        &first_root,
        &first_sysroot.join("output-a/library.slib"),
    );
    let second = build_manifest(
        &second_sysroot,
        &target,
        &second_root,
        &second_sysroot.join("output-b/library.slib"),
    );
    assert_artifacts_equal(&first, &second);

    let first_root = first_sysroot.join("cones/executable");
    let second_root = second_sysroot.join("other/cones/executable");
    for root in [&first_root, &second_root] {
        write_manifest_cone(
            root,
            "dev.example",
            "stage3.reproducible-executable",
            "executable",
            "fun main() { val message = \"stage3\" }\n",
        );
    }
    let first = build_manifest(
        &first_sysroot,
        &target,
        &first_root,
        &first_sysroot.join("output-a/executable.slib"),
    );
    let second = build_manifest(
        &second_sysroot,
        &target,
        &second_root,
        &second_sysroot.join("output-b/executable.slib"),
    );
    assert_artifacts_equal(&first, &second);

    let first_source = first_sysroot.join("inputs/first-name.scoop");
    let second_source = second_sysroot.join("other/inputs/second-name.scoop");
    std::fs::create_dir_all(first_source.parent().unwrap()).unwrap();
    std::fs::create_dir_all(second_source.parent().unwrap()).unwrap();
    std::fs::write(&first_source, "fun main() {}\n").unwrap();
    std::fs::write(&second_source, "fun main() {}\n").unwrap();
    let first = build_single_file(
        &first_sysroot,
        &target,
        &first_source,
        &first_sysroot.join("output-a/single-file.slib"),
    );
    let second = build_single_file(
        &second_sysroot,
        &target,
        &second_source,
        &second_sysroot.join("output-b/single-file.slib"),
    );
    assert_artifacts_equal(&first, &second);
}

#[test]
fn formal_pipeline_preserves_source_extern_without_attempting_final_link() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);

    let executable_root = sysroot.path().join("source-extern");
    write_manifest_cone(
        &executable_root,
        "dev.example",
        "stage3.source-extern",
        "executable",
        r#"@Extern(lib = "stage3_library_that_does_not_exist", name = "stage3_add")
fun stage3Add(left: Int, right: Int): Int

fun main() {
    @Unsafe {
        val result = stage3Add(1, 2)
    }
}
"#,
    );
    let artifact = build_manifest(
        sysroot.path(),
        &target,
        &executable_root,
        &sysroot.path().join("output/source-extern.slib"),
    );

    assert!(artifact.artifact().path().is_file());
    assert_eq!(
        artifact.artifact().validation().kind(),
        ConeKind::Executable
    );
    assert!(matches!(
        artifact.artifact().validation().link_summary().output(),
        SingleConeProductionOutputV1::Executable(_)
    ));
    assert_graph_dependencies(&artifact, &target, &[ConeIdentity::CORE]);
}

#[test]
fn production_entry_uses_the_shared_closure_profile_for_current_sources() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);
    let source = sysroot.path().join("bounded.scoop");
    std::fs::write(&source, "fun main() {}\n").unwrap();
    let output = sysroot.path().join("output/bounded.slib");
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let core_slot = crate::trusted_core::resolve_trusted_core_slot_at(
        sysroot.path(),
        target.lir_target_selection(),
    )
    .unwrap();
    let request = SingleConeBuildRequest::new(
        CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(&source).unwrap(),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core_slot.artifact()).unwrap()),
        target,
        SlibOutputDestination::new(&output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    let mut values = SlibClosureDecodeLimitsV1::M23_DEFAULT.values();
    values.source_bytes = 3;
    let closure_limits = SlibClosureDecodeLimitsV1::new(values).unwrap();

    assert!(matches!(
        request.build_and_publish_with_closure_limits(DecodeLimits::default(), closure_limits),
        Err(SingleConeProductionError::Sources(
            CurrentConeSourceStageError::SingleFile(source)
        )) if matches!(
            source.kind(),
            SingleFileInputErrorKind::ByteLimitExceeded {
                limit: 3,
                observed: 14,
            }
        )
    ));
    assert!(!output.exists());
}

#[test]
fn formal_pipeline_preserves_combined_stage3_language_features() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);

    let executable_root = sysroot.path().join("combined");
    write_manifest_cone(
        &executable_root,
        "dev.example",
        "stage3.combined",
        "executable",
        r#"val initializedMessage: String = makeMessage()

fun makeMessage(): String = "stage3"

fun apply(operation: () -> String): String = operation()

fun main() {
    val render: () -> String = { initializedMessage }
    val rendered = apply(render)
    val answer = stage3CoreAnswer()
    stage3CoreFailure()
}
"#,
    );
    let artifact = build_manifest(
        sysroot.path(),
        &target,
        &executable_root,
        &sysroot.path().join("output/combined.slib"),
    );

    assert!(artifact.artifact().path().is_file());
    assert_eq!(
        artifact.artifact().validation().kind(),
        ConeKind::Executable
    );
    assert!(matches!(
        artifact.artifact().validation().link_summary().output(),
        SingleConeProductionOutputV1::Executable(_)
    ));
    assert_graph_dependencies(&artifact, &target, &[ConeIdentity::CORE]);
}

#[test]
fn dependency_preflight_validates_artifacts_and_closure_before_source_discovery() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);

    let dependency_root = sysroot.path().join("dependency");
    write_manifest_cone(
        &dependency_root,
        "dev.example",
        "stage3.dependency",
        "library",
        "fun answer(): Long = 42\n",
    );
    let dependency = build_manifest(
        sysroot.path(),
        &target,
        &dependency_root,
        &sysroot.path().join("artifacts/dependency.slib"),
    );
    let dependency_path = dependency.artifact().path().to_path_buf();
    let dependency_coordinate =
        ConeCoordinate::new("dev.example", "stage3.dependency", "0.1.0").unwrap();

    let current = sysroot.path().join("current-valid");
    write_dependency_manifest(&current, "stage3.current-valid", &[&dependency_coordinate]);
    let error = build_manifest_request(
        sysroot.path(),
        &target,
        &current,
        &sysroot.path().join("output/current-valid.slib"),
        vec![dependency_path.clone()],
        Vec::new(),
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Sources(CurrentConeSourceStageError::Discovery(_))
    ));
    assert!(!current.join("src").exists());

    let malformed = sysroot.path().join("artifacts/malformed.slib");
    std::fs::write(&malformed, b"not a slib").unwrap();
    let current = sysroot.path().join("current-malformed");
    write_dependency_manifest(
        &current,
        "stage3.current-malformed",
        &[&dependency_coordinate],
    );
    let error = build_manifest_request(
        sysroot.path(),
        &target,
        &current,
        &sysroot.path().join("output/current-malformed.slib"),
        vec![malformed],
        Vec::new(),
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            CoreOnlyRequestValidationError::ExplicitDependencies(source)
        ) if matches!(
            source.as_ref(),
            ExplicitDependencyValidationError::Summary { .. }
        )
    ));
    assert!(!current.join("src").exists());

    let current = sysroot.path().join("current-missing-direct");
    write_dependency_manifest(
        &current,
        "stage3.current-missing-direct",
        &[&dependency_coordinate],
    );
    let error = build_manifest_request(
        sysroot.path(),
        &target,
        &current,
        &sysroot.path().join("output/current-missing-direct.slib"),
        Vec::new(),
        Vec::new(),
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            CoreOnlyRequestValidationError::ExplicitDependencies(source)
        ) if matches!(
            source.as_ref(),
            ExplicitDependencyValidationError::ManifestDirectSet { .. }
        )
    ));
    assert!(!current.join("src").exists());

    let current = sysroot.path().join("current-duplicate");
    write_dependency_manifest(
        &current,
        "stage3.current-duplicate",
        &[&dependency_coordinate],
    );
    let error = build_manifest_request(
        sysroot.path(),
        &target,
        &current,
        &sysroot.path().join("output/current-duplicate.slib"),
        vec![dependency_path.clone(), dependency_path.clone()],
        Vec::new(),
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            CoreOnlyRequestValidationError::ExplicitDependencies(source)
        ) if matches!(
            source.as_ref(),
            ExplicitDependencyValidationError::DuplicateIdentity {
                same_fingerprint: true,
                ..
            }
        )
    ));
    assert!(!current.join("src").exists());

    let current = sysroot.path().join("current-extra-support");
    write_dependency_manifest(&current, "stage3.current-extra-support", &[]);
    let error = build_manifest_request(
        sysroot.path(),
        &target,
        &current,
        &sysroot.path().join("output/current-extra-support.slib"),
        Vec::new(),
        vec![dependency_path],
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            CoreOnlyRequestValidationError::ExplicitDependencies(source)
        ) if matches!(
            source.as_ref(),
            ExplicitDependencyValidationError::SupportClosure { .. }
        )
    ));
    assert!(!current.join("src").exists());

    let current = sysroot.path().join("current-core-explicit");
    write_dependency_manifest(&current, "stage3.current-core-explicit", &[]);
    let error = build_manifest_request(
        sysroot.path(),
        &target,
        &current,
        &sysroot.path().join("output/current-core-explicit.slib"),
        Vec::new(),
        vec![core.artifact().path().to_path_buf()],
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap_err();
    assert!(
        matches!(
            &error,
            SingleConeProductionError::Validation(
                CoreOnlyRequestValidationError::ExplicitDependencies(source)
            ) if matches!(
                source.as_ref(),
                ExplicitDependencyValidationError::DuplicateIdentity { identity, .. } if *identity == ConeIdentity::CORE
            )
        ),
        "{error:?}"
    );
    assert!(!current.join("src").exists());

    let executable_root = sysroot.path().join("dependency-executable");
    write_manifest_cone(
        &executable_root,
        "dev.example",
        "stage3.dependency-executable",
        "executable",
        "fun main() {}\n",
    );
    let executable = build_manifest(
        sysroot.path(),
        &target,
        &executable_root,
        &sysroot.path().join("artifacts/dependency-executable.slib"),
    );
    let executable_coordinate =
        ConeCoordinate::new("dev.example", "stage3.dependency-executable", "0.1.0").unwrap();
    let current = sysroot.path().join("current-executable-dependency");
    write_dependency_manifest(
        &current,
        "stage3.current-executable-dependency",
        &[&executable_coordinate],
    );
    let error = build_manifest_request(
        sysroot.path(),
        &target,
        &current,
        &sysroot
            .path()
            .join("output/current-executable-dependency.slib"),
        vec![executable.artifact().path().to_path_buf()],
        Vec::new(),
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            CoreOnlyRequestValidationError::ExplicitDependencies(source)
        ) if matches!(
            source.as_ref(),
            ExplicitDependencyValidationError::UnsupportedArtifactShape {
                kind: ConeKind::Executable,
                source_form: ConeSourceForm::Manifest,
                ..
            }
        )
    ));
    assert!(!current.join("src").exists());
}

fn resolved_target() -> Option<scoop_toolchain::ResolvedTargetProfile> {
    // The target resolver owns host Apple-toolchain qualification. A machine
    // blocked by the Xcode license gate cannot enter object production.
    let target = scoop_toolchain::ResolvedTargetProfile::resolve_host().ok()?;
    scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection()).ok()?;
    Some(target)
}

fn bootstrap_core(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
) -> SingleConeProductionSuccess {
    copy_trusted_core_sources(sysroot);
    let slot =
        crate::trusted_core::resolve_trusted_core_slot_at(sysroot, target.lir_target_selection())
            .unwrap();
    let artifact_path = slot.artifact().to_path_buf();
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(slot.source_root()),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::BootstrapSelf,
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
    target: &scoop_toolchain::ResolvedTargetProfile,
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

fn build_single_file(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    source: &Path,
    output: &Path,
) -> SingleConeProductionSuccess {
    build_ordinary(
        sysroot,
        target,
        CurrentConeInput::SingleFile {
            source: scoop_manifest::SingleFileLocator::from_path(source).unwrap(),
        },
        output,
    )
}

fn build_ordinary(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
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
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core_slot.artifact()).unwrap()),
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish(DecodeLimits::default())
    .unwrap()
}

fn build_manifest_request(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    root: &Path,
    output: &Path,
    direct: Vec<std::path::PathBuf>,
    support: Vec<std::path::PathBuf>,
) -> SingleConeBuildRequest {
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let core_slot =
        crate::trusted_core::resolve_trusted_core_slot_at(sysroot, target.lir_target_selection())
            .unwrap();
    SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(root),
        },
        ExplicitDependencyInputs::new(
            direct
                .into_iter()
                .map(HostArtifactLocator::new)
                .collect::<Result<_, _>>()
                .unwrap(),
            support
                .into_iter()
                .map(HostArtifactLocator::new)
                .collect::<Result<_, _>>()
                .unwrap(),
        )
        .unwrap(),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core_slot.artifact()).unwrap()),
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
}

fn assert_graph_dependencies(
    artifact: &SingleConeProductionSuccess,
    target: &scoop_toolchain::ResolvedTargetProfile,
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

fn assert_artifacts_equal(
    first: &SingleConeProductionSuccess,
    second: &SingleConeProductionSuccess,
) {
    assert_eq!(
        std::fs::read(first.artifact().path()).unwrap(),
        std::fs::read(second.artifact().path()).unwrap()
    );
    assert_eq!(
        first.artifact().validation().artifact_fingerprint(),
        second.artifact().validation().artifact_fingerprint()
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

fn write_dependency_manifest(root: &Path, name: &str, dependencies: &[&ConeCoordinate]) -> String {
    std::fs::create_dir_all(root).unwrap();
    let mut manifest = format!(
        "schema = 1\n[cone]\ngroup = \"dev.example\"\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"library\"\n"
    );
    if !dependencies.is_empty() {
        manifest.push_str("[dependencies]\n");
        for coordinate in dependencies {
            manifest.push_str(&format!(
                "\"{}:{}\" = \"{}\"\n",
                coordinate.group(),
                coordinate.name(),
                coordinate.version()
            ));
        }
    }
    std::fs::write(root.join("Cone.toml"), &manifest).unwrap();
    manifest
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
        "public fun stage3CoreAnswer(): Long = 42\n\
         public fun stage3CoreFailure() { throw ArithmeticException() }\n",
    )
    .unwrap();
}
