use super::*;
use crate::{
    CheckedNominalSupportPropertySourceV1, NominalSupportCallableInterfaceV1,
    NominalSupportPropertyPayloadV1, ProtectedPropertyMutabilityV1,
};
use scoop_identity::CallableTemplateOrigin;

pub(super) fn validate<A: NestedNominalSemanticAuthority<E>, E>(
    record: &NominalSupportNestedInterfaceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    representations: &CanonicalNominalRepresentationSupportV1,
    authority: &mut A,
) -> Result<(), NestedSourceSemanticError<E>> {
    use NestedSourceSemanticError as Error;
    let records = record
        .payload()
        .source_interface()
        .source_support()
        .records();
    // Validate each source record before the second pass composes accessor
    // contracts. No DTO supplied by an unchecked caller becomes a proof.
    for entry in records {
        match entry {
            NestedSourceSupportV1::Callable(value) => {
                value
                    .validate_source(graph, authority)
                    .map_err(Error::Callable)?;
            }
            NestedSourceSupportV1::Constructor(value) => {
                value
                    .validate_source(graph, authority)
                    .map_err(Error::Callable)?;
            }
            NestedSourceSupportV1::Property(value) => {
                value
                    .validate_source(graph, authority)
                    .map_err(Error::Property)?;
            }
            NestedSourceSupportV1::NestedNominal(value) => {
                super::validate(value, graph, representations, authority)?
            }
        }
    }
    for entry in records {
        if let NestedSourceSupportV1::Property(value) = entry {
            let NominalSupportPropertyPayloadV1::Runtime { interface } = value.payload() else {
                continue;
            };
            let getter = accessor(records, interface.getter())?;
            let setter = match interface.mutability() {
                ProtectedPropertyMutabilityV1::ReadOnly => None,
                ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } => {
                    Some(accessor(records, *setter)?)
                }
            };
            let CheckedNominalSupportPropertySourceV1::Runtime(checked) = value
                .validate_source(graph, authority)
                .map_err(Error::Property)?
            else {
                return Err(Error::ReferenceClosure);
            };
            checked
                .validate_resolved_accessor_records(getter, setter)
                .map_err(Error::Accessor)?;
        }
    }
    Ok(())
}
fn accessor<E>(
    records: &[NestedSourceSupportV1],
    id: scoop_identity::PersistentPropertyAccessorId,
) -> Result<&NominalSupportCallableInterfaceV1, NestedSourceSemanticError<E>> {
    records
        .iter()
        .find_map(|record| match record {
            NestedSourceSupportV1::Callable(value)
                if value.declaration() == CallableTemplateOrigin::Accessor(id) =>
            {
                Some(value.as_ref())
            }
            _ => None,
        })
        .ok_or(NestedSourceSemanticError::ReferenceClosure)
}
