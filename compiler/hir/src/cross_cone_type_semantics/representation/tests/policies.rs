use super::*;

#[test]
fn intrinsic_representation_round_trips_each_concrete_family() {
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
        round_trip(
            &mut Fixture::new(kind),
            NominalRepresentationShapeV1::Intrinsic {
                representation: NominalIntrinsicRepresentationV1::new(family),
            },
        );
    }
}

#[test]
fn representation_preserves_the_complete_c_layout_policy() {
    let mut fixture = Fixture::new(SourceNominalKind::Struct);
    let field = fixture.struct_field("value", unit());
    let policy = NominalCLayoutPolicyV1::CLayout {
        contract: HirCLayoutContract {
            aligned: HirCLayoutValue::A8,
            packed: HirCLayoutValue::A1,
        },
    };
    let record = round_trip(
        &mut fixture,
        NominalRepresentationShapeV1::Struct {
            fields: vec![field],
            c_layout_policy: policy,
        },
    );
    let NominalRepresentationShapeV1::Struct {
        c_layout_policy, ..
    } = record.shape()
    else {
        panic!("expected struct representation")
    };
    assert_eq!(*c_layout_policy, policy);
}
