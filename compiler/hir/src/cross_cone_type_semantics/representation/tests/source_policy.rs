use super::*;

#[test]
fn intrinsic_representation_requires_its_exact_source_family_in_both_join_paths() {
    for family in crate::IntegerKind::ALL
        .into_iter()
        .map(crate::IntrinsicTypeKind::Integer)
        .chain([
            crate::IntrinsicTypeKind::Boolean,
            crate::IntrinsicTypeKind::String,
        ])
    {
        let kind = match family.target() {
            crate::IntrinsicTypeTarget::Struct => SourceNominalKind::Struct,
            crate::IntrinsicTypeTarget::Class => SourceNominalKind::Class,
        };
        let mut fixture = Fixture::new(kind);
        let representation = NominalIntrinsicRepresentationV1::new(family);
        let record = round_trip(
            &mut fixture,
            NominalRepresentationShapeV1::Intrinsic { representation },
        );
        for source in [
            NominalSourceShapeV1::Intrinsic(representation),
            NominalSourceShapeV1::Intrinsic(NominalIntrinsicRepresentationV1::new(
                crate::IntrinsicTypeKind::Array,
            )),
            NominalSourceShapeV1::Struct(
                StructSourceShapeV1::try_new(vec![], NominalCLayoutPolicyV1::Ordinary, false)
                    .unwrap(),
            ),
            NominalSourceShapeV1::Class(Default::default()),
        ] {
            let agrees = source == NominalSourceShapeV1::Intrinsic(representation);
            assert_eq!(record.validate_public_source_shape(&source).is_ok(), agrees);
            assert_eq!(
                record
                    .public_source_shape_matches(
                        &source,
                        &mut scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default()),
                        &scoop_wire::WirePath::root()
                    )
                    .unwrap(),
                agrees
            );
        }
        let other = NominalSourceShapeV1::Intrinsic(NominalIntrinsicRepresentationV1::new(
            crate::IntrinsicTypeKind::Integer(crate::IntegerKind::UNSIGNED_64),
        ));
        assert_eq!(
            record.validate_public_source_shape(&other).is_ok(),
            family == crate::IntrinsicTypeKind::Integer(crate::IntegerKind::UNSIGNED_64)
        );
    }
}

#[test]
fn representation_requires_the_complete_public_c_layout_policy() {
    let mut fixture = Fixture::new(SourceNominalKind::Struct);
    let field = fixture.struct_field("value", unit());
    let expected = NominalCLayoutPolicyV1::CLayout {
        contract: HirCLayoutContract {
            aligned: HirCLayoutValue::A8,
            packed: HirCLayoutValue::A1,
        },
    };
    let record = round_trip(
        &mut fixture,
        NominalRepresentationShapeV1::Struct {
            fields: vec![field.clone()],
            c_layout_policy: expected,
        },
    );
    for actual in [
        expected,
        NominalCLayoutPolicyV1::Ordinary,
        NominalCLayoutPolicyV1::CLayout {
            contract: HirCLayoutContract {
                aligned: HirCLayoutValue::A16,
                packed: HirCLayoutValue::A1,
            },
        },
        NominalCLayoutPolicyV1::CLayout {
            contract: HirCLayoutContract {
                aligned: HirCLayoutValue::A8,
                packed: HirCLayoutValue::A2,
            },
        },
    ] {
        let source = NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(
                vec![NominalSourceFieldV1::new(
                    field.field(),
                    field.value_type().clone(),
                )],
                actual,
                false,
            )
            .unwrap(),
        );
        let result = record.validate_public_source_shape(&source);
        if actual == expected {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(NominalRepresentationBuildError::PublicSourceShape)
            ));
        }
    }
}
