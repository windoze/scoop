use super::*;

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
                vec![StructSourceFieldV1::new(
                    field.field(),
                    field.value_type().clone(),
                )],
                actual,
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
