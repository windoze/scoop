use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use scoop_manifest::ManifestRootLocator;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "scoop-single-cone-request-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn direct_requests_reject_manifest_inputs_without_reading_them() {
    let directory = TempDirectory::new();
    let manifest = directory.0.join("Cone.toml");
    std::fs::write(&manifest, "not valid TOML").unwrap();
    for input in [&directory.0, &manifest] {
        assert!(matches!(
            normalize_direct_check_request(
                input,
                Vec::new(),
                Vec::new(),
                DiagnosticOutputPolicy::Human,
                Default::default()
            ),
            Err(BuildRequestNormalizationError::ManifestRequiresScoop)
        ));
        assert!(matches!(
            normalize_direct_build_request(
                input,
                Vec::new(),
                Vec::new(),
                directory.0.join("out.slib"),
                DiagnosticOutputPolicy::Human,
                StageDumpPolicy::None,
                Default::default()
            ),
            Err(BuildRequestNormalizationError::ManifestRequiresScoop)
        ));
    }
    assert_eq!(std::fs::read_to_string(manifest).unwrap(), "not valid TOML");
}

#[test]
fn explicit_dependency_roles_are_preserved() {
    let locator = HostArtifactLocator::new("dependency.slib").unwrap();
    let inputs = ExplicitDependencyInputs::new(
        vec![locator.clone()],
        vec![HostArtifactLocator::new("support.slib").unwrap()],
    );
    assert_eq!(inputs.direct()[0].as_path(), Path::new("dependency.slib"));
    assert_eq!(inputs.support()[0].as_path(), Path::new("support.slib"));
}

#[test]
fn single_file_dependencies_are_preserved_in_the_request() {
    let directory = TempDirectory::new();
    let source = directory.0.join("main.scoop");
    std::fs::write(&source, "fun main() {}\n").unwrap();
    let std = directory.0.join("std.slib");
    let support = directory.0.join("support.slib");
    std::fs::write(&std, []).unwrap();
    std::fs::write(&support, []).unwrap();
    let request = normalize_direct_build_request(
        &source,
        vec![std],
        vec![support],
        directory.0.join("main.slib"),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
        Default::default(),
    )
    .unwrap();
    assert_eq!(request.dependencies.direct().len(), 1);
    assert_eq!(request.dependencies.support().len(), 1);
}

#[test]
fn output_destination_requires_the_exact_slib_extension() {
    assert!(matches!(
        SlibOutputDestination::new("output.bin"),
        Err(SingleConeBuildRequestError::InvalidOutputExtension { .. })
    ));
}

#[test]
fn output_isolation_rejects_dependency_aliases_before_writing() {
    let directory = TempDirectory::new();
    let manifest = directory.0.join("Cone.toml");
    let core = directory.0.join("core.slib");
    let dependency = directory.0.join("dependency.slib");
    std::fs::write(
        &manifest,
        "schema=1\n[cone]\ngroup='test'\nname='output'\nversion='1.0.0'\nkind='library'\n",
    )
    .unwrap();
    std::fs::write(&core, "core").unwrap();
    std::fs::write(&dependency, "dependency").unwrap();
    let current = crate::request::test_support::expanded_fixture(
        ManifestRootLocator::cone_directory(directory.0.clone()),
    );
    let dependencies = ExplicitDependencyInputs::new(
        vec![HostArtifactLocator::new(&dependency).unwrap()],
        Vec::new(),
    );
    let trusted_core = TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap());
    let output = SlibOutputDestination::new(&dependency).unwrap();

    assert!(matches!(
        validate_output_isolation(&current, &dependencies, &trusted_core, &output),
        Err(SingleConeBuildRequestError::OutputIsolation {
            kind: OutputIsolationErrorKind::AliasesInput {
                role: OutputAliasRole::DirectArtifact { index: 0 },
                ..
            },
            ..
        })
    ));
}

#[test]
fn output_isolation_accepts_a_new_file_in_an_existing_directory() {
    let directory = TempDirectory::new();
    let manifest = directory.0.join("Cone.toml");
    let core = directory.0.join("core.slib");
    std::fs::write(
        &manifest,
        "schema=1\n[cone]\ngroup='test'\nname='output'\nversion='1.0.0'\nkind='library'\n",
    )
    .unwrap();
    std::fs::write(&core, "core").unwrap();
    let current = crate::request::test_support::expanded_fixture(
        ManifestRootLocator::cone_directory(directory.0.clone()),
    );
    let dependencies = ExplicitDependencyInputs::new(Vec::new(), Vec::new());
    let trusted_core = TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap());
    let output = SlibOutputDestination::new(directory.0.join("output.slib")).unwrap();

    validate_output_isolation(&current, &dependencies, &trusted_core, &output).unwrap();
}

mod core_dependencies;
