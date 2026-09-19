use super::*;
use scoop_identity::BindableEntity;

mod members;
mod shape;
use members::{public_entry, subsets};

pub(super) fn validate<E>(
    candidate: &CrossConeTypeSemanticsSectionV1,
    public: &CrossConeHirInterfaceSectionV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    sequence(
        candidate.protected_declarations().records().len(),
        meter,
        path,
    )?;
    let mut pending = Vec::new();
    for declaration in candidate.protected_declarations().records() {
        match declaration {
            ProtectedDeclarationInterfaceV1::Callable(record) => callables::validate(
                record.declaration(),
                record.declaration_access(),
                record.payload(),
                public,
                graph,
                meter,
                path,
            )?,
            ProtectedDeclarationInterfaceV1::Constructor(record) => callables::validate(
                CallableTemplateOrigin::Constructor(record.declaration()),
                record.declaration_access(),
                record.payload(),
                public,
                graph,
                meter,
                path,
            )?,
            ProtectedDeclarationInterfaceV1::Property(record) => properties::runtime(
                record.declaration(),
                record.declaration_access(),
                record.payload(),
                public,
                graph,
                meter,
                path,
            )?,
            ProtectedDeclarationInterfaceV1::NestedNominal(record) => push(
                &mut pending,
                record.declaration(),
                record.declaration_access(),
                record.payload(),
                1,
                meter,
                path,
            )?,
        }
    }
    while let Some((declaration, access, payload, depth)) = pending.pop() {
        let source = payload.source_interface();
        lookup(public.nominal_interfaces().records().len(), meter, path)?;
        let old = public.nominal_interfaces().get(declaration);
        if effective_public(access, graph, meter, path)? {
            let old = old.ok_or(TypeSectionExportValidationError::PublicOverlap)?;
            require(old.kind() == source.kind())?;
            require(binders(
                old.type_parameters(),
                source.type_parameters(),
                meter,
                path,
            )?)?;
            require(signatures(
                old.exact_supertypes().values(),
                source.supertypes().values(),
                meter,
                path,
            )?)?;
            require(shape::matches(
                source.source_shape(),
                old.source_shape(),
                meter,
                path,
            )?)?;
            subsets(source, old, public, meter, path)?;
        } else {
            require(old.is_none())?;
        }
        sequence(source.source_support().records().len(), meter, path)?;
        for record in source.source_support().records() {
            if let Some(old) = old
                && effective_public(record.declaration_access(), graph, meter, path)?
            {
                public_entry(record, source, old, public, meter, path)?;
            }
            match record {
                NestedSourceSupportV1::Callable(record) => callables::validate(
                    record.declaration(),
                    record.declaration_access(),
                    record.payload(),
                    public,
                    graph,
                    meter,
                    path,
                )?,
                NestedSourceSupportV1::Constructor(record) => callables::validate(
                    CallableTemplateOrigin::Constructor(record.declaration()),
                    record.declaration_access(),
                    record.payload(),
                    public,
                    graph,
                    meter,
                    path,
                )?,
                NestedSourceSupportV1::Property(record) => {
                    properties::validate(record, public, graph, meter, path)?
                }
                NestedSourceSupportV1::NestedNominal(record) => push(
                    &mut pending,
                    record.declaration(),
                    record.declaration_access(),
                    record.payload(),
                    depth.checked_add(1).ok_or_else(|| overflow(path))?,
                    meter,
                    path,
                )?,
            }
        }
    }
    Ok(())
}
type Entry<'a> = (
    SourceNominalId,
    &'a DeclarationAccessSourceV1,
    &'a ProtectedNestedNominalPayloadV1,
    u64,
);
fn push<'a>(
    pending: &mut Vec<Entry<'a>>,
    declaration: SourceNominalId,
    access: &'a DeclarationAccessSourceV1,
    payload: &'a ProtectedNestedNominalPayloadV1,
    depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_semantic_depth(depth, path)?;
    meter.charge_nodes(1, path)?;
    meter.charge_work(1, path)?;
    meter.try_reserve_collection_slots(pending, 1, path)?;
    pending.push((declaration, access, payload, depth));
    Ok(())
}
