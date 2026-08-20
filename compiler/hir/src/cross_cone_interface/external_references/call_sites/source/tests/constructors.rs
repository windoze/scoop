use super::*;

#[test]
fn source_constructor_does_not_turn_an_unsubstituted_owner_into_a_zero_argument_call() {
    let owner = generic_nominal().id();
    let fixture = Fixture::new(
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(owner)),
        None,
        Vec::new(),
        SignatureTypeKey::NominalApplication {
            origin: owner,
            arguments: NonEmptyVec::new(vec![SignatureTypeKey::Binder { depth: 0, index: 0 }])
                .unwrap(),
        },
        Vec::new(),
    )
    .into_constructor();
    assert!(matches!(
        validate(&fixture, &fixture.call(0, Vec::new(), unit_exact())),
        Err(HirDependencyCallSignatureError::GenericDeclaration(_))
    ));
}

#[test]
fn source_constructor_retains_structured_arguments_without_an_initializer_receiver() {
    let owner = nominal().id();
    let tuple = ExactTypeKey::Tuple(NonEmptyVec::new(vec![unit_exact(), bool_exact()]).unwrap());
    let fixture = Fixture::new(
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)),
        None,
        vec![
            SignatureTypeKey::Tuple(NonEmptyVec::new(vec![unit(), boolean()]).unwrap()),
            unit(),
        ],
        SignatureTypeKey::Nominal(owner),
        vec![tuple.clone()],
    )
    .into_constructor();
    validate(
        &fixture,
        &fixture.call(
            0,
            vec![exact(tuple.clone()), unit_exact()],
            exact(ExactTypeKey::Nominal(owner)),
        ),
    )
    .unwrap();
    assert!(matches!(
        validate(
            &fixture,
            &fixture.call(
                1,
                vec![
                    exact(ExactTypeKey::Nominal(owner)),
                    exact(tuple),
                    unit_exact()
                ],
                unit_exact()
            )
        ),
        Err(HirDependencyCallSignatureError::ArgumentCount {
            expected: 2,
            actual: 3
        })
    ));
}
