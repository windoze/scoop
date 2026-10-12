//! Existing target facts used by manifest grammar and semantic fixtures.

use crate::{
    LoadedConeManifest, ManifestParseError, ManifestRootError, ManifestRootLocator,
    ParsedConeManifest, SourceTarget,
};

pub(crate) const SOURCE_TARGETS: &[SourceTarget<'static>] = &[
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
    crate::parse_cone_manifest(source, SOURCE_TARGETS)
}

pub fn load_cone_manifest(
    locator: &ManifestRootLocator,
) -> Result<LoadedConeManifest, ManifestRootError> {
    crate::load_cone_manifest(locator, SOURCE_TARGETS)
}
