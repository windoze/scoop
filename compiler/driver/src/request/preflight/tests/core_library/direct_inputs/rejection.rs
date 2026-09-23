use super::*;

pub(super) fn check(
    root_target: &scoop_toolchain::ResolvedTargetProfile,
    root: &Path,
    workspace: &Path,
    sysroot: &Path,
    core: &Path,
    installed_core: &Path,
) {
    let output = workspace.join("rejected-direct.slib");
    let absent = workspace.join("missing-default");
    let build = |direct: &[&Path], support: &[&Path]| {
        request(root_target, root, &output, &absent, direct, support)
    };
    let duplicate = build(&[core, core], &[])
        .load_preflight(DecodeLimits::default())
        .unwrap();
    assert!(matches!(duplicate.validate(),
        Err(SingleConeDependencyValidationError::ExplicitDependencies(error))
            if matches!(*error, ExplicitDependencyValidationError::DuplicateIdentity { identity: ConeIdentity::CORE, .. })
    ));
    let support = build(&[], &[core])
        .load_preflight(DecodeLimits::default())
        .unwrap();
    assert!(matches!(support.validate(),
        Err(SingleConeDependencyValidationError::ExplicitDependencies(error))
            if matches!(*error, ExplicitDependencyValidationError::ManifestDirectSet { .. })
    ));
    let damaged = workspace.join("damaged-explicit.slib");
    std::fs::write(&damaged, b"invalid artifact").unwrap();
    assert!(
        matches!(build(&[core, &damaged], &[]).load_preflight(DecodeLimits::default()),
            Err(SingleConePreflightError::Dependencies(error))
                if matches!(&*error, ExplicitDependencyValidationError::Summary { input, .. } if input.path() == damaged)
        )
    );
    assert!(
        matches!(build(&[], &[]).load_preflight(DecodeLimits::default()),
            Err(SingleConePreflightError::DefaultCoreSlot(error)) if error.path() == absent
        )
    );
    assert!(matches!(
        request(root_target, root, installed_core, sysroot, &[], &[])
            .load_preflight(DecodeLimits::default()),
        Err(SingleConePreflightError::Request(error))
            if matches!(*error, crate::SingleConeBuildRequestError::OutputIsolation { kind: crate::OutputIsolationErrorKind::AliasesInput { .. }, .. })
    ));
    assert_eq!(
        std::fs::read(core).unwrap(),
        std::fs::read(installed_core).unwrap()
    );
    assert!(!output.exists());
    assert!(!absent.exists());

    let invalid = workspace.join("invalid-direct-manifest");
    std::fs::create_dir_all(&invalid).unwrap();
    std::fs::write(invalid.join("Cone.toml"), "invalid manifest").unwrap();
    assert!(matches!(
        request(root_target, &invalid, &output, &absent, &[&damaged], &[])
            .load_preflight(DecodeLimits::default()),
        Err(SingleConePreflightError::Manifest(_))
    ));
}
