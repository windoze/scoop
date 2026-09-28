use super::*;
use scoop_identity::{DecodedCallableDefinitionOwner, PersistentIdResolver};

/// A dependency key is not an authorized imported use or a selected handle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirTypeBridgeTargetV1 {
    Type(PersistentExactTypeId),
    Callable(CallableDefinitionOwner),
    Dispatch(PersistentExactTypeId),
    Object(PersistentObjectValueId),
    ShapeSupport(PersistentTypeId),
    InitializationUnit(PersistentInitializationUnitId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedMirTypeBridgeTargetV1 {
    Type(DecodedPersistentId<PersistentExactTypeId>),
    Callable(DecodedCallableDefinitionOwner),
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
    ) -> Result<MirTypeBridgeTargetV1, MirTypeBridgeReferenceError> {
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
