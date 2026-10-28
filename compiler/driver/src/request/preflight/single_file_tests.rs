//! A compiled std-coordinate library for dependency and pipeline tests.
use std::path::{Path, PathBuf};

use super::*;
use crate::{ExplicitDependencyInputs, HostArtifactLocator, SlibOutputDestination};

pub(super) fn standard_library(
    target: &scoop_toolchain::ResolvedTargetProfile,
    core: &Path,
    workspace: &Path,
) -> PathBuf {
    let root = workspace.join("single-file-std");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("Cone.toml"), "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.std\"\nversion = \"0.1.0\"\nkind = \"library\"\n").unwrap();
    // Standard API behavior is covered with the actual sysroot by CLI fixtures.
    std::fs::write(
        root.join("src/main.scoop"),
        "package scoop\npublic fun singleFileStandardValue(): Int = 1\n",
    )
    .unwrap();
    let output = root.join("scoop.std.slib");
    SingleConeBuildRequest::new(
        crate::request::test_support::expanded_fixture(
            scoop_manifest::ManifestRootLocator::cone_directory(&root),
        ),
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core).unwrap()),
        target.clone(),
        SlibOutputDestination::new(&output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish()
    .unwrap();
    output
}
