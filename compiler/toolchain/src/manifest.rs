//! Manifest parsing uses the driver's actual source-condition target catalog.

use scoop_lir::TargetProfileId;
use scoop_manifest::{
    LoadedConeManifest, ManifestParseError, ManifestRootError, ManifestRootLocator,
    ParsedConeManifest, SourceTarget,
};

pub const SOURCE_TARGETS: &[SourceTarget<'static>] = &[
    SourceTarget {
        id: scoop_target_data::DARWIN_AARCH64_ID,
        triple: "aarch64-apple-darwin",
        os: "darwin",
        arch: "aarch64",
        env: "none",
    },
    SourceTarget {
        id: scoop_target_data::LINUX_X86_64_GNU_ID,
        triple: "x86_64-unknown-linux-gnu",
        os: "linux",
        arch: "x86_64",
        env: "gnu",
    },
    SourceTarget {
        id: scoop_target_data::LINUX_X86_64_MUSL_ID,
        triple: "x86_64-unknown-linux-musl",
        os: "linux",
        arch: "x86_64",
        env: "musl",
    },
];

pub fn parse_cone_manifest(source: &str) -> Result<ParsedConeManifest, ManifestParseError> {
    scoop_manifest::parse_cone_manifest(source, SOURCE_TARGETS)
}

pub fn load_cone_manifest(
    locator: &ManifestRootLocator,
) -> Result<LoadedConeManifest, ManifestRootError> {
    scoop_manifest::load_cone_manifest(locator, SOURCE_TARGETS)
}

pub(crate) fn source_target(
    id: TargetProfileId,
) -> Result<SourceTarget<'static>, crate::ToolchainError> {
    SOURCE_TARGETS
        .iter()
        .find(|target| target.id == id)
        .copied()
        .ok_or_else(|| {
            crate::ToolchainError(format!(
                "unsupported native source target {}/{}",
                id.canonical_name(),
                id.major_version()
            ))
        })
}
