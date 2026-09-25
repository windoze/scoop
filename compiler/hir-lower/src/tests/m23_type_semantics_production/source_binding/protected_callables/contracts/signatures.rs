use super::*;

#[test]
fn protected_parameters_and_generic_binders_must_match_source_keys() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();

        let record = sources
            .callables
            .records()
            .iter()
            .find(|r| r.payload().parameters().len_u32() == 2)
            .unwrap();
        let generic = sources
            .callables
            .records()
            .iter()
            .find(|r| matches!(r.declaration(), CallableTemplateOrigin::GenericFunction(_)))
            .unwrap();
        for wrong_binders in [false, true] {
            let record = if wrong_binders { generic } else { record };
            let old = record.payload();
            let mut parameters = old.parameters().parameters().to_vec();
            let binders = if wrong_binders {
                let mut binders = old.type_parameters().binders().to_vec();
                binders.push(hir::TypeParameterBinderV1::new(
                    scoop_identity::CanonicalIdentifier::new("U").unwrap(),
                    binders[0].bounds().clone(),
                ));
                hir::CanonicalBinderListV1::try_new(binders).unwrap()
            } else {
                parameters.reverse();
                old.type_parameters().clone()
            };
            let mut forged = sources.clone();
            forged.replace(changed_signature(
                record,
                binders,
                hir::CanonicalSourceParameterShapesV1::try_new(parameters).unwrap(),
                old.result().clone(),
            ));
            let Error::Semantic(error) = forged.bind(&foundation).unwrap_err() else {
                panic!("source signature mismatch")
            };
            assert!(matches!(
                (wrong_binders, *error),
                (false, hir::ProtectedCallableSemanticError::ParameterShape)
                    | (true, hir::ProtectedCallableSemanticError::Identity)
            ));
        }
        let old = generic.payload();
        let mut forged = sources.clone();
        forged.replace(changed_signature(
            generic,
            old.type_parameters().clone(),
            old.parameters().clone(),
            SignatureTypeKey::Binder { depth: 1, index: 0 },
        ));
        let Error::Semantic(error) = forged.bind(&foundation).unwrap_err() else {
            panic!("out-of-scope source binder")
        };
        assert!(matches!(
            *error,
            hir::ProtectedCallableSemanticError::Signature(_)
        ));
    });
}

#[test]
fn accessors_replay_property_value_types_and_the_language_unit_identity() {
    with_source(DIRECT, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();

        let mut checked = 0;
        for record in sources.callables.records() {
            let CallableTemplateOrigin::Accessor(id) = record.declaration() else {
                continue;
            };
            let old = record.payload();
            let role = foundation.accessor_key(id).unwrap().role();
            let wrong_result = match role {
                scoop_identity::AccessorRole::Getter => SignatureTypeKey::Nominal(
                    scoop_identity::CoreBuiltinNominal::Unit
                        .identity_record()
                        .id(),
                ),
                scoop_identity::AccessorRole::Setter => {
                    old.parameters().parameters()[0].value_type().clone()
                }
            };
            assert_ne!(&wrong_result, old.result());
            let mut forged = sources.clone();
            forged.replace(changed_signature(
                record,
                old.type_parameters().clone(),
                old.parameters().clone(),
                wrong_result,
            ));
            let Error::Semantic(error) = forged.bind(&foundation).unwrap_err() else {
                panic!("wrong accessor result")
            };
            assert!(matches!(
                *error,
                hir::ProtectedCallableSemanticError::Result
            ));
            if role == scoop_identity::AccessorRole::Setter {
                let parameter = &old.parameters().parameters()[0];
                let parameters = hir::CanonicalSourceParameterShapesV1::try_new(vec![
                    hir::SourceParameterShapeV1::new(
                        parameter.name().clone(),
                        old.result().clone(),
                    ),
                ])
                .unwrap();
                let mut forged = sources.clone();
                forged.replace(changed_signature(
                    record,
                    old.type_parameters().clone(),
                    parameters,
                    old.result().clone(),
                ));
                let Error::Semantic(error) = forged.bind(&foundation).unwrap_err() else {
                    panic!("wrong setter parameter")
                };
                assert!(matches!(
                    *error,
                    hir::ProtectedCallableSemanticError::ParameterShape
                ));
            }
            checked += 1;
        }
        assert_eq!(checked, 2);
    });
}
