use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

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
fn operand_classifier_uses_only_the_closed_file_shapes() {
    let directory = TempDirectory::new();
    let source = directory.0.join("main.scoop");
    let manifest = directory.0.join("Cone.toml");
    let other = directory.0.join("other.toml");
    std::fs::write(&source, "fun main() {}\n").unwrap();
    std::fs::write(&manifest, "manifest").unwrap();
    std::fs::write(&other, "other").unwrap();

    assert!(matches!(
        classify_current_cone_operand(&directory.0).unwrap(),
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::ConeDirectory(_)
        }
    ));
    assert!(matches!(
        classify_current_cone_operand(&manifest).unwrap(),
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::ExactConeManifestFile(_)
        }
    ));
    assert!(matches!(
        classify_current_cone_operand(&source).unwrap(),
        CurrentConeInput::SingleFile { .. }
    ));
    assert!(matches!(
        classify_current_cone_operand(&other).unwrap_err().kind(),
        CurrentConeOperandErrorKind::UnsupportedFile
    ));
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
fn single_file_dependencies_fail_before_toolchain_resolution() {
    let directory = TempDirectory::new();
    let source = directory.0.join("main.scoop");
    std::fs::write(&source, "fun main() {}\n").unwrap();

    assert!(matches!(
        normalize_direct_build_request(
            &source,
            vec![PathBuf::from("dependency.slib")],
            Vec::new(),
            directory.0.join("main.slib"),
            DiagnosticOutputPolicy::Human,
            StageDumpPolicy::None,
        ),
        Err(BuildRequestNormalizationError::Request(
            SingleConeBuildRequestError::SingleFileHasDependencies
        ))
    ));
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
    std::fs::write(&manifest, "manifest").unwrap();
    std::fs::write(&core, "core").unwrap();
    std::fs::write(&dependency, "dependency").unwrap();
    let current = CurrentConeInput::Manifest {
        root: ManifestRootLocator::cone_directory(directory.0.clone()),
    };
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
    std::fs::write(&manifest, "manifest").unwrap();
    std::fs::write(&core, "core").unwrap();
    let current = CurrentConeInput::Manifest {
        root: ManifestRootLocator::cone_directory(directory.0.clone()),
    };
    let dependencies = ExplicitDependencyInputs::new(Vec::new(), Vec::new());
    let trusted_core = TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap());
    let output = SlibOutputDestination::new(directory.0.join("output.slib")).unwrap();

    validate_output_isolation(&current, &dependencies, &trusted_core, &output).unwrap();
}

#[test]
fn policy_projection_is_total() {
    assert_eq!(
        map_diagnostic_policy(scoop_protocol::DiagnosticOutputPolicyV1::Structured),
        DiagnosticOutputPolicy::Structured
    );
    for (wire, expected) in [
        (StageDumpKindV1::Ast, StageDumpKind::Ast),
        (StageDumpKindV1::Hir, StageDumpKind::Hir),
        (StageDumpKindV1::Mir, StageDumpKind::Mir),
        (StageDumpKindV1::Lir, StageDumpKind::Lir),
    ] {
        assert_eq!(
            map_dump_policy(StageDumpPolicyV1::Stage(wire)),
            StageDumpPolicy::Stage(expected)
        );
    }
}

mod core_dependencies;
