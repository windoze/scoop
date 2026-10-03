//! Artifact-only machine inputs shared with the normal object verifiers.
use crate::{BootstrapManifest, MergedOdrDefinitions, ReplayedLayoutLinkSymbolUsesV1};
use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_lir as lir;

mod graph;
mod read;
pub use read::read_program_link_closure;

pub struct ProgramLinkArtifact {
    pub(crate) manifest: BootstrapManifest,
    pub(crate) identities: ValidatedIdentityGraph,
    pub(crate) foundation: lir::ConeLirFoundation,
    pub(crate) production: lir::ConeProductionSectionV2,
    pub(crate) ordinary: lir::CrossConeLirBridgeSectionV1,
    pub(crate) layout: lir::PhysicalImportsReplayedLayoutAbiSectionV1,
}

impl ProgramLinkArtifact {
    pub fn identity(&self) -> ConeIdentity {
        self.manifest.cone().identity()
    }
    pub fn manifest(&self) -> &BootstrapManifest {
        &self.manifest
    }
    pub fn identities(&self) -> &ValidatedIdentityGraph {
        &self.identities
    }
    pub fn foundation(&self) -> &lir::ConeLirFoundation {
        &self.foundation
    }
    pub fn production(&self) -> &lir::ConeProductionSectionV2 {
        &self.production
    }
    pub fn layout(&self) -> &lir::PhysicalImportsReplayedLayoutAbiSectionV1 {
        &self.layout
    }
}

pub struct ProgramLinkClosure {
    pub(crate) root: ConeIdentity,
    pub(crate) artifacts: Vec<ProgramLinkArtifact>,
    pub(crate) symbols: Vec<ReplayedLayoutLinkSymbolUsesV1>,
    pub(crate) odr: MergedOdrDefinitions,
}

impl ProgramLinkClosure {
    pub fn root(&self) -> ConeIdentity {
        self.root
    }
    pub fn artifacts(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ProgramLinkArtifact, &ReplayedLayoutLinkSymbolUsesV1)>
    {
        self.artifacts.iter().zip(&self.symbols)
    }
    pub fn odr_definitions(&self) -> &MergedOdrDefinitions {
        &self.odr
    }
}

#[derive(Debug)]
pub struct ProgramLinkReadError(pub String);
impl std::fmt::Display for ProgramLinkReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ProgramLinkReadError {}
fn error(value: impl std::fmt::Display) -> ProgramLinkReadError {
    ProgramLinkReadError(value.to_string())
}
