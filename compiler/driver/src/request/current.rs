use super::*;

/// The compiler receives a bound Cone configuration or an explicit source file.
#[derive(Debug)]
pub enum CurrentConeInput {
    ExpandedManifest {
        manifest: Box<scoop_manifest::LoadedConeManifest>,
    },
    SingleFile {
        source: SingleFileLocator,
    },
}
