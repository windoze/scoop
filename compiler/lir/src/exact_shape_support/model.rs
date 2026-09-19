use scoop_identity::{ConeIdentity, PersistentExactTypeId, PersistentTypeId};

use crate::{LirTargetProfile, ParamFreeShapeSupportRolesV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeShapeSupportExportV1 {
    pub(super) source: PersistentTypeId,
    pub(super) exact: PersistentExactTypeId,
    pub(super) provider: ConeIdentity,
    pub(super) target: LirTargetProfile,
    pub(super) roles: ParamFreeShapeSupportRolesV1,
}

impl ParamFreeShapeSupportExportV1 {
    pub const fn source_nominal(&self) -> PersistentTypeId {
        self.source
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub const fn roles(&self) -> &ParamFreeShapeSupportRolesV1 {
        &self.roles
    }
}
