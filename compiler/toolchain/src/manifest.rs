//! Manifest parsing uses the driver's actual source-condition target catalog.

use scoop_lir::TargetProfileId;
use scoop_manifest::{
    LoadedConeManifest, ManifestParseError, ManifestRootError, ManifestRootLocator,
    ParsedConeManifest, SourceTarget,
};

pub const SOURCE_TARGETS: &[SourceTarget<'static>] = &[
    SourceTarget {
        id: TargetProfileId::DarwinAarch64,
        triple: "aarch64-apple-darwin",
        os: "darwin",
        arch: "aarch64",
        env: "none",
    },
    SourceTarget {
        id: TargetProfileId::LinuxX86_64Gnu,
        triple: "x86_64-unknown-linux-gnu",
        os: "linux",
        arch: "x86_64",
        env: "gnu",
    },
    SourceTarget {
        id: TargetProfileId::LinuxX86_64Musl,
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
