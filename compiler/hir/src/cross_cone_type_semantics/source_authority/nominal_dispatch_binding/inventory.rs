use super::*;

pub(super) fn validate(
    bound: &BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>,
) -> Result<(), Error> {
    let protected = members::collect(bound)?;
    let parameters = bound.parameters;
    let nominals = parameters.members().nominals;
    let path = WirePath::root();
    for inventory in bound.inventory().records() {
        let node = bound
            .slots
            .graph()
            .get(inventory.owner())
            .ok_or(Error::Inventory {
                owner: inventory.owner(),
                field: "owner",
            })?;

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
            let record = parameters
                .constructors()
                .constructor_source(*id)
                .map_err(NominalNestedBindingError::from)?;
            let owners = record.declaration_access().lexical_owners();

            if owners
                .iter()
                .any(|owner| matches!(owner, SourceNominalId::GenericTemplate(_)))
            {
                continue;
            }
            if matches!(
                record.declaration_access().declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            ) {
                scoop_wire::allocation::try_reserve(&mut constructors, 1, &path)?;
                constructors.push(*id);
            }
        }

        if constructors != inventory.constructors().values() {
            return Err(Error::Inventory {
                owner: inventory.owner(),
                field: "constructors",
            });
        }

        let expected = protected.get(&node.source());

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
