use super::*;
use scoop_identity::{LocalValueSelector, StructuralPathSegment};

pub(super) fn validate(
    template: &DefaultSourceTemplateV1,
    provider: DefaultTemplateProviderShapeV1,
    shapes: &mut sources::Shapes<'_, '_, '_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    let definition_len = template.definition_path().segments().len() as u64;
    meter.charge_work(definition_len, path)?;
    for local in template.locals().records() {
        let length = match local.selector() {
            LocalValueSelector::This | LocalValueSelector::Parameter { .. } => 0,
            LocalValueSelector::LocalDeclaration { path }
            | LocalValueSelector::BoundReceiver { path }
            | LocalValueSelector::Synthetic { path, .. }
            | LocalValueSelector::SuspensionResult { site: path } => path.segments().len() as u64,
        };
        meter.charge_work(
            length.saturating_add(definition_len).saturating_add(1),
            path,
        )?;
        // Reserve the existing scope diagnostic's owned selector before checking.
        meter.charge_nodes(length.saturating_add(1), path)?;
        meter.charge_collection_slots(length, path)?;
        let bytes = length.saturating_mul(std::mem::size_of::<StructuralPathSegment>() as u64);
        meter.charge_owned_bytes(bytes, path)?;
    }
    template
        .locals()
        .validate_definition_path(template.definition_path())
        .map_err(Error::LocalScope)?;
    let scope = provider.signature_scope();
    for (index, local) in template.locals().records().iter().enumerate() {
        scope
            .validate_signature_semantics_metered(local.value_type(), shapes, meter, path)
            .map_err(|error| Error::LocalType {
                index,
                error: Box::new(error),
            })?;
    }
    // Origins were bound atomically before any declaration contract was built.
    // The type walk uses the same body visitor as complete envelope validation.
    template
        .body()
        .validate_provider_types_semantics(provider, shapes, meter, path)
        .map_err(|error| Error::BodyEnvelope(Box::new(error)))
}
