use scoop_identity::{
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey,
    GeneratedEnumVariantRole, GeneratedNominalKey, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExactTypeId,
};

use super::*;
use crate::{
    ExactRepresentationKindV1, FieldStorageKindV1, InstanceRepresentationKindV1, NichePointerKind,
};

pub(super) fn validate_boxed(
    payload: PersistentExactTypeId,
    boxed: PersistentExactTypeId,
    descriptors: &CanonicalExactDescriptorExportsV1,
) -> Result<(), ParamFreeShapeSupportExportError> {
    let source = descriptors
        .get(payload)
        .ok_or(ParamFreeShapeSupportExportError::MissingDescriptor(payload))?;
    let descriptor = descriptors
        .get(boxed)
        .ok_or(ParamFreeShapeSupportExportError::MissingDescriptor(boxed))?;
    if !matches!(
        descriptor.value_layout().representation().kind(),
        ExactRepresentationKindV1::QualifiedPointer(NichePointerKind::Managed)
    ) {
        return Err(ParamFreeShapeSupportExportError::BoxedRepresentation(boxed));
    }
    let InstanceRepresentationKindV1::BoxedPayload(actual) =
        descriptor.instance_layout().representation().kind()
    else {
        return Err(ParamFreeShapeSupportExportError::BoxedRepresentation(boxed));
    };
    (actual == source.value_layout().value())
        .then_some(())
        .ok_or(ParamFreeShapeSupportExportError::BoxedRepresentation(boxed))
}

pub(super) fn validate_helper(
    owner: &GeneratedNominalKey,
    roles: [GeneratedEnumVariantRole; 2],
    payload_index: usize,
    payload: PersistentExactTypeId,
    helper: PersistentExactTypeId,
    descriptors: &CanonicalExactDescriptorExportsV1,
) -> Result<(), ParamFreeShapeSupportExportError> {
    let descriptor = descriptors
        .get(helper)
        .ok_or(ParamFreeShapeSupportExportError::MissingDescriptor(helper))?;
    let variants = match descriptor.value_layout().representation().kind() {
        ExactRepresentationKindV1::TaggedEnum(value) => value.variants(),
        ExactRepresentationKindV1::NicheEnum(value) => {
            let expected = variant(owner, roles[payload_index])?;
            if value.payload_variant() != expected {
                return Err(ParamFreeShapeSupportExportError::HelperPayload(helper));
            }
            value.variants()
        }
        _ => {
            return Err(ParamFreeShapeSupportExportError::HelperRepresentation(
                helper,
            ));
        }
    };
    if variants.len() != 2 {
        return Err(ParamFreeShapeSupportExportError::HelperVariants(helper));
    }
    for (index, (actual, role)) in variants.iter().zip(roles).enumerate() {
        let expected = variant(owner, role)?;
        if actual.variant() != expected {
            return Err(ParamFreeShapeSupportExportError::HelperVariants(helper));
        }
        if index == payload_index {
            validate_payload(actual, expected, payload, helper, descriptors)?;
        } else if !actual.fields().is_empty() {
            return Err(ParamFreeShapeSupportExportError::HelperVariants(helper));
        }
    }
    Ok(())
}

fn validate_payload(
    actual: &crate::EnumVariantLayoutV1,
    variant: PersistentEnumVariantId,
    payload: PersistentExactTypeId,
    helper: PersistentExactTypeId,
    descriptors: &CanonicalExactDescriptorExportsV1,
) -> Result<(), ParamFreeShapeSupportExportError> {
    let field = PersistentEnumVariantFieldId::from_key(&EnumVariantFieldKey::new(
        variant,
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    ))?;
    let [actual] = actual.fields() else {
        return Err(ParamFreeShapeSupportExportError::HelperPayload(helper));
    };
    if actual.field() != field || !payload_storage(actual.storage().kind(), payload, descriptors)? {
        return Err(ParamFreeShapeSupportExportError::HelperPayload(helper));
    }
    Ok(())
}

fn payload_storage(
    actual: FieldStorageKindV1<'_>,
    payload: PersistentExactTypeId,
    descriptors: &CanonicalExactDescriptorExportsV1,
) -> Result<bool, ParamFreeShapeSupportExportError> {
    let expected = descriptors
        .get(payload)
        .ok_or(ParamFreeShapeSupportExportError::MissingDescriptor(payload))?
        .value_layout()
        .value();
    Ok(match (actual, expected.nonzero_ref()) {
        (
            FieldStorageKindV1::ElidedZst {
                exact, alignment, ..
            },
            None,
        ) => exact == payload && alignment == expected.storage().alignment(),
        (FieldStorageKindV1::Stored { exact, layout, .. }, Some(expected)) => {
            exact == payload && layout == &expected
        }
        _ => false,
    })
}

fn variant(
    owner: &GeneratedNominalKey,
    role: GeneratedEnumVariantRole,
) -> Result<PersistentEnumVariantId, ParamFreeShapeSupportExportError> {
    Ok(PersistentEnumVariantId::from_key(
        &EnumVariantIdentityKey::generated(owner, role)?,
    )?)
}
