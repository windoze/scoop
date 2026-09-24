use scoop_identity::{
    DecodedCallableMaterialization, DecodedConcreteExpressionOrigin, DecodedPersistentId,
    DecodedPropertyOwner, PersistentExtensionPropertyId, PersistentIdResolver,
    PersistentPropertyId,
};
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::*;
use crate::HirDependencyCallSiteResolver;

mod encoding;

pub trait HirDependencyTypeSiteResolver<E>:
    HirDependencyCallSiteResolver<E>
    + PersistentIdResolver<PersistentLocalValueId, Error = E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
    + PersistentIdResolver<PersistentFieldId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
{
}

impl<R, E> HirDependencyTypeSiteResolver<E> for R where
    R: HirDependencyCallSiteResolver<E>
        + PersistentIdResolver<PersistentLocalValueId, Error = E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
        + PersistentIdResolver<PersistentFieldId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedHirDependencyTypeSiteV1 {
    Expression {
        root: DecodedCallableMaterialization,
        expression_index: u32,
        origin: Box<DecodedConcreteExpressionOrigin>,
        role: HirExpressionTypeRoleV1,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    CallableSignature {
        root: DecodedCallableMaterialization,
        position: HirCallableTypePositionV1,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    LocalValue {
        local: DecodedPersistentId<PersistentLocalValueId>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    BackingStorage {
        property: DecodedPropertyOwner,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    DelegateStorage {
        property: DecodedPropertyOwner,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    FieldStorage {
        field: DecodedPersistentId<PersistentFieldId>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    EnumVariantFieldStorage {
        field: DecodedPersistentId<PersistentEnumVariantFieldId>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    ConstructorInitializerResult {
        constructor: DecodedCallableMaterialization,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    InitializationCycleMessage {
        unit: DecodedPersistentId<PersistentInitializationUnitId>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl DecodedHirDependencyTypeSiteV1 {
    pub fn resolve<R: HirDependencyTypeSiteResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<HirDependencyTypeSiteV1, HirDependencyTypeSiteResolutionError<E>> {
        use HirDependencyTypeSiteResolutionError as Error;
        let bytes = scoop_wire::encoded_length(&self).map_err(|_| {
            WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            )
        })?;
        meter.charge_owned_bytes(bytes, path)?;
        meter.charge_work(bytes, path)?;
        meter.charge_nodes(2, path)?;
        if matches!(&self, Self::Expression { .. }) {
            meter
                .charge_owned_bytes(std::mem::size_of::<HirExpressionTypeSiteV1>() as u64, path)?;
        }
        Ok(match self {
            Self::Expression {
                root,
                expression_index,
                origin,
                role,
                exact,
            } => HirDependencyTypeSiteV1::new(
                ExecutableExpressionPosition {
                    root: root.resolve(resolver).map_err(Error::Identity)?,
                    expression_index,
                },
                (*origin).resolve(resolver).map_err(Error::Origin)?,
                role,
                resolver.resolve(exact).map_err(Error::Identity)?,
            ),
            Self::CallableSignature {
                root,
                position,
                exact,
            } => HirDependencyTypeSiteV1::CallableSignature {
                root: root.resolve(resolver).map_err(Error::Identity)?,
                position,
                exact: resolver.resolve(exact).map_err(Error::Identity)?,
            },
            Self::LocalValue { local, exact } => HirDependencyTypeSiteV1::LocalValue {
                local: resolver.resolve(local).map_err(Error::Identity)?,
                exact: resolver.resolve(exact).map_err(Error::Identity)?,
            },
            Self::BackingStorage { property, exact } => HirDependencyTypeSiteV1::BackingStorage {
                property: property.resolve(resolver).map_err(Error::Identity)?,
                exact: resolver.resolve(exact).map_err(Error::Identity)?,
            },
            Self::DelegateStorage { property, exact } => HirDependencyTypeSiteV1::DelegateStorage {
                property: property.resolve(resolver).map_err(Error::Identity)?,
                exact: resolver.resolve(exact).map_err(Error::Identity)?,
            },
            Self::FieldStorage { field, exact } => HirDependencyTypeSiteV1::FieldStorage {
                field: resolver.resolve(field).map_err(Error::Identity)?,
                exact: resolver.resolve(exact).map_err(Error::Identity)?,
            },
            Self::EnumVariantFieldStorage { field, exact } => {
                HirDependencyTypeSiteV1::EnumVariantFieldStorage {
                    field: resolver.resolve(field).map_err(Error::Identity)?,
                    exact: resolver.resolve(exact).map_err(Error::Identity)?,
                }
            }
            Self::ConstructorInitializerResult { constructor, exact } => {
                HirDependencyTypeSiteV1::ConstructorInitializerResult {
                    constructor: constructor.resolve(resolver).map_err(Error::Identity)?,
                    exact: resolver.resolve(exact).map_err(Error::Identity)?,
                }
            }
            Self::InitializationCycleMessage { unit, exact } => {
                HirDependencyTypeSiteV1::InitializationCycleMessage {
                    unit: resolver.resolve(unit).map_err(Error::Identity)?,
                    exact: resolver.resolve(exact).map_err(Error::Identity)?,
                }
            }
        })
    }
}

impl WireDecode for DecodedHirDependencyTypeSiteV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                super::wire::require_fields(decoder, fields, 6)?;
                Ok(Self::Expression {
                    root: decoder.field(1, DecodedCallableMaterialization::decode)?,
                    expression_index: decoder.field(2, Decoder::u32)?,
                    origin: decoder.field(3, |decoder| {
                        let path = decoder.path().clone();
                        decoder.meter().charge_owned_bytes(
                            std::mem::size_of::<DecodedConcreteExpressionOrigin>() as u64,
                            &path,
                        )?;
                        DecodedConcreteExpressionOrigin::decode(decoder).map(Box::new)
                    })?,
                    role: decoder.field(4, HirExpressionTypeRoleV1::decode)?,
                    exact: decoder.field(5, DecodedPersistentId::decode)?,
                })
            }
            2 => {
                super::wire::require_fields(decoder, fields, 4)?;
                Ok(Self::CallableSignature {
                    root: decoder.field(1, DecodedCallableMaterialization::decode)?,
                    position: decoder.field(2, HirCallableTypePositionV1::decode)?,
                    exact: decoder.field(3, DecodedPersistentId::decode)?,
                })
            }
            3 => {
                super::wire::require_fields(decoder, fields, 3)?;
                Ok(Self::LocalValue {
                    local: decoder.field(1, DecodedPersistentId::decode)?,
                    exact: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            4 => {
                super::wire::require_fields(decoder, fields, 3)?;
                Ok(Self::BackingStorage {
                    property: decoder.field(1, DecodedPropertyOwner::decode)?,
                    exact: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            5 => {
                super::wire::require_fields(decoder, fields, 3)?;
                Ok(Self::DelegateStorage {
                    property: decoder.field(1, DecodedPropertyOwner::decode)?,
                    exact: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            6 => {
                super::wire::require_fields(decoder, fields, 3)?;
                Ok(Self::FieldStorage {
                    field: decoder.field(1, DecodedPersistentId::decode)?,
                    exact: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            7 => {
                super::wire::require_fields(decoder, fields, 3)?;
                Ok(Self::EnumVariantFieldStorage {
                    field: decoder.field(1, DecodedPersistentId::decode)?,
                    exact: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            8 => {
                super::wire::require_fields(decoder, fields, 3)?;
                Ok(Self::ConstructorInitializerResult {
                    constructor: decoder.field(1, DecodedCallableMaterialization::decode)?,
                    exact: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            9 => {
                super::wire::require_fields(decoder, fields, 3)?;
                Ok(Self::InitializationCycleMessage {
                    unit: decoder.field(1, DecodedPersistentId::decode)?,
                    exact: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(super::wire::unknown_tag(decoder, tag)),
        }
    }
}
