use super::*;

#[test]
fn nominal_operations_reject_wrong_kinds_field_owners_and_enum_owners() {
    with_source(SOURCE, |output, nominals| {
        let packet = value_type(output, "packet", 0);
        let empty = value_type(output, "empty", 0);
        let choice = value_type(output, "choice", 0);
        let other = value_type(output, "other", 0);
        let hir::NominalSourceShapeV1::Struct(fields) = source(nominals, &packet).source_shape()
        else {
            panic!("struct");
        };
        assert!(
            matches!(nominals.default_nominal_operation_shape(Target::StructField { declaration: fields.fields()[0].field(), owner_type: &empty },  &WirePath::root()), Err(Error::Nominal(error)) if matches!(*error, hir::NominalSourceBindingError::FieldOwner { .. }))
        );
        assert!(matches!(
            nominals.default_nominal_operation_shape(Target::Struct(&choice), &WirePath::root()),
            Err(Error::Kind {
                expected: hir::PublicNominalKindV1::Struct,
                actual: hir::PublicNominalKindV1::Enum,
                ..
            })
        ));
        assert!(matches!(
            nominals.default_nominal_operation_shape(
                Target::Type {
                    owner_type: &packet,
                    expected: hir::PublicNominalKindV1::Class
                },
                &WirePath::root()
            ),
            Err(Error::Kind { .. })
        ));
        let hir::NominalSourceShapeV1::Enum(variants) = source(nominals, &choice).source_shape()
        else {
            panic!("enum");
        };
        let variant = &variants.variants()[1];
        let reference = hir::DefaultEnumVariantRefV1::new(variant.variant(), other.clone());
        assert!(matches!(
            nominals
                .default_nominal_operation_shape(Target::Variant(&reference), &WirePath::root()),
            Err(Error::VariantOwner { .. })
        ));
        let reference = hir::DefaultEnumVariantFieldRefV1::new(variant.fields()[0].field(), other);
        assert!(matches!(
            nominals.default_nominal_operation_shape(
                Target::VariantField(&reference),
                &WirePath::root()
            ),
            Err(Error::VariantOwner { .. })
        ));
    });
}

#[test]
fn nominal_operations_reject_missing_artifact_sources_and_bad_owner_arity() {
    with_source(COMBINED, |output, nominals| {
        let pair = value_type(output, "pair", 1);
        let Type::NominalApplication { origin, arguments } = &pair else {
            panic!("generic pair");
        };
        let short = Type::NominalApplication {
            origin: *origin,
            arguments: NonEmptyVec::from_first(arguments.as_slice()[0].clone(), []),
        };
        assert!(matches!(
            nominals.default_nominal_operation_shape(Target::Struct(&short), &WirePath::root()),
            Err(Error::Arity {
                expected: 2,
                actual: 1,
                ..
            })
        ));
        assert!(matches!(
            nominals.default_nominal_operation_shape(
                Target::Struct(&Type::Binder { depth: 0, index: 0 }),
                &WirePath::root()
            ),
            Err(Error::NonNominalOwner)
        ));
        with_source(SOURCE, |_, other| {
            assert!(
                matches!(other.default_nominal_operation_shape(Target::Struct(&pair),  &WirePath::root()), Err(Error::Nominal(error)) if matches!(*error, hir::NominalSourceBindingError::MissingSource(_)))
            );
        });
    });
}

#[test]
fn intrinsic_source_representation_is_never_an_empty_ordinary_struct() {
    use super::super::core_foundation::support::{artifact, import, lower_extra};
    let output = lower_extra("");
    let (foundation, identities) = artifact(&output);
    let source = hir::CrossConeTypeSemanticsFoundationV1::from_hir(&output)
        .unwrap()
        .source_transcript()
        .unwrap();
    let table = hir::CanonicalNominalSourceContractsV1::from_export_hir(
        &output.export,
        &source.entries().source_roots,
    )
    .unwrap();
    let bound = source.bind_to_foundation(&foundation, &identities).unwrap();
    let nominals = bound.bind_nominal_sources(&table).unwrap();
    let definitions = hir::CompilerProtocolDefinitionsV1::from_export(&output.export).unwrap();
    let imported = import(&foundation, &identities);
    let inputs = imported.import_core_inputs(&definitions).unwrap();
    let ty = Type::Nominal(
        inputs
            .protocols()
            .fundamental_types()
            .integer(hir::IntegerKind::SIGNED_32)
            .persistent(),
    );
    assert!(matches!(
        nominals.default_nominal_operation_shape(Target::Struct(&ty), &WirePath::root()),
        Err(Error::StructRepresentation(_))
    ));
    assert_eq!(
        query(
            &nominals,
            Target::Type {
                owner_type: &ty,
                expected: hir::PublicNominalKindV1::Struct
            }
        ),
        Shape::Type(ty)
    );
}
