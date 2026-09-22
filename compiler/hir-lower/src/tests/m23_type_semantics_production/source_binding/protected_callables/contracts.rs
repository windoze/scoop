use super::*;

mod signatures;

fn changed_signature(
    record: &Record,
    binders: hir::CanonicalBinderListV1,
    parameters: hir::CanonicalSourceParameterShapesV1,
    result: SignatureTypeKey,
) -> Record {
    let old = record.payload();
    Record::try_new(
        record.declaration(),
        record.declaration_access().clone(),
        hir::ProtectedCallablePayloadV1::try_new(
            record.declaration(),
            old.owner(),
            binders,
            parameters,
            result,
            old.effects(),
            old.modality(),
            old.slot_relations().clone(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn protected_origins_must_match_the_exact_typed_foundation_subject() {
    with_source(DIRECT, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();

        for record in sources.callables.records() {
            let old = record.declaration_access();
            let other = fixture
                .source
                .entries()
                .definition_sources
                .sources()
                .iter()
                .find(|source| *source != old.definition_origin())
                .unwrap();
            assert!(foundation.contains_definition_source(other));
            let mut forged = sources.clone();
            forged.replace(
                Record::try_new(
                    record.declaration(),
                    hir::DeclarationAccessSourceV1::try_new(
                        old.declared_visibility(),
                        old.lexical_owners().to_vec(),
                        other.clone(),
                    )
                    .unwrap(),
                    record.payload().clone(),
                )
                .unwrap(),
            );
            let expected = match record.declaration() {
                CallableTemplateOrigin::Function(id) => {
                    scoop_identity::DefinitionOriginSubject::Function(id)
                }
                CallableTemplateOrigin::GenericFunction(id) => {
                    scoop_identity::DefinitionOriginSubject::GenericFunction(id)
                }
                CallableTemplateOrigin::Accessor(id) => {
                    scoop_identity::DefinitionOriginSubject::PropertyAccessor(id)
                }
                _ => panic!("protected callable"),
            };
            assert!(matches!(forged.bind(&foundation,
                &mut meter()), Err(Error::DefinitionOrigin(actual)) if actual == expected));
        }
    });
}

#[test]
fn protected_sources_cannot_truncate_nested_lexical_owners() {
    with_source(DIRECT, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();

        let mut checked = 0;
        for record in sources
            .callables
            .records()
            .iter()
            .filter(|r| r.declaration_access().lexical_owners().len() == 2)
        {
            let old = record.declaration_access();
            let mut owners = old.lexical_owners().to_vec();
            owners.remove(0);
            let mut forged = sources.clone();
            forged.replace(
                Record::try_new(
                    record.declaration(),
                    hir::DeclarationAccessSourceV1::try_new(
                        old.declared_visibility(),
                        owners,
                        old.definition_origin().clone(),
                    )
                    .unwrap(),
                    record.payload().clone(),
                )
                .unwrap(),
            );
            let error = forged.bind(&foundation, &mut meter()).unwrap_err();
            match (record.declaration(), error) {
                (CallableTemplateOrigin::Accessor(id), Error::AccessorAccess(actual)) => {
                    assert_eq!(id, actual)
                }
                (_, Error::Semantic(error)) => assert!(matches!(
                    *error,
                    hir::ProtectedCallableSemanticError::Source(_)
                )),
                (_, error) => panic!("unexpected lexical error: {error}"),
            }
            checked += 1;
        }
        assert_eq!(checked, 4);
    });
}
