//! Manifest parsing uses the driver's actual source-condition target catalog.

use scoop_lir::TargetProfileId;
use scoop_manifest::{
    LoadedConeManifest, ManifestParseError, ManifestRootError, ManifestRootLocator,
    ParsedConeManifest, SourceTarget,
};

pub use scoop_target_data::SOURCE_CATALOG_KEY;

const fn source_projection(target: scoop_target_data::TargetDefinition) -> SourceTarget<'static> {
    SourceTarget {
        id: target.selection.target().id(),
        triple: target.canonical(),
        os: target.os,
        arch: target.arch,
        env: target.env,
    }
}

pub const SOURCE_TARGETS: &[SourceTarget<'static>] = &{
    let catalog = scoop_target_data::TARGETS;
    let mut targets = [source_projection(catalog[0]); scoop_target_data::TARGETS.len()];
    let mut index = 0;
    while index < catalog.len() {
        targets[index] = source_projection(catalog[index]);
        index += 1;
    }
    targets
};

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
