use super::*;

/// A logical published-root read; the LIR stage derives its physical storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirObjectValueReadPlanV1 {
    PublishedSingletonRoot { object: PersistentExactTypeId },
}
impl MirObjectValueReadPlanV1 {
    pub const fn object(self) -> PersistentExactTypeId {
        match self {
            Self::PublishedSingletonRoot { object } => object,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeMirObjectValueV1 {
    pub(super) value: PersistentObjectValueId,
    pub(super) backing: PersistentExactTypeId,
    pub(super) unit: PersistentInitializationUnitId,
    pub(super) ensure: StrongCallableDefinitionOwner,
    pub(super) read: MirObjectValueReadPlanV1,
    pub(super) provider: ConeIdentity,
}
impl ParamFreeMirObjectValueV1 {
    pub fn try_new(
        authority: MirObjectBridgeAuthority<'_>,
        value: PersistentObjectValueId,
        backing: PersistentExactTypeId,
        unit: PersistentInitializationUnitId,
        ensure: StrongCallableDefinitionOwner,
        read: MirObjectValueReadPlanV1,
    ) -> Result<Self, MirObjectBridgeError> {
        let provider = authority.validate_object(value, backing, unit, ensure, read)?;
        Ok(Self {
            value,
            backing,
            unit,
            ensure,
            read,
            provider,
        })
    }
    pub const fn value(&self) -> PersistentObjectValueId {
        self.value
    }
    pub const fn backing(&self) -> PersistentExactTypeId {
        self.backing
    }
    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.unit
    }
    pub const fn ensure(&self) -> StrongCallableDefinitionOwner {
        self.ensure
    }
    pub const fn read(&self) -> MirObjectValueReadPlanV1 {
        self.read
    }
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
}

#[derive(Clone, Copy)]
pub struct MirObjectBridgeAuthority<'a> {
    pub identities: &'a ValidatedIdentityGraph,
    pub types: &'a dyn MirTypeBridgeTypeLookupV1,
    pub callables: &'a dyn MirTypeBridgeCallableLookupV1,
}
