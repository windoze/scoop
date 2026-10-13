//! Presentation data for tests in one compiled root Cone.

use scoop_identity::NormalizedSourcePath;

use crate::{
    BootstrapManifest, ManifestSection, MemberPurposeSet, manifest_test_directory_capability,
};

#[cfg(test)]
mod tests;
mod wire;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestSourceLocation {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestCaseListing {
    pub name: String,
    pub display_name: String,
    pub location: TestSourceLocation,
    pub ignore_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestContainerListing {
    pub qualified_name: String,
    pub location: TestSourceLocation,
    pub cases: Vec<TestCaseListing>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestDirectory {
    pub containers: Vec<TestContainerListing>,
}

impl TestDirectory {
    pub(crate) fn section(&self) -> Result<ManifestSection, TestDirectoryError> {
        let payload = scoop_wire::encode(self).map_err(error)?;
        ManifestSection::new(
            manifest_test_directory_capability(),
            MemberPurposeSet::NONE,
            payload,
        )
        .map_err(error)
    }

    pub(crate) fn from_manifest(
        manifest: &BootstrapManifest,
    ) -> Result<Option<Self>, TestDirectoryError> {
        let Some(section) = manifest
            .sections()
            .iter()
            .find(|s| s.capability() == &manifest_test_directory_capability())
        else {
            return Ok(None);
        };
        if manifest.cone().kind() != crate::ConeKind::Executable
            || section.required_for() != MemberPurposeSet::NONE
        {
            return Err(error(
                "test directory requires an executable with no semantic purposes",
            ));
        }
        Self::read(section.payload()).map(Some)
    }

    fn read(payload: &[u8]) -> Result<Self, TestDirectoryError> {
        let directory: Self = scoop_wire::decode_canonical(payload).map_err(error)?;
        for container in &directory.containers {
            check_location(&container.location)?;
            for case in &container.cases {
                check_location(&case.location)?;
            }
        }
        Ok(directory)
    }
}

fn check_location(location: &TestSourceLocation) -> Result<(), TestDirectoryError> {
    let path = NormalizedSourcePath::new(&location.path).map_err(error)?;
    if path.as_str() != location.path || location.line == 0 || location.column == 0 {
        return Err(error(
            "test location requires a normalized path and one-based line/column",
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct TestDirectoryError(String);

fn error(message: impl std::fmt::Display) -> TestDirectoryError {
    TestDirectoryError(message.to_string())
}

impl std::fmt::Display for TestDirectoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid test directory: {}", self.0)
    }
}

impl std::error::Error for TestDirectoryError {}
