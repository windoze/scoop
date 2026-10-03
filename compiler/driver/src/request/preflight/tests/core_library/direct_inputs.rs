use super::*;

mod rejection;

fn request(
    target: &scoop_toolchain::ResolvedTargetProfile,
    root: &Path,
    output: &Path,
    sysroot: &Path,
    direct: &[&Path],
    support: &[&Path],
) -> SingleConeBuildRequest {
    let locators = |paths: &[&Path]| {
        paths
            .iter()
            .map(|path| HostArtifactLocator::new(*path).unwrap())
            .collect()
    };
    SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(root),
        },
        ExplicitDependencyInputs::new(locators(direct), locators(support)),
        TrustedCoreInput::DependenciesOrDefault {
            sysroot: sysroot.to_owned(),
        },
        target.clone(),
        SlibOutputDestination::new(output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
}

pub(super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    workspace: &Path,
    core: &Path,
) {
    let root = workspace.join("direct-library");
    let absent = workspace.join("absent-direct-sysroot");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        include_str!(
            "../../../../../../../tests/fixtures/core-library/direct-build/standalone.toml"
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("src/main.scoop"),
        include_str!(
            "../../../../../../../tests/fixtures/core-library/direct-build/standalone.scoop"
        ),
    )
    .unwrap();
    let explicit = workspace.join("direct-explicit.slib");
    request(target, &root, &explicit, &absent, &[core], &[])
        .build_and_publish()
        .unwrap();
    assert!(!absent.exists());

    let sysroot = workspace.join("artifact-only-sysroot");
    let slot =
        scoop_toolchain::TrustedCoreSlotLayoutV1::new(&sysroot, target.lir_target_selection());
    std::fs::create_dir_all(slot.artifact().parent().unwrap()).unwrap();
    std::fs::copy(core, slot.artifact()).unwrap();
    let implicit = workspace.join("direct-implicit.slib");
    request(target, &root, &implicit, &sysroot, &[], &[])
        .build_and_publish()
        .unwrap();
    assert_eq!(
        std::fs::read(&explicit).unwrap(),
        std::fs::read(&implicit).unwrap()
    );
    assert!(!sysroot.join("lib/scoop.core/src").exists());
    rejection::check(target, &root, workspace, &sysroot, core, slot.artifact());
}
