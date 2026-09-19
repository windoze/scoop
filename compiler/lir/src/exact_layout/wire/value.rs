use super::*;

impl WireEncode for ExactRepresentationLayoutV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        match self.kind() {
            ExactRepresentationKindV1::Scalar(kind) => {
                sum(encoder, 1, 1)?;
                encoder.field(1)?;
                scalar(encoder, kind)
            }
            ExactRepresentationKindV1::QualifiedPointer(kind) => {
                sum(encoder, 2, 1)?;
                encoder.field(1)?;
                pointer(encoder, kind)
            }
            ExactRepresentationKindV1::Struct(value) => {
                sum(encoder, 3, 3)?;
                encoder.field(1)?;
                policy(encoder, value.policy())?;
                unsigned(encoder, 2, u64::from(value.interior_mutable()))?;
                encoder.field(3)?;
                nominal_fields(encoder, value.fields())
            }
            ExactRepresentationKindV1::Tuple(value) => {
                sum(encoder, 4, 1)?;
                encoder.field(1)?;
                array(encoder, value.elements())
            }
            ExactRepresentationKindV1::TaggedEnum(value) => {
                sum(encoder, 5, 3)?;
                encoder.field(1)?;
                region(encoder, value.geometry().tag_layout())?;
                encoder.field(2)?;
                region(encoder, value.geometry().pure_region())?;
                encoder.field(3)?;
                encoder.array(value.variants().len() as u64)?;
                for (variant, geometry) in value.variants().iter().zip(value.geometry().variants())
                {
                    encoder.map(3)?;
                    variant_fields(encoder, variant)?;
                    encoder.field(3)?;
                    match geometry.slot() {
                        crate::EnumVariantSlotV1::SharedPure(_) => {
                            sum(encoder, 1, 2)?;
                            unsigned(encoder, 1, geometry.storage().size())?;
                            unsigned(encoder, 2, geometry.storage().alignment().get())?;
                        }
                        crate::EnumVariantSlotV1::Dedicated(slot) => {
                            sum(encoder, 2, 3)?;
                            unsigned(encoder, 1, slot.offset())?;
                            unsigned(encoder, 2, slot.byte_size())?;
                            unsigned(encoder, 3, slot.alignment().get())?;
                        }
                    }
                }
                Ok(())
            }
            ExactRepresentationKindV1::NicheEnum(value) => {
                sum(encoder, 6, 3)?;
                encoder.field(1)?;
                pointer(encoder, value.pointer_kind())?;
                encoder.field(2)?;
                encoder.array(value.variants().len() as u64)?;
                for variant in value.variants() {
                    encoder.map(2)?;
                    variant_fields(encoder, variant)?;
                }
                field(encoder, 3, &value.payload_variant())
            }
            ExactRepresentationKindV1::IntrinsicValue(IntrinsicValueFamilyV1::Unit) => {
                sum(encoder, 7, 1)?;
                encoder.field(1)?;
                sum(encoder, 1, 0)
            }
        }
    }
}

pub(super) fn scalar(encoder: &mut Encoder, kind: ScalarRepresentationKindV1) -> EncodeResult {
    match kind {
        ScalarRepresentationKindV1::Integer(kind) => {
            sum(encoder, 1, 2)?;
            encoder.field(1)?;
            sum(
                encoder,
                match kind.signedness() {
                    crate::IntegerSignedness::Signed => 1,
                    crate::IntegerSignedness::Unsigned => 2,
                },
                0,
            )?;
            encoder.field(2)?;
            sum(
                encoder,
                match kind.width() {
                    crate::IntegerWidth::W8 => 1,
                    crate::IntegerWidth::W16 => 2,
                    crate::IntegerWidth::W32 => 3,
                    crate::IntegerWidth::W64 => 4,
                },
                0,
            )
        }
        ScalarRepresentationKindV1::Boolean => sum(encoder, 2, 0),
    }
}

fn policy(encoder: &mut Encoder, policy: &StructLayoutPolicyV1) -> EncodeResult {
    match policy {
        StructLayoutPolicyV1::Ordinary(_) => sum(encoder, 1, 0),
        StructLayoutPolicyV1::CLayout(layout) => {
            sum(encoder, 2, 3)?;
            encoder.field(1)?;
            alignment(encoder, layout.contract().layout().aligned())?;
            encoder.field(2)?;
            alignment(encoder, layout.contract().layout().packed())?;
            field(encoder, 3, &layout.contract().fingerprint())
        }
    }
}

pub(super) fn alignment(
    encoder: &mut Encoder,
    value: scoop_identity::CLayoutOverride,
) -> EncodeResult {
    use scoop_identity::{CLayoutByteAlignment as A, CLayoutOverride as O};
    sum(
        encoder,
        match value {
            O::Natural => 1,
            O::Bytes(value) => match value {
                A::Bytes1 => 2,
                A::Bytes2 => 3,
                A::Bytes4 => 4,
                A::Bytes8 => 5,
                A::Bytes16 => 6,
            },
        },
        0,
    )
}

fn region(encoder: &mut Encoder, region: crate::EnumStorageRegionV1) -> EncodeResult {
    encoder.map(3)?;
    unsigned(encoder, 1, region.offset())?;
    unsigned(encoder, 2, region.byte_size())?;
    unsigned(encoder, 3, region.alignment().get())
}

fn variant_fields(encoder: &mut Encoder, variant: &EnumVariantLayoutV1) -> EncodeResult {
    field(encoder, 1, &variant.variant())?;
    encoder.field(2)?;
    encoder.array(variant.fields().len() as u64)?;
    for item in variant.fields() {
        encoder.map(3)?;
        field(encoder, 1, &item.field())?;
        field(encoder, 2, item.storage())?;
        unsigned(encoder, 3, item.access_alignment().get())?;
    }
    Ok(())
}
