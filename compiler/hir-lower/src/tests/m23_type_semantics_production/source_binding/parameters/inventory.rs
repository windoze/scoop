use super::*;

#[test]
fn parameter_protocols_require_exact_owned_declaration_inventory() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let unused =
            output
                .output()
                .export
                .function_identities
                .iter()
                .find_map(|(_, identity)| {
                    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(
                        record,
                    )) = identity
                    else {
                        return None;
                    };
                    let owner = CallableTemplateOrigin::Function(record.id());
                    sources.protocols.get(owner).is_none().then_some(owner)
                })
                .unwrap();
        for extra in [false, true] {
            let mut forged = sources.clone();
            let mut records = sources.protocols.records().to_vec();
            if extra {
                records.push(Record::try_new(unused, vec![]).unwrap());
            } else {
                records.pop().unwrap();
            }
            forged.protocols = Table::try_new(records).unwrap();
            assert!(matches!(
                forged.bind(&foundation, inputs.protocols().fundamental_types()),
                Err(Error::Inventory)
            ));
        }
    });
}

#[test]
fn parameter_sources_cannot_mix_distinct_bound_foundations() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let other = fixture.bind().unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let core = inputs.protocols().fundamental_types();
        let protected = sources.protected(&foundation);
        let constructors = other
            .bind_inheritance_constructor_sources(
                &sources.properties.dispatch.inventory,
                &sources.constructors,
            )
            .unwrap();
        assert!(matches!(
            protected.bind_parameter_protocols(&constructors, &sources.protocols, core),
            Err(Error::FoundationMismatch)
        ));
    });
}

#[test]
fn constructor_and_protected_source_inventories_must_agree() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let core = inputs.protocols().fundamental_types();
        let protected = sources.protected(&foundation);
        let mut records = sources.properties.dispatch.inventory.records().to_vec();
        let owner = records
            .iter_mut()
            .find(|owner| !owner.protected_members().values().is_empty())
            .unwrap();
        *owner = hir::SourceInheritanceInventoryV1::try_new(
            owner.owner(),
            owner.constructors().clone(),
            hir::CanonicalProtectedDeclarationRefsV1::try_new(vec![]).unwrap(),
            owner.slot_schemas().clone(),
        )
        .unwrap();
        let inventory = hir::CanonicalSourceInheritanceInventoriesV1::try_new(records).unwrap();
        let constructors = foundation
            .bind_inheritance_constructor_sources(&inventory, &sources.constructors)
            .unwrap();
        assert!(matches!(
            protected.bind_parameter_protocols(&constructors, &sources.protocols, core),
            Err(Error::Inventory)
        ));
    });
}
