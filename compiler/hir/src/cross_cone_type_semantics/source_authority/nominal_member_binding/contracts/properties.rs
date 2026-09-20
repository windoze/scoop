use super::*;

pub(in super::super) fn validate(
    bound: &mut BoundNominalMemberSourcesV1<'_, '_, '_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let properties = bound.properties;
    let callables = bound.callables;
    binding_keys::charge_map(properties.records().len(), meter, &WirePath::root())?;
    for record in properties.records() {
        let checked = record
            .validate_source(graph, bound, meter)
            .map_err(Error::from_property)?;
        let proof = match checked {
            CheckedNominalSupportPropertySourceV1::Const(_) => NominalMemberPropertyProofV1::Const,
            CheckedNominalSupportPropertySourceV1::Runtime(checked) => {
                let payload = checked.payload();
                let getter = lookup(callables, payload.getter(), meter)?;
                let setter = match payload.mutability() {
                    ProtectedPropertyMutabilityV1::ReadOnly => None,
                    ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } => {
                        Some(lookup(callables, *setter, meter)?)
                    }
                };
                // Every callable above has already passed source semantics.
                checked
                    .validate_resolved_accessor_records(getter, setter, meter)
                    .map_err(Error::Accessor)?;
                let mut slots = BTreeSet::new();
                for record in std::iter::once(getter).chain(setter) {
                    let values = record.payload().slot_relations().slots();
                    binding_keys::charge_map(values.len(), meter, &WirePath::root())?;
                    for slot in values {
                        query(slots.len(), meter)?;
                        meter.check_table_entries(slots.len() as u64 + 1, &WirePath::root())?;
                        slots.insert(*slot);
                    }
                }
                meter.charge_work(slots.len() as u64, &WirePath::root())?;
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
fn lookup<'a>(
    callables: &'a CanonicalNominalSourceCallablesV1,
    id: scoop_identity::PersistentPropertyAccessorId,
    meter: &mut BudgetMeter,
) -> Result<&'a NominalSupportCallableInterfaceV1, Error> {
    query(callables.records().len(), meter)?;
    let id = CallableTemplateOrigin::Accessor(id);
    callables.get(id).ok_or(Error::MissingCallable(id))
}
