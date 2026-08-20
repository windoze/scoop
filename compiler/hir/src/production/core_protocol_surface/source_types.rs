use super::CoreCompilerProtocolSurfaceV1;
use scoop_identity::PersistentGenericTypeId;

impl CoreCompilerProtocolSurfaceV1 {
    pub fn float_source_type(&self, kind: crate::FloatKind) -> scoop_identity::PersistentTypeId {
        match kind {
            crate::FloatKind::F32 => self.fundamental_types.float,
            crate::FloatKind::F64 => self.fundamental_types.double,
        }
    }

    pub fn character_source_type(&self) -> scoop_identity::PersistentTypeId {
        self.fundamental_types().character
    }

    /// Source declarations behind normalized pointer signatures. Their access
    /// and representation still come from the actual provider's nominal table.
    pub fn pointer_source_type(&self) -> PersistentGenericTypeId {
        self.fundamental_types.ptr
    }

    pub fn function_pointer_source_type(&self) -> PersistentGenericTypeId {
        self.fundamental_types.fun_ptr
    }
}
