use scoop_manifest::{LoadedConeManifest, ManifestRootLocator};

use super::CurrentConeInput;

/// Existing compiler fixtures declare exact coordinates. Prepare their bound
/// configuration in the test caller, before entering the compiler request.
pub(super) fn expanded_fixture(root: ManifestRootLocator) -> CurrentConeInput {
    let manifest = scoop_toolchain::load_cone_manifest(&root).unwrap();
    CurrentConeInput::ExpandedManifest {
        manifest: Box::new(
            LoadedConeManifest::from_expanded(
                manifest.real_root().to_owned(),
                manifest.manifest_path().to_owned(),
                manifest.source_text().to_owned(),
                manifest.parsed().clone().into_bound(),
                manifest.metadata().clone(),
            )
            .unwrap(),
        ),
    }
}
