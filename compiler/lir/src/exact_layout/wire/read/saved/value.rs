use super::*;
use crate::exact_layout::model::ValueRepresentation;
use crate::{
    AggregateStorageLayoutV1, CLayoutStorageReplayV1, EnumStorageGeometryV1, EnumStorageRegionV1,
    EnumVariantSlotV1, EnumVariantStorageGeometryV1, NonZeroPow2, StorageGeometryV1,
    StoragePlacementV1, TupleStorageLayoutV1,
};

impl RawValue {
    pub(super) fn read(
        self,
        value: &ValueLayoutConstituentV1,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExactRepresentationLayoutV1, LinkDataError> {
        let extent = value.storage().byte_size();
        Ok(ExactRepresentationLayoutV1(match self {
            Self::Scalar(kind) => ValueRepresentation::Scalar(kind),
            Self::QualifiedPointer(kind) => ValueRepresentation::QualifiedPointer(kind),
            Self::Interface => ValueRepresentation::Interface,
            Self::Unit => ValueRepresentation::IntrinsicValue(IntrinsicValueFamilyV1::Unit),
            Self::MaybeUninit(id) => {
                ValueRepresentation::MaybeUninit(id.read(value.target(), identities)?)
            }
            Self::Struct {
                policy,
                interior_mutable,
                fields: raw,
            } => {
                let aggregate = AggregateStorageLayoutV1 {
                    storage: value.storage().clone(),
                    fields: fields(raw, identities, value.target(), extent)?,
                };
                let policy = match policy {
                    RawPolicy::Ordinary => StructLayoutPolicyV1::Ordinary(aggregate),
                    RawPolicy::CLayout { contract, .. } => {
                        let contract = identities.resolve(contract).map_err(link_error)?;
                        let contract = foundation
                            .c_abi_layouts()
                            .iter()
                            .find(|entry| entry.fingerprint() == contract)
                            .ok_or_else(|| link_error("missing C layout contract"))?
                            .clone();
                        StructLayoutPolicyV1::CLayout(CLayoutStorageReplayV1 {
                            target: value.target(),
                            contract,
                            aggregate,
                        })
                    }
                };
                ValueRepresentation::Struct(StructRepresentationLayoutV1 {
                    policy,
                    interior_mutable,
                })
            }
            Self::Tuple(raw) => ValueRepresentation::Tuple(TupleStorageLayoutV1 {
                exact: value.exact(),
                storage: value.storage().clone(),
                elements: raw
                    .into_iter()
                    .enumerate()
                    .map(|(i, field)| {
                        field.read(value.exact(), i, identities, value.target(), extent)
                    })
                    .collect::<Result<_, _>>()?,
            }),
            Self::NicheEnum {
                pointer,
                variants,
                payload,
            } => ValueRepresentation::NicheEnum(NicheEnumRepresentationLayoutV1 {
                pointer_kind: pointer,
                variants: variants
                    .into_iter()
                    .map(|raw| raw.read(identities, value.target(), extent))
                    .collect::<Result<_, _>>()?,
                payload_variant: identities.resolve(payload).map_err(link_error)?,
            }),
            Self::TaggedEnum {
                tag,
                pure,
                variants,
            } => {
                let tag_layout = tag.read(extent)?;
                let pure_region = pure.read(extent)?;
                let mut geometry = Vec::with_capacity(variants.len());
                let mut read = Vec::with_capacity(variants.len());
                for raw in variants {
                    let (slot, size, alignment) = match raw.slot {
                        RawSlot::Shared { size, alignment } => {
                            (EnumVariantSlotV1::SharedPure(pure_region), size, alignment)
                        }
                        RawSlot::Dedicated(region) => {
                            let region = region.read(extent)?;
                            (
                                EnumVariantSlotV1::Dedicated(region),
                                region.byte_size(),
                                region.alignment().get(),
                            )
                        }
                    };
                    let variant = raw.variant.read(identities, value.target(), extent)?;
                    geometry.push(EnumVariantStorageGeometryV1 {
                        storage: StorageGeometryV1::new(value.target(), size, alignment)
                            .map_err(link_error)?,
                        fields: variant
                            .fields()
                            .iter()
                            .map(|field| StoragePlacementV1 {
                                offset: field.storage().offset().get(),
                                access_alignment: field.access_alignment(),
                            })
                            .collect(),
                        slot,
                    });
                    read.push(variant);
                }
                ValueRepresentation::TaggedEnum(TaggedEnumRepresentationLayoutV1 {
                    geometry: EnumStorageGeometryV1 {
                        storage: StorageGeometryV1::new(
                            value.target(),
                            extent,
                            value.storage().alignment().get(),
                        )
                        .map_err(link_error)?,
                        tag_layout,
                        pure_region,
                        variants: geometry,
                    },
                    variants: read,
                })
            }
        }))
    }
}

impl RawVariant {
    fn read(
        self,
        identities: &mut ValidatedIdentityGraph,
        target: LirTargetProfile,
        extent: u64,
    ) -> Result<EnumVariantLayoutV1, LinkDataError> {
        Ok(EnumVariantLayoutV1 {
            variant: identities.resolve(self.id).map_err(link_error)?,
            fields: self
                .fields
                .into_iter()
                .map(|field| {
                    Ok(EnumVariantFieldLayoutV1 {
                        field: identities.resolve(field.id).map_err(link_error)?,
                        storage: field.storage.read(identities, target, extent)?,
                        access_alignment: NonZeroPow2::new(field.alignment).map_err(link_error)?,
                    })
                })
                .collect::<Result<_, LinkDataError>>()?,
        })
    }
}

impl RawRegion {
    fn read(self, extent: u64) -> Result<EnumStorageRegionV1, LinkDataError> {
        if self
            .offset
            .checked_add(self.size)
            .is_none_or(|end| end > extent)
        {
            return Err(link_error("enum region exceeds its storage"));
        }
        Ok(EnumStorageRegionV1 {
            offset: self.offset,
            byte_size: self.size,
            alignment: NonZeroPow2::new(self.alignment).map_err(link_error)?,
        })
    }
}
