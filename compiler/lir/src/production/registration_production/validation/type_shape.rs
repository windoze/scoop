//! Instance geometry and descriptor scan validation.

use super::*;

pub(super) fn validate_type_descriptor_inline_scan(
    decoded: DecodedTypeDescriptorInlineScanV1,
    shape: &TypeInstanceShapeV1,
    exact_type: scoop_identity::PersistentExactTypeId,
    target: LirTargetProfile,
    foundation: &OdrFreeLirFoundation,
    index: usize,
) -> Result<TypeDescriptorInlineScanV1, StrongRegistrationProductionValidationError> {
    let contains_reference = shape.inline_scan().contains_reference();
    let DecodedTypeDescriptorInlineScanV1::Defined(decoded_scan) = decoded else {
        return if contains_reference {
            Err(semantic_error(
                RegistrationProductionTableV1::Type,
                index,
                "inline_scan_definition",
            ))
        } else {
            Ok(TypeDescriptorInlineScanV1::Null)
        };
    };
    if !contains_reference {
        return Err(semantic_error(
            RegistrationProductionTableV1::Type,
            index,
            "inline_scan_definition",
        ));
    }
    let scan_record = foundation
        .scans()
        .iter()
        .find(|record| record.id().as_array() == decoded_scan.as_array())
        .ok_or_else(|| {
            semantic_error(
                RegistrationProductionTableV1::Type,
                index,
                "inline_scan_definition",
            )
        })?;
    let scan = scan_record.id();
    let layout = foundation
        .layouts()
        .iter()
        .find(|record| record.id() == scan_record.key().layout())
        .ok_or_else(|| {
            semantic_error(
                RegistrationProductionTableV1::Type,
                index,
                "inline_scan_layout",
            )
        })?;
    let relation_matches = match shape.instance_kind() {
        TypeInstanceKindV1::BoxedValue => {
            scan_record.key().role() == ScanRole::InlineValue
                && layout.key().representation() == RepresentationRole::ManagedValue
                && layout.key().target_profile() == &target.wire_id()
        }
        TypeInstanceKindV1::InlineArray => {
            scan_record.key().role() == ScanRole::ArrayElement
                && layout.key().representation() == RepresentationRole::ManagedObject
                && layout.key().target_profile() == &target.wire_id()
                && layout.key().exact_type() == exact_type
        }
        TypeInstanceKindV1::FixedObject
        | TypeInstanceKindV1::InlineBytes
        | TypeInstanceKindV1::AbstractRef => false,
    };
    if !relation_matches {
        return Err(semantic_error(
            RegistrationProductionTableV1::Type,
            index,
            "inline_scan_relation",
        ));
    }
    Ok(TypeDescriptorInlineScanV1::Defined(scan))
}

pub(super) fn validate_type_instance_shape(
    decoded: DecodedTypeInstanceShapeV1,
    target: LirTargetProfile,
    index: usize,
) -> Result<TypeInstanceShapeV1, StrongRegistrationProductionValidationError> {
    let object_scan = validate_type_ref_scan(decoded.object_scan, index, "object_scan")?;
    let inline_scan = validate_type_ref_scan(decoded.inline_scan, index, "inline_scan")?;
    let shape = match decoded.instance_kind {
        1 => TypeInstanceShapeV1::fixed_object(
            target,
            decoded.minimum_size,
            decoded.instance_alignment,
            object_scan.clone(),
        ),
        2 => {
            let storage = match decoded.inline_storage_kind {
                1 => ValueStorageLayoutV1::inline(
                    decoded.inline_size,
                    decoded.inline_alignment,
                    inline_scan.clone(),
                ),
                2 => ValueStorageLayoutV1::zero_sized(decoded.inline_alignment),
                _ => {
                    return Err(semantic_error(
                        RegistrationProductionTableV1::Type,
                        index,
                        "inline_storage_kind",
                    ));
                }
            };
            storage.and_then(|storage| TypeInstanceShapeV1::boxed_value(target, storage))
        }
        3 => TypeInstanceShapeV1::inline_bytes(target),
        4 => {
            let storage = match decoded.inline_storage_kind {
                1 => ArrayElementStorageV1::inline(
                    decoded.inline_size,
                    decoded.inline_alignment,
                    inline_scan.clone(),
                ),
                2 => ArrayElementStorageV1::zero_sized(decoded.inline_alignment),
                _ => {
                    return Err(semantic_error(
                        RegistrationProductionTableV1::Type,
                        index,
                        "inline_storage_kind",
                    ));
                }
            };
            storage.and_then(|storage| TypeInstanceShapeV1::inline_array(target, storage))
        }
        5 => Ok(TypeInstanceShapeV1::abstract_ref()),
        _ => {
            return Err(semantic_error(
                RegistrationProductionTableV1::Type,
                index,
                "instance_kind",
            ));
        }
    }
    .map_err(|_| semantic_error(RegistrationProductionTableV1::Type, index, "instance_shape"))?;
    if shape.instance_kind().tag() != decoded.instance_kind
        || shape.inline_storage_kind().tag() != decoded.inline_storage_kind
        || shape.minimum_size() != decoded.minimum_size
        || shape.instance_alignment() != decoded.instance_alignment
        || shape.inline_offset() != decoded.inline_offset
        || shape.inline_size() != decoded.inline_size
        || shape.inline_stride() != decoded.inline_stride
        || shape.inline_alignment() != decoded.inline_alignment
        || shape.object_scan() != &object_scan
        || shape.inline_scan() != &inline_scan
    {
        return Err(semantic_error(
            RegistrationProductionTableV1::Type,
            index,
            "instance_shape",
        ));
    }
    Ok(shape)
}

fn validate_type_ref_scan(
    decoded: DecodedRefScan,
    index: usize,
    field: &'static str,
) -> Result<RefScan, StrongRegistrationProductionValidationError> {
    match decoded {
        DecodedRefScan::None => Ok(RefScan::None),
        DecodedRefScan::References(offsets) => Ok(RefScan::References(offsets)),
        DecodedRefScan::Sequence(parts) => parts
            .into_iter()
            .map(|part| validate_type_ref_scan(part, index, field))
            .collect::<Result<Vec<_>, _>>()
            .map(RefScan::Sequence),
        DecodedRefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let stride = NonZeroU64::new(stride)
                .ok_or_else(|| semantic_error(RegistrationProductionTableV1::Type, index, field))?;
            let element = validate_type_ref_scan(*element, index, field)?;
            let element = NonEmptyRefScan::new(element)
                .ok_or_else(|| semantic_error(RegistrationProductionTableV1::Type, index, field))?;
            Ok(RefScan::Array {
                length_offset,
                first_element_offset,
                stride,
                element: Box::new(element),
            })
        }
    }
}
