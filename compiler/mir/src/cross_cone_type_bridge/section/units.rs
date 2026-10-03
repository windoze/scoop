use super::*;
use scoop_identity::{
    GeneratedCallableKey, InitializationCallableRole, PersistentGeneratedCallableId,
};

mod shared;
pub use shared::replay_source_initialization_units;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTypeBridgeInitializationUnitV1 {
    unit: PersistentInitializationUnitId,
    initializer: StrongCallableDefinitionOwner,
    ensure: StrongCallableDefinitionOwner,
    signature: MirBridgeCallableSignatureV1,
}
impl MirTypeBridgeInitializationUnitV1 {
    pub fn new(
        unit: PersistentInitializationUnitId,
        initializer: PersistentGeneratedCallableId,
        ensure: PersistentGeneratedCallableId,
        signature: MirBridgeCallableSignatureV1,
    ) -> Self {
        Self {
            unit,
            initializer: StrongCallableDefinitionOwner::GeneratedCallable(initializer),
            ensure: StrongCallableDefinitionOwner::GeneratedCallable(ensure),
            signature,
        }
    }
    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.unit
    }
    pub const fn initializer(&self) -> StrongCallableDefinitionOwner {
        self.initializer
    }
    pub const fn ensure(&self) -> StrongCallableDefinitionOwner {
        self.ensure
    }
    pub const fn signature(&self) -> &MirBridgeCallableSignatureV1 {
        &self.signature
    }
}
