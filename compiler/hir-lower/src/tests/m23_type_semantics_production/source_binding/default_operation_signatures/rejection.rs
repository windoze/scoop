use super::*;

#[test]
fn member_operation_queries_reject_omitted_generic_owner_wrong_owner_and_both_arities() {
    with_source(COMBINED, |output, bound, _| {
        let [original]: [hir::DefaultCallableRefV1; 1] = callables(&template(output, "method", 2))
            .try_into()
            .unwrap();
        let reference = hir::DefaultCallableRefV1::try_new(
            original.declaration(),
            OptionalSignatureType::Absent,
            original.type_arguments().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            bound.members().default_member_callable_shape(
                &reference,
                &mut meter(),
                &WirePath::root()
            ),
            Err(Error::MissingOwner(_))
        ));
        let reference = hir::DefaultCallableRefV1::try_new(
            original.declaration(),
            original.owner().clone(),
            vec![],
        )
        .unwrap();
        assert!(
            matches!(bound.members().default_member_callable_shape(&reference, &mut meter(), &WirePath::root()), Err(Error::Nominal(error)) if matches!(*error, hir::DefaultSourceNominalOperationError::CallableArity { expected: 1, actual: 0, .. }))
        );
        let OptionalSignatureType::Present(owner) = original.owner() else {
            panic!("generic owner");
        };
        let Type::NominalApplication { origin, arguments } = owner.as_ref() else {
            panic!("generic owner");
        };
        let owner = Type::NominalApplication {
            origin: *origin,
            arguments: NonEmptyVec::from_first(
                arguments.as_slice()[0].clone(),
                [arguments.as_slice()[0].clone()],
            ),
        };
        let reference = hir::DefaultCallableRefV1::try_new(
            original.declaration(),
            OptionalSignatureType::Present(Box::new(owner)),
            original.type_arguments().to_vec(),
        )
        .unwrap();
        assert!(
            matches!(bound.members().default_member_callable_shape(&reference, &mut meter(), &WirePath::root()), Err(Error::Nominal(error)) if matches!(*error, hir::DefaultSourceNominalOperationError::Arity { expected: 1, actual: 2, .. }))
        );
        let [nested]: [hir::DefaultCallableRefV1; 1] = callables(&template(output, "nested", 0))
            .try_into()
            .unwrap();
        let owner = bound
            .members()
            .default_member_callable_shape(&nested, &mut meter(), &WirePath::root())
            .unwrap()
            .receiver()
            .unwrap()
            .clone();
        let reference = hir::DefaultCallableRefV1::try_new(
            original.declaration(),
            OptionalSignatureType::Present(Box::new(owner)),
            original.type_arguments().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            bound.members().default_member_callable_shape(
                &reference,
                &mut meter(),
                &WirePath::root()
            ),
            Err(Error::Owner { .. })
        ));
        with_core_source(SOURCE, |_, other, _| {
            assert!(matches!(
                other.members().default_member_callable_shape(
                    &original,
                    &mut meter(),
                    &WirePath::root()
                ),
                Err(Error::Member(_))
            ));
        });
    });
}

#[test]
fn constructor_operation_queries_reject_a_different_applied_owner() {
    with_core_source(SOURCE, |output, bound, _| {
        let mut reference = constructor(&template(output, "cell", 0));
        let hir::DefaultConstructorRefV1::Class { owner_type, .. } = &mut reference else {
            panic!("class constructor");
        };
        *owner_type = template(output, "empty", 0).result().clone();
        assert!(matches!(
            bound.default_constructor_operation_shape(&reference, &mut meter(), &WirePath::root()),
            Err(Error::Target(_))
        ));
    });
}
