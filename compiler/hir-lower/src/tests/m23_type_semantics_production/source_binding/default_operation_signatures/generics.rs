use super::*;

#[test]
fn member_operation_signatures_substitute_owner_and_own_binders_once() {
    with_source(COMBINED, |output, bound, _| {
        let template = template(output, "method", 2);
        let [reference]: [hir::DefaultCallableRefV1; 1] = callables(&template).try_into().unwrap();
        let shape = bound
            .members()
            .default_member_callable_shape(&reference, &mut meter(), &WirePath::root())
            .unwrap();
        assert_eq!(shape.result(), template.result());
        assert_eq!(shape.parameters(), reference.type_arguments());
        let OptionalSignatureType::Present(owner) = reference.owner() else {
            panic!("applied owner");
        };
        assert_eq!(shape.receiver(), Some(owner.as_ref()));
        let Type::NominalApplication { origin, .. } = owner.as_ref() else {
            panic!("generic owner");
        };
        let left = Type::Binder { depth: 7, index: 4 };
        let right = Type::Binder { depth: 3, index: 9 };
        let owner = Type::NominalApplication {
            origin: *origin,
            arguments: NonEmptyVec::from_first(left.clone(), []),
        };
        let remapped = hir::DefaultCallableRefV1::try_new(
            reference.declaration(),
            OptionalSignatureType::Present(Box::new(owner.clone())),
            vec![right.clone()],
        )
        .unwrap();
        let shape = bound
            .members()
            .default_member_callable_shape(&remapped, &mut meter(), &WirePath::root())
            .unwrap();
        assert_eq!(
            shape.result(),
            &Type::Tuple(NonEmptyVec::from_first(left, [right.clone()]))
        );
        assert_eq!(shape.parameters(), &[right]);
        assert_eq!(shape.receiver(), Some(&owner));
    });
}

#[test]
fn owner_only_generic_and_static_nested_signatures_preserve_their_source_frames() {
    with_source(COMBINED, |output, bound, _| {
        for (name, position) in [("echo", 2), ("nested", 0)] {
            let template = template(output, name, position);
            let [reference]: [hir::DefaultCallableRefV1; 1] =
                callables(&template).try_into().unwrap();
            assert!(reference.type_arguments().is_empty());
            let shape = bound
                .members()
                .default_member_callable_shape(&reference, &mut meter(), &WirePath::root())
                .unwrap();
            assert_eq!(shape.result(), template.result());
            assert_eq!(shape.parameters(), std::slice::from_ref(template.result()));
            assert!(shape.captures().is_empty());
            if name == "nested" {
                assert!(matches!(shape.receiver(), Some(Type::Nominal(_))));
            }
        }
        let template = template(output, "constructed", 1);
        let reference = constructor(&template);
        let Shape::Constructor {
            owner_type,
            parameters,
        } = bound
            .default_constructor_operation_shape(&reference, &mut meter(), &WirePath::root())
            .unwrap()
        else {
            panic!("constructor shape");
        };
        let Type::NominalApplication { arguments, .. } = &owner_type else {
            panic!("applied owner");
        };
        assert_eq!(parameters, arguments.as_slice());
        assert_eq!(&owner_type, template.result());
    });
}
