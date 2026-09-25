use super::*;

#[test]
fn dispatch_keys_must_be_owned_even_when_the_validated_graph_contains_them() {
    with_source(INTERFACES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        for field in [1, 2, 3] {
            let mut canonical = fixture.foundation.as_canonical().clone();
            match field {
                1 => canonical.set_functions(vec![]).unwrap(),
                2 => canonical.set_properties(vec![]).unwrap(),
                3 => canonical.set_dispatch_slots(vec![]).unwrap(),
                _ => unreachable!(),
            }
            let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let foundation = fixture
                .source
                .bind_to_foundation(&incomplete, &fixture.identities)
                .unwrap();
            let error = sources.bind(&foundation).unwrap_err();
            assert!(matches!(
                (field, error),
                (1, Error::MissingFunction(_))
                    | (2, Error::MissingProperty(_))
                    | (3, Error::MissingSlot(_))
            ));
        }
    });
}

#[test]
fn dispatch_access_joins_the_exact_function_or_accessor_definition_origin() {
    with_source(INTERFACES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        for role in [0, 1, 2] {
            let mut forged = sources.clone();
            let mut records = forged.callables.records().to_vec();
            let record = records
                .iter_mut()
                .find(|record| match record.declaration() {
                    hir::InheritanceCallableDeclarationV1::Function(_) => role == 0,
                    hir::InheritanceCallableDeclarationV1::Getter(_) => role == 1,
                    hir::InheritanceCallableDeclarationV1::Setter(_) => role == 2,
                })
                .unwrap();
            let access = record.declaration_access();
            let other = fixture
                .source
                .entries()
                .definition_sources
                .sources()
                .iter()
                .find(|origin| *origin != access.definition_origin())
                .unwrap();
            assert!(foundation.contains_definition_source(other));
            let declaration = record.declaration();
            *record = hir::InheritanceSourceCallableV1::new(
                declaration,
                record.signature().clone(),
                record.modality(),
                hir::DeclarationAccessSourceV1::try_new(
                    access.declared_visibility(),
                    access.lexical_owners().to_vec(),
                    other.clone(),
                )
                .unwrap(),
            );
            forged.callables =
                hir::CanonicalInheritanceSourceCallablesV1::try_new(records).unwrap();
            assert!(
                matches!(forged.bind(&foundation), Err(Error::DefinitionOrigin(actual)) if actual == declaration)
            );
        }
    });
}

#[test]
fn dispatch_access_cannot_forge_its_lexical_owner_chain() {
    with_source(INTERFACES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let mut records = sources.callables.records().to_vec();
        let record = records
            .iter_mut()
            .find(|record| {
                record.declaration_access().declared_visibility()
                    == hir::DeclaredVisibilityV1::Public
            })
            .unwrap();
        assert!(!record.declaration_access().lexical_owners().is_empty());
        let declaration = record.declaration();
        *record = hir::InheritanceSourceCallableV1::new(
            declaration,
            record.signature().clone(),
            record.modality(),
            hir::DeclarationAccessSourceV1::try_new(
                hir::DeclaredVisibilityV1::Public,
                vec![],
                record.declaration_access().definition_origin().clone(),
            )
            .unwrap(),
        );
        sources.callables = hir::CanonicalInheritanceSourceCallablesV1::try_new(records).unwrap();
        assert!(
            matches!(sources.bind(&foundation), Err(Error::Access { declaration: actual, .. }) if actual == declaration)
        );
    });
}
