use super::{CoreCompilerProtocolSurfaceV1, generic_entry};
use scoop_identity::PersistentGenericTypeId;

impl CoreCompilerProtocolSurfaceV1 {
    pub fn character_source_type(&self) -> scoop_identity::PersistentTypeId {
        super::concrete_entry(self.fundamental_types().entries(), 15)
    }

    /// Source declarations behind normalized pointer signatures. Their access
    /// and representation still come from the actual provider's nominal table.
    pub fn pointer_source_type(&self) -> PersistentGenericTypeId {
        generic_entry(self.fundamental_types.entries(), 13)
    }

    pub fn function_pointer_source_type(&self) -> PersistentGenericTypeId {
        generic_entry(self.fundamental_types.entries(), 14)
    }
}
