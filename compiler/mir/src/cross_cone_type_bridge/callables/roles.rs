use super::*;
use scoop_identity::PersistentIdResolver;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirCallableLoweringRoleV1 {
    Ordinary,
    ClassInitializer {
        owner: PersistentExactTypeId,
    },
    ValueConstructor {
        owner: PersistentExactTypeId,
    },
    PrimaryValueConstructor {
        owner: PersistentExactTypeId,
    },
    Accessor,
    DispatchAdjust {
        target: StrongCallableDefinitionOwner,
    },
    BoxingAdjust {
        target: StrongCallableDefinitionOwner,
    },
    ObjectEnsure {
        unit: PersistentInitializationUnitId,
    },
    ObjectInitializer {
        unit: PersistentInitializationUnitId,
    },
    PureVirtualTrap {
        slot: PersistentDispatchSlotId,
    },
    DerivedEquality {
        owner: PersistentExactTypeId,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedMirCallableLoweringRoleV1 {
    Ordinary,
    ClassInitializer {
        owner: DecodedPersistentId<PersistentExactTypeId>,
    },
    ValueConstructor {
        owner: DecodedPersistentId<PersistentExactTypeId>,
    },
    PrimaryValueConstructor {
        owner: DecodedPersistentId<PersistentExactTypeId>,
    },
    Accessor,
    DispatchAdjust {
        target: DecodedStrongCallableDefinitionOwner,
    },
    BoxingAdjust {
        target: DecodedStrongCallableDefinitionOwner,
    },
    ObjectEnsure {
        unit: DecodedPersistentId<PersistentInitializationUnitId>,
    },
    ObjectInitializer {
        unit: DecodedPersistentId<PersistentInitializationUnitId>,
    },
    PureVirtualTrap {
        slot: DecodedPersistentId<PersistentDispatchSlotId>,
    },
    DerivedEquality {
        owner: DecodedPersistentId<PersistentExactTypeId>,
    },
}
impl DecodedMirCallableLoweringRoleV1 {
    pub(super) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<MirCallableLoweringRoleV1, MirCallableBridgeError> {
        Ok(match self {
            Self::Ordinary => MirCallableLoweringRoleV1::Ordinary,
            Self::ClassInitializer { owner } => MirCallableLoweringRoleV1::ClassInitializer {
                owner: graph.resolve(owner)?,
            },
            Self::ValueConstructor { owner } => MirCallableLoweringRoleV1::ValueConstructor {
                owner: graph.resolve(owner)?,
            },
            Self::PrimaryValueConstructor { owner } => {
                MirCallableLoweringRoleV1::PrimaryValueConstructor {
                    owner: graph.resolve(owner)?,
                }
            }
            Self::Accessor => MirCallableLoweringRoleV1::Accessor,
            Self::DispatchAdjust { target } => MirCallableLoweringRoleV1::DispatchAdjust {
                target: target.resolve(graph)?,
            },
            Self::BoxingAdjust { target } => MirCallableLoweringRoleV1::BoxingAdjust {
                target: target.resolve(graph)?,
            },
            Self::ObjectEnsure { unit } => MirCallableLoweringRoleV1::ObjectEnsure {
                unit: graph.resolve(unit)?,
            },
            Self::ObjectInitializer { unit } => MirCallableLoweringRoleV1::ObjectInitializer {
                unit: graph.resolve(unit)?,
            },
            Self::PureVirtualTrap { slot } => MirCallableLoweringRoleV1::PureVirtualTrap {
                slot: graph.resolve(slot)?,
            },
            Self::DerivedEquality { owner } => MirCallableLoweringRoleV1::DerivedEquality {
                owner: graph.resolve(owner)?,
            },
        })
    }
}
macro_rules! encode_role {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                let (kind, value): (u64, Option<&dyn WireEncode>) = match self {
                    Self::Ordinary => (1, None),
                    Self::ClassInitializer { owner } => (2, Some(owner)),
                    Self::ValueConstructor { owner } => (3, Some(owner)),
                    Self::Accessor => (4, None),
                    Self::DispatchAdjust { target } => (5, Some(target)),
                    Self::BoxingAdjust { target } => (6, Some(target)),
                    Self::ObjectEnsure { unit } => (7, Some(unit)),
                    Self::ObjectInitializer { unit } => (8, Some(unit)),
                    Self::PureVirtualTrap { slot } => (9, Some(slot)),
                    Self::DerivedEquality { owner } => (10, Some(owner)),
                    Self::PrimaryValueConstructor { owner } => (11, Some(owner)),
                };
                tag(encoder, if value.is_some() { 2 } else { 1 }, kind)?;
                if let Some(value) = value {
                    encoder.field(1)?;
                    value.encode(encoder)?;
                }
                Ok(())
            }
        }
    };
}
encode_role!(MirCallableLoweringRoleV1);
encode_role!(DecodedMirCallableLoweringRoleV1);
impl WireDecode for DecodedMirCallableLoweringRoleV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        let kind = decoder.field(0, Decoder::unsigned)?;
        fields(decoder, count, if matches!(kind, 1 | 4) { 1 } else { 2 })?;
        Ok(match kind {
            1 => Self::Ordinary,
            2 => Self::ClassInitializer {
                owner: decoder.field(1, DecodedPersistentId::decode)?,
            },
            3 => Self::ValueConstructor {
                owner: decoder.field(1, DecodedPersistentId::decode)?,
            },
            4 => Self::Accessor,
            5 => Self::DispatchAdjust {
                target: decoder.field(1, DecodedStrongCallableDefinitionOwner::decode)?,
            },
            6 => Self::BoxingAdjust {
                target: decoder.field(1, DecodedStrongCallableDefinitionOwner::decode)?,
            },
            7 => Self::ObjectEnsure {
                unit: decoder.field(1, DecodedPersistentId::decode)?,
            },
            8 => Self::ObjectInitializer {
                unit: decoder.field(1, DecodedPersistentId::decode)?,
            },
            9 => Self::PureVirtualTrap {
                slot: decoder.field(1, DecodedPersistentId::decode)?,
            },
            10 => Self::DerivedEquality {
                owner: decoder.field(1, DecodedPersistentId::decode)?,
            },
            11 => Self::PrimaryValueConstructor {
                owner: decoder.field(1, DecodedPersistentId::decode)?,
            },
            tag => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        })
    }
}
