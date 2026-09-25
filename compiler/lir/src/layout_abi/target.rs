use super::*;
use scoop_identity::PersistentIdResolver;

/// The five independent semantic tables exported by one layout/ABI section.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LayoutAbiSemanticTargetV1 {
    Layout(PersistentLayoutId),
    Descriptor(PersistentExactTypeId),
    Dispatch(PersistentDispatchTableId),
    Callable(StrongCallableDefinitionOwner),
    ShapeSupport(PersistentTypeId),
}

/// Untrusted wire form. Resolution verifies typed identities only; the
/// containing section separately proves terminal ownership and selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedLayoutAbiSemanticTargetV1 {
    Layout(DecodedPersistentId<PersistentLayoutId>),
    Descriptor(DecodedPersistentId<PersistentExactTypeId>),
    Dispatch(DecodedPersistentId<PersistentDispatchTableId>),
    Callable(DecodedStrongCallableDefinitionOwner),
    ShapeSupport(DecodedPersistentId<PersistentTypeId>),
}

impl DecodedLayoutAbiSemanticTargetV1 {
    pub fn resolve(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<LayoutAbiSemanticTargetV1, LayoutAbiDependencyError> {
        Ok(match self {
            Self::Layout(layout) => LayoutAbiSemanticTargetV1::Layout(identities.resolve(layout)?),
            Self::Descriptor(exact) => {
                LayoutAbiSemanticTargetV1::Descriptor(identities.resolve(exact)?)
            }
            Self::Dispatch(table) => {
                LayoutAbiSemanticTargetV1::Dispatch(identities.resolve(table)?)
            }
            Self::Callable(target) => {
                LayoutAbiSemanticTargetV1::Callable(target.resolve(identities)?)
            }
            Self::ShapeSupport(source) => {
                LayoutAbiSemanticTargetV1::ShapeSupport(identities.resolve(source)?)
            }
        })
    }
}
