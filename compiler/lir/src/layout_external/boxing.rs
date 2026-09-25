use super::*;
use crate::{BoxedValueDescriptor, boxing::BoxedDescriptorReference};

pub(super) fn materialize<'a>(
    selected: &'a impl MaterializationSelection<'a>,
    provider: ConeIdentity,
    source: scoop_identity::PersistentTypeId,
    external: &la_arena::Arena<ExternalTypeDescriptor>,
    descriptor: crate::ExternalTypeDescriptorId,
    storage_type: crate::LirType,
    meter: &mut BudgetMeter,
) -> Result<BoxedValueDescriptor, LayoutExternalMaterializationError> {
    use LayoutExternalMaterializationError as Error;
    validate_provider(selected, provider)?;
    meter.charge_work(selected.semantic_count() as u64, &WirePath::root())?;
    let Some(LayoutAbiSemanticRecordV1::ShapeSupport(shape)) =
        selected.semantic_record(provider, LayoutAbiSemanticTargetV1::ShapeSupport(source))
    else {
        return Err(Error::MissingShapeSupport { provider, source });
    };
    let boxed = shape
        .roles()
        .boxed_value()
        .available()
        .ok_or(Error::UnavailableBoxedValue(source))?;
    let definition = materialize_type_descriptor(selected, provider, boxed.exact(), meter)?;
    if descriptor.into_raw().into_u32() as usize >= external.len()
        || external[descriptor] != definition
    {
        return Err(Error::BoxDescriptor(
            crate::BoxDescriptorError::InvalidDescriptor,
        ));
    }
    meter.charge_work(selected.semantic_count() as u64, &WirePath::root())?;
    let Some(LayoutAbiSemanticRecordV1::Descriptor(record)) = selected.semantic_record(
        provider,
        LayoutAbiSemanticTargetV1::Descriptor(boxed.exact()),
    ) else {
        return Err(Error::MissingDescriptor {
            provider,
            exact: boxed.exact(),
        });
    };
    crate::exact_descriptor::resources::scan(record.shape().inline_scan(), meter, 1)?;
    BoxedValueDescriptor::from_shape(
        BoxedDescriptorReference::External {
            id: descriptor,
            definition,
        },
        boxed.exact(),
        record.shape(),
        shape.exact(),
        storage_type,
    )
    .map_err(Error::BoxDescriptor)
}
