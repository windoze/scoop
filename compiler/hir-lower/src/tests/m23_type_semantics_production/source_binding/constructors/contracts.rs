use super::*;

#[test]
fn constructor_origin_must_match_its_exact_foundation_subject() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let record = &sources.constructors.records()[0];
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
        sources.replace(
            hir::NominalSupportConstructorInterfaceV1::try_new(
                declaration,
                hir::DeclarationAccessSourceV1::try_new(
                    access.declared_visibility(),
                    access.lexical_owners().to_vec(),
                    other.clone(),
                )
                .unwrap(),
                record.payload().clone(),
            )
            .unwrap(),
        );
        assert!(
            matches!(sources.bind(&foundation), Err(Error::DefinitionOrigin(actual)) if actual == declaration)
        );
    });
}

#[test]
fn constructor_access_cannot_forge_visibility_or_lexical_owners() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let record = &sources.constructors.records()[0];
        let access = record.declaration_access();
        let declaration = record.declaration();
        for visibility in [
            hir::DeclaredVisibilityV1::Private,
            hir::DeclaredVisibilityV1::Internal,
        ] {
            let mut forged = sources.clone();
            forged.replace(
                hir::NominalSupportConstructorInterfaceV1::try_new(
                    declaration,
                    hir::DeclarationAccessSourceV1::try_new(
                        visibility,
                        access.lexical_owners().to_vec(),
                        access.definition_origin().clone(),
                    )
                    .unwrap(),
                    record.payload().clone(),
                )
                .unwrap(),
            );
            assert!(
                matches!(forged.bind(&foundation), Err(Error::Visibility(actual)) if actual == declaration)
            );
        }
        let other = fixture
            .source
            .entries()
            .sources
            .records()
            .iter()
            .find(|source| source.owner() != record.payload().owner())
            .unwrap()
            .owner();
        let mut owners = access.lexical_owners().to_vec();
        owners.insert(0, other);
        let mut forged = sources.clone();
        forged.replace(
            hir::NominalSupportConstructorInterfaceV1::try_new(
                declaration,
                hir::DeclarationAccessSourceV1::try_new(
                    access.declared_visibility(),
                    owners,
                    access.definition_origin().clone(),
                )
                .unwrap(),
                record.payload().clone(),
            )
            .unwrap(),
        );
        assert!(
            matches!(forged.bind(&foundation), Err(Error::Access { declaration: actual, .. }) if actual == declaration)
        );
    });
}

#[test]
fn constructor_parameter_order_and_result_are_checked_against_typed_keys() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let record = sources
            .constructors
            .records()
            .iter()
            .find(|record| record.payload().parameters().len_u32() == 2)
            .unwrap();
        let declaration = record.declaration();
        let payload = record.payload();
        let other = sources
            .constructors
            .records()
            .iter()
            .find(|other| other.payload().owner() != payload.owner())
            .unwrap()
            .payload()
            .result()
            .clone();
        for wrong_result in [false, true] {
            let mut parameters = payload.parameters().parameters().to_vec();
            let result: SignatureTypeKey = if wrong_result {
                other.clone()
            } else {
                parameters.reverse();
                payload.result().clone()
            };
            let mut forged = sources.clone();
            forged.replace(
                hir::NominalSupportConstructorInterfaceV1::try_new(
                    declaration,
                    record.declaration_access().clone(),
                    hir::NominalSourceCallablePayloadV1::try_new(
                        CallableTemplateOrigin::Constructor(declaration),
                        payload.owner(),
                        payload.type_parameters().clone(),
                        hir::CanonicalSourceParameterShapesV1::try_new(parameters).unwrap(),
                        result,
                        payload.effects(),
                        payload.modality(),
                        payload.slot_relations().clone(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            );
            assert!(
                matches!(forged.bind(&foundation), Err(Error::Signature(actual)) if actual == declaration)
            );
        }
    });
}
