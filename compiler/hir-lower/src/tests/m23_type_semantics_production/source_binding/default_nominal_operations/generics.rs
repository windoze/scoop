use super::*;

#[test]
fn nominal_operation_fields_substitute_once_and_static_nested_owners_have_no_outer_arguments() {
    with_source(COMBINED, |output, nominals| {
        let pair = value_type(output, "pair", 1);
        let Type::NominalApplication { origin, arguments } = &pair else {
            panic!("generic pair");
        };
        let [left, right] = arguments.as_slice() else {
            panic!("two owner arguments");
        };
        assert_ne!(left, right);
        let expected = vec![
            right.clone(),
            left.clone(),
            Type::Tuple(NonEmptyVec::from_first(left.clone(), [right.clone()])),
        ];
        let Shape::Aggregate(shape) = query(nominals, Target::Struct(&pair)) else {
            panic!("struct aggregate");
        };
        assert_eq!(shape.fields(), expected);
        let hir::NominalSourceShapeV1::Struct(pair_source) = source(nominals, &pair).source_shape()
        else {
            panic!("source struct");
        };
        for (index, field) in pair_source.fields().iter().enumerate() {
            let Shape::Field(shape) = query(
                nominals,
                Target::StructField {
                    declaration: field.field(),
                    owner_type: &pair,
                },
            ) else {
                panic!("struct field");
            };
            assert_eq!(shape.value_type(), &expected[index]);
        }
        let remapped = Type::NominalApplication {
            origin: *origin,
            arguments: NonEmptyVec::from_first(
                Type::Binder { depth: 7, index: 4 },
                [Type::Binder { depth: 3, index: 9 }],
            ),
        };
        let Shape::Aggregate(shape) = query(nominals, Target::Struct(&remapped)) else {
            panic!("struct aggregate");
        };
        assert_eq!(shape.fields()[0], Type::Binder { depth: 3, index: 9 });
        assert_eq!(shape.fields()[1], Type::Binder { depth: 7, index: 4 });
        let choice = value_type(output, "choice", 1);
        let hir::NominalSourceShapeV1::Enum(variants) = source(nominals, &choice).source_shape()
        else {
            panic!("source enum");
        };
        for (index, expected) in [
            vec![],
            vec![left.clone(), right.clone()],
            vec![right.clone(), left.clone()],
        ]
        .iter()
        .enumerate()
        {
            let variant = &variants.variants()[index];
            let reference = hir::DefaultEnumVariantRefV1::new(variant.variant(), choice.clone());
            let Shape::Aggregate(shape) = query(nominals, Target::Variant(&reference)) else {
                panic!("variant aggregate");
            };
            assert_eq!(shape.fields(), expected);
            for (position, field) in variant.fields().iter().enumerate() {
                let reference =
                    hir::DefaultEnumVariantFieldRefV1::new(field.field(), choice.clone());
                let Shape::VariantField(shape) = query(nominals, Target::VariantField(&reference))
                else {
                    panic!("variant field");
                };
                assert_eq!(shape.value_type(), &expected[position]);
            }
        }
        let nested = value_type(output, "nested", 0);
        assert!(matches!(nested, Type::Nominal(_)));
        let Shape::Aggregate(shape) = query(nominals, Target::Struct(&nested)) else {
            panic!("static nested struct");
        };
        assert_eq!(shape.fields().len(), 1);
    });
}
