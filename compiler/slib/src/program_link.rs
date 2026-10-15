//! Saved artifact data and lazy machine members used by the final program link.
use crate::{BootstrapManifest, MergedOdrDefinitions};
use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_lir as lir;

mod data;
mod error;
mod graph;
mod native;
mod read;
pub use data::{ProgramLinkData, ProgramLinkMember, ProgramLinkRequirement};
pub use error::ProgramLinkReadError;
use error::error;
pub use read::read_program_link_closure;

pub struct ProgramLinkArtifact {
    pub(crate) manifest: BootstrapManifest,
    pub(crate) source: crate::ArtifactFile,
    pub(crate) identities: ValidatedIdentityGraph,
    pub(crate) native_cxx: bool,
    pub(crate) production: lir::ProgramLinkProduction,
    pub(crate) native_values: std::collections::BTreeMap<
        scoop_identity::PersistentExactTypeId,
        lir::LinkValueRepresentation,
    >,
    pub(crate) runtime_image_records: crate::RuntimeImageRecords,
    pub(crate) native_objects: Vec<crate::NativeLinkObject>,
}

impl ProgramLinkArtifact {
    pub fn object_bytes(&self, member: crate::SlibMemberId) -> Result<&[u8], ProgramLinkReadError> {
        self.source
            .member(self.source.directory().map_err(error)?, member)
            .map_err(error)?
            .ok_or_else(|| {
                error(format!(
                    "missing object member {member} in Cone {}",
                    self.identity()
                ))
            })
    }

    pub fn runtime_image_records(&self) -> &crate::RuntimeImageRecords {
        &self.runtime_image_records
    }

    pub fn identity(&self) -> ConeIdentity {
        self.manifest.cone().identity()
    }
    pub fn native_objects(&self) -> &[crate::NativeLinkObject] {
        &self.native_objects
    }
    pub fn manifest(&self) -> &BootstrapManifest {
        &self.manifest
    }
    pub fn identities(&self) -> &ValidatedIdentityGraph {
        &self.identities
    }
    pub fn native_cxx(&self) -> bool {
        self.native_cxx
    }
    pub fn production(&self) -> &lir::ProgramLinkProduction {
        &self.production
    }
    pub fn native_value(
        &self,
        exact: scoop_identity::PersistentExactTypeId,
    ) -> Option<lir::LinkValueRepresentation> {
        self.native_values.get(&exact).copied()
    }
}

pub struct ProgramLinkClosure {
    pub(crate) root: ConeIdentity,
    pub(crate) artifacts: Vec<ProgramLinkArtifact>,
    pub(crate) symbols: Vec<ProgramLinkData>,
    pub(crate) odr: MergedOdrDefinitions,
}

impl ProgramLinkClosure {
    pub fn root(&self) -> ConeIdentity {
        self.root
    }
    pub fn artifacts(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ProgramLinkArtifact, &ProgramLinkData)> {
        self.artifacts.iter().zip(&self.symbols)
    }
    pub fn odr_definitions(&self) -> &MergedOdrDefinitions {
        &self.odr
    }
}
