//! Archive assembly shared by the current Strong/ODR production path.

mod assembly;
use assembly::{LayerAssembly, StrongArtifactAssemblyError, build_section};

mod layout;
pub use layout::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongArtifactSectionV1 {
    HirFoundation,
    HirProduction,
    MirFoundation,
    MirProduction,
    LirFoundation,
    LirProduction,
    LinkIdentityClosure,
    HirCrossConeInterface,
    HirCrossConeTypeSemantics,
    MirCrossConeBridge,
    MirCrossConeTypeBridge,
    LirCrossConeBridge,
    LirCrossConeLayoutAbi,
    LinkSupport,
}
