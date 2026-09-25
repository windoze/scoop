use super::*;

pub(in super::super) fn validate(
    bound: &mut BoundNominalMemberSourcesV1<'_, '_, '_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
) -> Result<(), Error> {
    let properties = bound.properties;
    let callables = bound.callables;

    for record in properties.records() {
        let checked = record
            .validate_source(graph, bound)
            .map_err(Error::from_property)?;
        let proof = match checked {
            CheckedNominalSupportPropertySourceV1::Const(_) => NominalMemberPropertyProofV1::Const,
            CheckedNominalSupportPropertySourceV1::Runtime(checked) => {
                let payload = checked.payload();
                let getter = lookup(callables, payload.getter())?;
                let setter = match payload.mutability() {
                    ProtectedPropertyMutabilityV1::ReadOnly => None,
                    ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } => {
                        Some(lookup(callables, *setter)?)
                    }
                };
                // Every callable above has already passed source semantics.
                checked
                    .validate_resolved_accessor_records(getter, setter)
                    .map_err(Error::Accessor)?;
                let mut slots = BTreeSet::new();
                for record in std::iter::once(getter).chain(setter) {
                    let values = record.payload().slot_relations().slots();

                    for slot in values {
                        slots.insert(*slot);
                    }
                }

                if !slots.iter().eq(payload.slot_relations().slots()) {
                    return Err(Error::Inventory("property accessor slots"));
                }
                NominalMemberPropertyProofV1::Runtime(checked.access_proof())
            }
        };
        bound.proofs.insert(record.declaration(), proof);
    }
    Ok(())
}
fn lookup(
    callables: &CanonicalNominalSourceCallablesV1,
    id: scoop_identity::PersistentPropertyAccessorId,
) -> Result<&NominalSupportCallableInterfaceV1, Error> {
    let id = CallableTemplateOrigin::Accessor(id);
    callables.get(id).ok_or(Error::MissingCallable(id))
}
