use super::*;

pub(crate) fn validate_inline_scan(
    semantic: &StrongTypeDescriptorSemanticPlanV2,
    instance_record: &crate::ExactLayoutExportV1,
    instance: &crate::ExactInstanceLayoutV1,
    layouts: &crate::CanonicalExactLayoutExportsV1,
) -> Result<(), ExactDescriptorError> {
    let expected = expected_inline_scan(instance_record, instance, layouts)?;
    let actual = match semantic.inline_scan() {
        TypeDescriptorInlineScanV1::Null => None,
        TypeDescriptorInlineScanV1::Defined(scan) => Some(scan),
    };
    if expected == actual {
        Ok(())
    } else {
        Err(ExactDescriptorError::InlineScan(semantic.exact_type()))
    }
}

pub(crate) fn expected_inline_scan(
    instance_record: &crate::ExactLayoutExportV1,
    instance: &crate::ExactInstanceLayoutV1,
    layouts: &crate::CanonicalExactLayoutExportsV1,
) -> Result<Option<scoop_identity::PersistentScanId>, ExactDescriptorError> {
    Ok(match instance.representation().kind() {
        InstanceRepresentationKindV1::BoxedPayload(value) => {
            inline_scan(value.layout(), instance.shape(), layouts)?
        }
        InstanceRepresentationKindV1::InlineArray { .. } => instance
            .shape()
            .inline_scan()
            .contains_reference()
            .then_some(instance_record.scan()),
        InstanceRepresentationKindV1::ClassObject(_)
        | InstanceRepresentationKindV1::InlineBytes
        | InstanceRepresentationKindV1::AbstractReference => None,
    })
}

fn inline_scan(
    layout: scoop_identity::PersistentLayoutId,
    shape: &crate::TypeInstanceShapeV1,
    layouts: &crate::CanonicalExactLayoutExportsV1,
) -> Result<Option<scoop_identity::PersistentScanId>, ExactDescriptorError> {
    if !shape.inline_scan().contains_reference() {
        return Ok(None);
    }
    layouts
        .get(layout)
        .map(|record| Some(record.scan()))
        .ok_or(ExactDescriptorError::MissingInlineLayout(layout))
}
