use super::resolve::{constituent, exact_target, target};
use super::*;
use scoop_identity::{LayoutKey, RepresentationRole};

pub(super) fn enqueue(
    record: &crate::ExactLayoutExportV1,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let profile = record.identity().target();
    match record.kind() {
        crate::ExactLayoutBodyKindV1::Value(value) => match value.representation().kind() {
            crate::ExactRepresentationKindV1::Struct(value) => {
                fields(value.fields(), profile, depth, views, index, pending, meter)?
            }
            crate::ExactRepresentationKindV1::Tuple(value) => {
                for element in value.elements() {
                    field(
                        element.storage(),
                        profile,
                        depth,
                        views,
                        index,
                        pending,
                        meter,
                    )?;
                }
            }
            crate::ExactRepresentationKindV1::TaggedEnum(value) => variants(
                value.variants(),
                profile,
                depth,
                views,
                index,
                pending,
                meter,
            )?,
            crate::ExactRepresentationKindV1::NicheEnum(value) => variants(
                value.variants(),
                profile,
                depth,
                views,
                index,
                pending,
                meter,
            )?,
            crate::ExactRepresentationKindV1::Scalar(_)
            | crate::ExactRepresentationKindV1::QualifiedPointer(_)
            | crate::ExactRepresentationKindV1::IntrinsicValue(_) => {}
        },
        crate::ExactLayoutBodyKindV1::Instance(instance) => {
            match instance.representation().kind() {
                crate::InstanceRepresentationKindV1::ClassObject(value) => {
                    if let crate::ClassBasePrefixV1::BasePrefix {
                        exact,
                        layout,
                        byte_size,
                        alignment,
                    } = value.base_prefix()
                    {
                        let owner = target(
                            LayoutAbiSemanticTargetV1::Layout(layout),
                            None,
                            depth,
                            views,
                            index,
                            pending,
                            meter,
                        )?;
                        let Some(candidate) = views[owner].layouts().get(layout) else {
                            return Err(LayoutAbiSemanticClosureError::MissingTarget(
                                LayoutAbiSemanticTargetV1::Layout(layout),
                            ));
                        };
                        let matches = candidate.instance_handle().is_some_and(|candidate| {
                            candidate.identity().exact() == exact
                                && candidate.shape().minimum_size() == byte_size
                                && candidate.shape().instance_alignment() == alignment.get()
                        });
                        if !matches {
                            return Err(LayoutAbiSemanticClosureError::EmbeddedRecord(
                                LayoutAbiSemanticTargetV1::Layout(layout),
                            ));
                        }
                    }
                    fields(
                        value.declared_fields(),
                        profile,
                        depth,
                        views,
                        index,
                        pending,
                        meter,
                    )?;
                }
                crate::InstanceRepresentationKindV1::BoxedPayload(value) => {
                    constituent(value, depth, views, index, pending, meter)?;
                }
                crate::InstanceRepresentationKindV1::InlineArray { element, .. } => {
                    constituent(element, depth, views, index, pending, meter)?;
                }
                crate::InstanceRepresentationKindV1::InlineBytes
                | crate::InstanceRepresentationKindV1::AbstractReference => {}
            }
        }
    }
    Ok(())
}

fn variants(
    variants: &[crate::EnumVariantLayoutV1],
    profile: crate::LirTargetProfile,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    for variant in variants {
        for item in variant.fields() {
            field(item.storage(), profile, depth, views, index, pending, meter)?;
        }
    }
    Ok(())
}

fn fields(
    fields: &[crate::PlacedFieldStorageV1],
    profile: crate::LirTargetProfile,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    for item in fields {
        field(item.storage(), profile, depth, views, index, pending, meter)?;
    }
    Ok(())
}

fn field(
    field: &crate::FieldStorageV1,
    profile: crate::LirTargetProfile,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let (layout, expected, zst_alignment) = match field.kind() {
        crate::FieldStorageKindV1::Stored { layout, .. } => (layout.layout(), Some(layout), None),
        crate::FieldStorageKindV1::ElidedZst {
            exact, alignment, ..
        } => (
            PersistentLayoutId::from_key(&LayoutKey::new(
                exact,
                profile.wire_id(),
                RepresentationRole::ManagedValue,
            ))?,
            None,
            Some(alignment),
        ),
    };
    let owner = exact_target(layout, None, depth, views, index, pending, meter)?;
    let Some(candidate) = views[owner].layouts().get(layout) else {
        return Err(LayoutAbiSemanticClosureError::MissingTarget(
            LayoutAbiSemanticTargetV1::Layout(layout),
        ));
    };
    let Some(candidate) = candidate.value_handle() else {
        return Err(LayoutAbiSemanticClosureError::EmbeddedRecord(
            LayoutAbiSemanticTargetV1::Layout(layout),
        ));
    };
    let matches = match expected {
        Some(expected) => candidate.value().nonzero_ref().as_ref() == Some(expected),
        None => zst_alignment.is_some_and(|alignment| {
            candidate.value().nonzero_ref().is_none()
                && candidate.value().storage().alignment() == alignment
        }),
    };
    if matches {
        Ok(())
    } else {
        Err(LayoutAbiSemanticClosureError::EmbeddedRecord(
            LayoutAbiSemanticTargetV1::Layout(layout),
        ))
    }
}
