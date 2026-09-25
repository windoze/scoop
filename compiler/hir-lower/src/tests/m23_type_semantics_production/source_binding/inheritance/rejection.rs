use super::*;

mod ownership;

#[test]
fn inheritance_sources_require_equal_inventories_and_nominal_modalities() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();

        let dispatch = sources.properties.dispatch.bind(&foundation).unwrap();
        let slots = dispatch.bind_slot_sources().unwrap();
        let properties = sources.properties.bind(&foundation).unwrap();
        let protected = properties
            .bind_protected_callable_sources(&sources.callables)
            .unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        let mut records = sources.properties.dispatch.inventory.records().to_vec();
        let row = records
            .iter_mut()
            .find(|row| !row.protected_members().values().is_empty())
            .unwrap();
        *row = hir::SourceInheritanceInventoryV1::try_new(
            row.owner(),
            row.constructors().clone(),
            hir::CanonicalProtectedDeclarationRefsV1::try_new(vec![]).unwrap(),
            row.slot_schemas().clone(),
        )
        .unwrap();
        let inventory = hir::CanonicalSourceInheritanceInventoriesV1::try_new(records).unwrap();
        let constructors = foundation
            .bind_inheritance_constructor_sources(&inventory, &sources.constructors)
            .unwrap();
        assert!(matches!(
            protected.bind_inheritance_sources(&constructors, &nominals, &slots),
            Err(Error::Inventory)
        ));
        let mut records = sources.nominals.records().to_vec();
        let source = records
            .iter_mut()
            .find(|source| source.modality() == hir::NominalInheritanceModalityV1::Open)
            .unwrap();
        let owner = source.owner();
        *source = hir::NominalSourceContractV1::try_new(
            owner,
            hir::NominalInheritanceModalityV1::Final,
            source.type_parameters().clone(),
            source.supertypes().clone(),
            source.constructors().clone(),
            source.members().clone(),
            source.children().clone(),
            source.source_shape().clone(),
        )
        .unwrap();
        let mut changed = sources.clone();
        changed.nominals = hir::CanonicalNominalSourceContractsV1::try_new(records).unwrap();
        assert!(
            matches!(changed.with_bound(&foundation,  |_, _| ()), Err(Error::Modality(actual)) if actual == owner)
        );
    });
}
