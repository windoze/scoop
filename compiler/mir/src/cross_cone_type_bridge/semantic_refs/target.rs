use super::*;
use scoop_identity::{DecodedStrongCallableDefinitionOwner, PersistentIdResolver};

/// A dependency key is not an authorized imported use or a selected handle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirTypeBridgeTargetV1 {
    Type(PersistentExactTypeId),
    Callable(StrongCallableDefinitionOwner),
    Dispatch(PersistentExactTypeId),
    Object(PersistentObjectValueId),
    ShapeSupport(PersistentTypeId),
    InitializationUnit(PersistentInitializationUnitId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedMirTypeBridgeTargetV1 {
    Type(DecodedPersistentId<PersistentExactTypeId>),
    Callable(DecodedStrongCallableDefinitionOwner),
    Dispatch(DecodedPersistentId<PersistentExactTypeId>),
    Object(DecodedPersistentId<PersistentObjectValueId>),
    ShapeSupport(DecodedPersistentId<PersistentTypeId>),
    InitializationUnit(DecodedPersistentId<PersistentInitializationUnitId>),
}
impl DecodedMirTypeBridgeTargetV1 {
    /// Identity resolution grants no provider ownership or selection proof.
    pub fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<MirTypeBridgeTargetV1, MirTypeBridgeReferenceError> {
        meter.charge_work(1, &WirePath::root())?;
        Ok(match self {
            Self::Type(exact) => MirTypeBridgeTargetV1::Type(graph.resolve(exact)?),
            Self::Callable(target) => MirTypeBridgeTargetV1::Callable(target.resolve(graph)?),
            Self::Dispatch(owner) => MirTypeBridgeTargetV1::Dispatch(graph.resolve(owner)?),
            Self::Object(value) => MirTypeBridgeTargetV1::Object(graph.resolve(value)?),
            Self::ShapeSupport(source) => {
                MirTypeBridgeTargetV1::ShapeSupport(graph.resolve(source)?)
            }
            Self::InitializationUnit(unit) => {
                MirTypeBridgeTargetV1::InitializationUnit(graph.resolve(unit)?)
            }
        })
    }
}
