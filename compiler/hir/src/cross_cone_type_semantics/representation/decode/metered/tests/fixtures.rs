use super::*;

pub(super) fn structure(
    value: SignatureTypeKey,
    count: usize,
) -> (Fixture, NominalRepresentationSupportV1) {
    let mut fixture = Fixture::new(SourceNominalKind::Struct);
    let fields = (0..count)
        .map(|index| fixture.struct_field(&format!("f{index}"), value.clone()))
        .collect();
    let record = NominalRepresentationSupportV1::try_new(
        &fixture.key,
        fixture.access.clone(),
        NominalRepresentationShapeV1::Struct {
            fields,
            c_layout_policy: NominalCLayoutPolicyV1::Ordinary,
        },
    )
    .unwrap();
    (fixture, record)
}
pub(super) fn cases() -> Vec<(Fixture, NominalRepresentationSupportV1)> {
    let mut result = vec![structure(unit(), 2)];
    let mut fixture = Fixture::new(SourceNominalKind::Enum);
    let shape = NominalRepresentationShapeV1::Enum {
        variants: vec![
            fixture.variant("Payload", true),
            fixture.variant("Empty", false),
        ],
    };
    result.push(build(fixture, shape));
    let mut fixture = Fixture::new(SourceNominalKind::Class);
    let shape = NominalRepresentationShapeV1::Class {
        base: scoop_identity::OptionalSignatureType::Absent,
        declared_fields: vec![fixture.class_field("backing")],
    };
    result.push(build(fixture, shape));
    let mut fixture = Fixture::new(SourceNominalKind::Object);
    let shape = NominalRepresentationShapeV1::Object {
        backing_class: fixture.backing(),
        declared_fields: vec![fixture.class_field("backing")],
    };
    result.push(build(fixture, shape));
    result.push(build(
        Fixture::new(SourceNominalKind::Interface),
        NominalRepresentationShapeV1::Interface,
    ));
    result.push(build(
        Fixture::new(SourceNominalKind::Struct),
        NominalRepresentationShapeV1::Intrinsic {
            representation: NominalIntrinsicRepresentationV1::new(IntrinsicTypeKind::Boolean),
        },
    ));
    result
}
fn build(
    fixture: Fixture,
    shape: NominalRepresentationShapeV1,
) -> (Fixture, NominalRepresentationSupportV1) {
    let record =
        NominalRepresentationSupportV1::try_new(&fixture.key, fixture.access.clone(), shape)
            .unwrap();
    (fixture, record)
}
