use super::*;

pub(super) fn structure(
    value_type: SignatureTypeKey,
    field_count: usize,
) -> (Fixture, PersistentTypeId) {
    let mut source = SourceFixture::new(SourceNominalKind::Struct);
    let fields: Vec<_> = (0..field_count)
        .map(|index| source.struct_field(&format!("field{index}"), value_type.clone()))
        .collect();
    let public = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            fields
                .iter()
                .map(|field| NominalSourceFieldV1::new(field.field(), field.value_type().clone()))
                .collect(),
            crate::NominalCLayoutPolicyV1::Ordinary,
        )
        .unwrap(),
    );
    let mut fixture = Fixture::new();
    let owner = fixture.add(
        &source,
        NominalRepresentationShapeV1::Struct {
            fields,
            c_layout_policy: NominalCLayoutPolicyV1::Ordinary,
        },
        Some(public),
    );
    (fixture, owner)
}

pub(super) fn mixed() -> Fixture {
    let (mut fixture, _) = structure(unit(), 2);
    let mut enumeration = SourceFixture::new(SourceNominalKind::Enum);
    let variants = vec![
        enumeration.variant("Payload", true),
        enumeration.variant("Empty", false),
    ];
    let public = NominalSourceShapeV1::Enum(
        EnumSourceShapeV1::try_new(
            variants
                .iter()
                .map(|variant| {
                    EnumSourceVariantV1::try_new(
                        variant.variant(),
                        if variant.fields().is_empty() {
                            EnumSourceVariantStyleV1::Unit
                        } else {
                            EnumSourceVariantStyleV1::Positional
                        },
                        variant
                            .fields()
                            .iter()
                            .map(|field| {
                                EnumSourceFieldV1::new(field.field(), field.value_type().clone())
                            })
                            .collect(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
    );
    fixture.add(
        &enumeration,
        NominalRepresentationShapeV1::Enum { variants },
        Some(public),
    );
    let mut class = SourceFixture::new(SourceNominalKind::Class);
    let field = class.class_field("stored");
    fixture.add(
        &class,
        NominalRepresentationShapeV1::Class {
            base: scoop_identity::OptionalSignatureType::Absent,
            declared_fields: vec![field],
        },
        None,
    );
    let mut object = SourceFixture::new(SourceNominalKind::Object);
    let field = object.class_field("stored");
    let backing_class = object.backing();
    fixture.add(
        &object,
        NominalRepresentationShapeV1::Object {
            backing_class,
            declared_fields: vec![field],
        },
        None,
    );
    fixture.add(
        &SourceFixture::new(SourceNominalKind::Interface),
        NominalRepresentationShapeV1::Interface,
        None,
    );
    let mut boolean = SourceFixture::new(SourceNominalKind::Struct);
    boolean.key = key("Boolean", SourceNominalKind::Struct, 0, vec![]);
    fixture.add(
        &boolean,
        NominalRepresentationShapeV1::Intrinsic {
            representation: NominalIntrinsicRepresentationV1::new(IntrinsicTypeKind::Boolean),
        },
        None,
    );
    fixture
}
