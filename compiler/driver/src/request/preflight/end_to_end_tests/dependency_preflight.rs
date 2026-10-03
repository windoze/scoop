use super::*;

#[test]
fn dependency_preflight_validates_artifacts_and_closure_before_source_discovery() {
    let target = resolved_target().expect("artifact integration tests require a host target");
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
    .build_and_publish()
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
    .build_and_publish()
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            SingleConeDependencyValidationError::ExplicitDependencies(source)
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
    .build_and_publish()
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            SingleConeDependencyValidationError::ExplicitDependencies(source)
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
    .build_and_publish()
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            SingleConeDependencyValidationError::ExplicitDependencies(source)
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
    .build_and_publish()
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            SingleConeDependencyValidationError::ExplicitDependencies(source)
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
    .build_and_publish()
    .unwrap_err();
    assert!(
        matches!(
            &error,
            SingleConeProductionError::Validation(
                SingleConeDependencyValidationError::ExplicitDependencies(source)
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
    .build_and_publish()
    .unwrap_err();
    assert!(matches!(
        error,
        SingleConeProductionError::Validation(
            SingleConeDependencyValidationError::ExplicitDependencies(source)
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
