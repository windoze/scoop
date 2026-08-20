use super::*;

mod wire_impl;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSelectedTypeUseV1 {
    Signature {
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    Representation {
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    Construct {
        exact: DecodedPersistentId<PersistentExactTypeId>,
        declaration: DecodedSelectedTypeConstructionV1,
    },
    MemberCall {
        receiver: DecodedPersistentId<PersistentExactTypeId>,
        declaration: DecodedInheritanceCallableDeclarationV1,
    },
    SlotCall {
        receiver: DecodedPersistentId<PersistentExactTypeId>,
        slot: DecodedPersistentId<PersistentDispatchSlotId>,
    },
    TypeTest {
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    SingletonValue {
        exact: DecodedPersistentId<PersistentExactTypeId>,
        value: DecodedPersistentId<PersistentObjectValueId>,
    },
    Inheritance {
        derived: DecodedPersistentId<PersistentExactTypeId>,
        edge: DecodedSelectedDirectInheritanceEdgeV1,
    },
    ShapeSupport {
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}
impl DecodedSelectedTypeUseV1 {
    pub fn resolve<R: SelectedTypeUseResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<SelectedTypeUseV1, SelectedTypeUseResolutionError<E>> {
        self.resolve_at_depth(resolver)
    }
    pub(super) fn resolve_at_depth<R: SelectedTypeUseResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<SelectedTypeUseV1, SelectedTypeUseResolutionError<E>> {
        use SelectedTypeUseResolutionError::Reference;

        Ok(match self {
            Self::Signature { exact } => SelectedTypeUseV1::Signature {
                exact: resolver.resolve(exact).map_err(Reference)?,
            },
            Self::Representation { exact } => SelectedTypeUseV1::Representation {
                exact: resolver.resolve(exact).map_err(Reference)?,
            },
            Self::Construct { exact, declaration } => SelectedTypeUseV1::Construct {
                exact: resolver.resolve(exact).map_err(Reference)?,
                declaration: declaration.resolve(resolver).map_err(Reference)?,
            },
            Self::MemberCall {
                receiver,
                declaration,
            } => SelectedTypeUseV1::MemberCall {
                receiver: resolver.resolve(receiver).map_err(Reference)?,
                declaration: declaration.resolve(resolver).map_err(Reference)?,
            },
            Self::SlotCall { receiver, slot } => SelectedTypeUseV1::SlotCall {
                receiver: resolver.resolve(receiver).map_err(Reference)?,
                slot: resolver.resolve(slot).map_err(Reference)?,
            },
            Self::TypeTest { exact } => SelectedTypeUseV1::TypeTest {
                exact: resolver.resolve(exact).map_err(Reference)?,
            },
            Self::SingletonValue { exact, value } => SelectedTypeUseV1::SingletonValue {
                exact: resolver.resolve(exact).map_err(Reference)?,
                value: resolver.resolve(value).map_err(Reference)?,
            },
            Self::Inheritance { derived, edge } => SelectedTypeUseV1::Inheritance {
                derived: resolver.resolve(derived).map_err(Reference)?,
                edge: edge.resolve(resolver).map_err(Reference)?,
            },
            Self::ShapeSupport { exact } => SelectedTypeUseV1::ShapeSupport {
                exact: resolver.resolve(exact).map_err(Reference)?,
            },
        })
    }
}
