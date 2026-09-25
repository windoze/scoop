use super::*;
use scoop_identity::BindableEntity;

mod members;
mod shape;
use members::{public_entry, subsets};

pub(super) fn validate<E>(
    candidate: &CrossConeTypeSemanticsSectionV1,
    public: &CrossConeHirInterfaceSectionV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,

    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    let mut pending = Vec::new();
    for declaration in candidate.protected_declarations().records() {
        match declaration {
            ProtectedDeclarationInterfaceV1::Callable(record) => callables::validate(
                record.declaration(),
                record.declaration_access(),
                record.payload(),
                public,
                graph,
                path,
            )?,
            ProtectedDeclarationInterfaceV1::Constructor(record) => callables::validate(
                CallableTemplateOrigin::Constructor(record.declaration()),
                record.declaration_access(),
                record.payload(),
                public,
                graph,
                path,
            )?,
            ProtectedDeclarationInterfaceV1::Property(record) => properties::runtime(
                record.declaration(),
                record.declaration_access(),
                record.payload(),
                public,
                graph,
                path,
            )?,
            ProtectedDeclarationInterfaceV1::NestedNominal(record) => push(
                &mut pending,
                record.declaration(),
                record.declaration_access(),
                record.payload(),
                path,
            )?,
        }
    }
    while let Some((declaration, access, payload)) = pending.pop() {
        let source = payload.source_interface();

        let old = public.nominal_interfaces().get(declaration);
        if effective_public(access, graph)? {
            let old = old.ok_or(TypeSectionExportValidationError::PublicOverlap)?;
            require(old.kind() == source.kind())?;
            require(binders(
                old.type_parameters(),
                source.type_parameters(),
                path,
            )?)?;
            require(signatures(
                old.exact_supertypes().values(),
                source.supertypes().values(),
                path,
            )?)?;
            require(shape::matches(source.source_shape(), old.source_shape()))?;
            subsets(source, old, public)?;
        } else {
            require(old.is_none())?;
        }

        for record in source.source_support().records() {
            if let Some(old) = old
                && effective_public(record.declaration_access(), graph)?
            {
                public_entry(record, source, old, public)?;
            }
            match record {
                NestedSourceSupportV1::Callable(record) => callables::validate(
                    record.declaration(),
                    record.declaration_access(),
                    record.payload(),
                    public,
                    graph,
                    path,
                )?,
                NestedSourceSupportV1::Constructor(record) => callables::validate(
                    CallableTemplateOrigin::Constructor(record.declaration()),
                    record.declaration_access(),
                    record.payload(),
                    public,
                    graph,
                    path,
                )?,
                NestedSourceSupportV1::Property(record) => {
                    properties::validate(record, public, graph, path)?
                }
                NestedSourceSupportV1::NestedNominal(record) => push(
                    &mut pending,
                    record.declaration(),
                    record.declaration_access(),
                    record.payload(),
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
);
fn push<'a>(
    pending: &mut Vec<Entry<'a>>,
    declaration: SourceNominalId,
    access: &'a DeclarationAccessSourceV1,
    payload: &'a ProtectedNestedNominalPayloadV1,
    path: &WirePath,
) -> Result<(), WireError> {
    scoop_wire::allocation::try_reserve(pending, 1, path)?;
    pending.push((declaration, access, payload));
    Ok(())
}
