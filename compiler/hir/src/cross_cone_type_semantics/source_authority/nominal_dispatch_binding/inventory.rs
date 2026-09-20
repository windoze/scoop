use super::*;

pub(super) fn validate(
    bound: &BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let protected = members::collect(bound, meter)?;
    let parameters = bound.parameters;
    let nominals = parameters.members().nominals;
    let path = WirePath::root();
    for inventory in bound.inventory().records() {
        meter.charge_nodes(1, &path)?;
        query(bound.inventory().records().len(), meter)?;
        let node = bound
            .slots
            .graph()
            .get(inventory.owner())
            .ok_or(Error::Inventory {
                owner: inventory.owner(),
                field: "owner",
            })?;
        query(nominals.table().records().len(), meter)?;
        let source = nominals
            .nominal_source(node.source())
            .map_err(NominalNestedBindingError::from)?;
        if source.modality() != node.edges().modality() {
            return Err(Error::Inventory {
                owner: inventory.owner(),
                field: "modality",
            });
        }
        let mut constructors = Vec::new();
        for id in source.constructors().values() {
            query(parameters.constructors().table().records().len(), meter)?;
            let record = parameters
                .constructors()
                .constructor_source(*id)
                .map_err(NominalNestedBindingError::from)?;
            if matches!(
                record.declaration_access().declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            ) {
                meter.try_reserve_collection_slots(&mut constructors, 1, &path)?;
                constructors.push(*id);
            }
        }
        meter.charge_work(
            (constructors.len() as u64 + inventory.constructors().values().len() as u64)
                .saturating_mul(64),
            &path,
        )?;
        if constructors != inventory.constructors().values() {
            return Err(Error::Inventory {
                owner: inventory.owner(),
                field: "constructors",
            });
        }
        query(protected.len(), meter)?;
        let expected = protected.get(&node.source());
        meter.charge_work(
            (expected.map_or(0, BTreeSet::len) as u64
                + inventory.protected_members().values().len() as u64)
                .saturating_mul(128),
            &path,
        )?;
        if !expected.into_iter().flatten().copied().eq(inventory
            .protected_members()
            .values()
            .iter()
            .copied())
        {
            return Err(Error::Inventory {
                owner: inventory.owner(),
                field: "protected members",
            });
        }
    }
    Ok(())
}
