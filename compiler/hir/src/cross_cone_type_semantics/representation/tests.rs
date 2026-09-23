use scoop_identity::SourceNominalKind;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{
    EnumSourceFieldV1, EnumSourceShapeV1, EnumSourceVariantStyleV1, EnumSourceVariantV1,
    HirCLayoutContract, HirCLayoutValue, IntrinsicTypeKind, NominalSourceFieldV1,
    StructSourceShapeV1,
};

pub(in crate::cross_cone_type_semantics::representation) mod support;
use support::{Fixture, source_key, unit};

mod source_policy;

fn round_trip(
    fixture: &mut Fixture,
    shape: NominalRepresentationShapeV1,
) -> NominalRepresentationSupportV1 {
    let record =
        NominalRepresentationSupportV1::try_new(&fixture.key, fixture.access.clone(), shape)
            .unwrap();
    let bytes = encode(&record).unwrap();
    let decoded: DecodedNominalRepresentationSupportV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(fixture).unwrap(), record);
    record
}

#[test]
fn struct_support_preserves_declaration_order_and_matches_public_shape() {
    let mut fixture = Fixture::new(SourceNominalKind::Struct);
    let first = fixture.struct_field("first", unit());
    let second = fixture.struct_field("second", unit());
    let fields = vec![second.clone(), first.clone()];
    let record = round_trip(
        &mut fixture,
        NominalRepresentationShapeV1::Struct {
            fields,
            c_layout_policy: NominalCLayoutPolicyV1::Ordinary,
        },
    );
    let public = |fields: &[StructRepresentationFieldV1]| {
        NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(
                fields
                    .iter()
                    .map(|field| {
                        NominalSourceFieldV1::new(field.field(), field.value_type().clone())
                    })
                    .collect(),
                crate::NominalCLayoutPolicyV1::Ordinary,
            )
            .unwrap(),
        )
    };
    record
        .validate_public_source_shape(&public(&[second.clone(), first.clone()]))
        .unwrap();
    assert!(
        record
            .validate_public_source_shape(&public(&[first, second]))
            .is_err()
    );
    let NominalRepresentationShapeV1::Struct { fields, .. } = record.shape() else {
        panic!("expected struct representation")
    };
    assert_eq!(
        fields[0].field(),
        fixture.struct_field("second", unit()).field()
    );
}

#[test]
fn enum_support_keeps_variant_order_gc_and_typed_payload_identity() {
    let mut fixture = Fixture::new(SourceNominalKind::Enum);
    let payload = fixture.variant("Payload", true);
    let empty = fixture.variant("Empty", false);
    let record = round_trip(
        &mut fixture,
        NominalRepresentationShapeV1::Enum {
            variants: vec![payload.clone(), empty.clone()],
        },
    );
    let variant = |variant: &EnumRepresentationVariantV1, style| {
        EnumSourceVariantV1::try_new(
            variant.variant(),
            style,
            variant
                .fields()
                .iter()
                .map(|field| EnumSourceFieldV1::new(field.field(), field.value_type().clone()))
                .collect(),
        )
        .unwrap()
    };
    let source = NominalSourceShapeV1::Enum(
        EnumSourceShapeV1::try_new(vec![
            variant(&payload, EnumSourceVariantStyleV1::Positional),
            variant(&empty, EnumSourceVariantStyleV1::Unit),
        ])
        .unwrap(),
    );
    record.validate_public_source_shape(&source).unwrap();
    assert_eq!(payload.gc(), ExactTypeGcV1::GcFree);
}

#[test]
fn class_object_interface_and_intrinsic_shapes_round_trip_with_complete_fields() {
    let mut class = Fixture::new(SourceNominalKind::Class);
    let field = class.class_field("privateValue");
    let source = NominalSourceShapeV1::Class(
        crate::NominalSourceFieldsV1::try_new(vec![crate::NominalSourceFieldV1::new(
            field.field(),
            field.value_type().clone(),
        )])
        .unwrap(),
    );
    let record = round_trip(
        &mut class,
        NominalRepresentationShapeV1::Class {
            base: OptionalSignatureType::Absent,
            declared_fields: vec![field],
        },
    );
    record.validate_public_source_shape(&source).unwrap();
    let mut object = Fixture::new(SourceNominalKind::Object);
    let field = object.class_field("storedValue");
    let backing = object.backing();
    assert_ne!(backing, object.owner());
    round_trip(
        &mut object,
        NominalRepresentationShapeV1::Object {
            backing_class: backing,
            declared_fields: vec![field],
        },
    );
    round_trip(
        &mut Fixture::new(SourceNominalKind::Interface),
        NominalRepresentationShapeV1::Interface,
    );
    round_trip(
        &mut Fixture::new(SourceNominalKind::Struct),
        NominalRepresentationShapeV1::Intrinsic {
            representation: NominalIntrinsicRepresentationV1::new(IntrinsicTypeKind::Boolean),
        },
    );
}

#[test]
fn support_rejects_templates_binders_duplicate_fields_and_wrong_source_kind() {
    let mut fixture = Fixture::new(SourceNominalKind::Struct);
    let build =
        |key, shape| NominalRepresentationSupportV1::try_new(key, fixture.access.clone(), shape);
    let generic = source_key(SourceNominalKind::Struct, 1);
    assert!(matches!(
        build(
            &generic,
            NominalRepresentationShapeV1::Struct {
                fields: vec![],
                c_layout_policy: NominalCLayoutPolicyV1::Ordinary
            }
        ),
        Err(NominalRepresentationBuildError::GenericTemplate)
    ));
    assert!(matches!(
        build(&fixture.key, NominalRepresentationShapeV1::Interface),
        Err(NominalRepresentationBuildError::SourceKind)
    ));
    let field = fixture.struct_field("field", unit());
    assert!(matches!(
        NominalRepresentationSupportV1::try_new(
            &fixture.key,
            fixture.access.clone(),
            NominalRepresentationShapeV1::Struct {
                fields: vec![field.clone(), field],
                c_layout_policy: NominalCLayoutPolicyV1::Ordinary
            }
        ),
        Err(NominalRepresentationBuildError::DuplicateField { .. })
    ));
    let field = fixture.struct_field("dependent", SignatureTypeKey::Binder { depth: 0, index: 0 });
    assert!(matches!(
        NominalRepresentationSupportV1::try_new(
            &fixture.key,
            fixture.access.clone(),
            NominalRepresentationShapeV1::Struct {
                fields: vec![field],
                c_layout_policy: NominalCLayoutPolicyV1::Ordinary
            }
        ),
        Err(NominalRepresentationBuildError::Binder(_))
    ));
}

#[test]
fn malformed_c_layout_base_and_object_backing_are_rejected() {
    let structure = Fixture::new(SourceNominalKind::Struct);
    let shape = NominalRepresentationShapeV1::Struct {
        fields: vec![],
        c_layout_policy: NominalCLayoutPolicyV1::CLayout {
            contract: HirCLayoutContract {
                aligned: HirCLayoutValue::Natural,
                packed: HirCLayoutValue::Natural,
            },
        },
    };
    assert!(matches!(
        NominalRepresentationSupportV1::try_new(&structure.key, structure.access, shape),
        Err(NominalRepresentationBuildError::EmptyCLayout)
    ));
    let class = Fixture::new(SourceNominalKind::Class);
    assert!(matches!(
        NominalRepresentationSupportV1::try_new(
            &class.key,
            class.access.clone(),
            NominalRepresentationShapeV1::Intrinsic {
                representation: NominalIntrinsicRepresentationV1::new(IntrinsicTypeKind::Array)
            }
        ),
        Err(NominalRepresentationBuildError::GenericIntrinsicFamily)
    ));
    let shape = NominalRepresentationShapeV1::Class {
        base: OptionalSignatureType::Present(Box::new(SignatureTypeKey::RawPointer(Box::new(
            unit(),
        )))),
        declared_fields: vec![],
    };
    assert!(matches!(
        NominalRepresentationSupportV1::try_new(&class.key, class.access, shape),
        Err(NominalRepresentationBuildError::NonNominalBase)
    ));
    let object = Fixture::new(SourceNominalKind::Object);
    let shape = NominalRepresentationShapeV1::Object {
        backing_class: object.owner(),
        declared_fields: vec![],
    };
    assert!(matches!(
        NominalRepresentationSupportV1::try_new(&object.key, object.access, shape),
        Err(NominalRepresentationBuildError::ObjectBackingClass)
    ));
}

#[test]
fn shape_wire_has_fixed_tags_and_rejects_unknown_or_wrong_products() {
    assert_eq!(
        encode(&NominalRepresentationShapeV1::Struct {
            fields: vec![],
            c_layout_policy: NominalCLayoutPolicyV1::Ordinary
        })
        .unwrap(),
        [0xa3, 0, 1, 1, 0x80, 2, 0xa1, 0, 1]
    );
    assert_eq!(
        encode(&NominalRepresentationShapeV1::Enum { variants: vec![] }).unwrap(),
        [0xa2, 0, 2, 1, 0x80]
    );
    assert_eq!(
        encode(&NominalRepresentationShapeV1::Interface).unwrap(),
        [0xa1, 0, 4]
    );
    for bytes in [
        &[0xa1, 0, 7][..],
        &[0xa2, 0, 4, 1, 0][..],
        &[0xa1, 0, 1][..],
    ] {
        assert!(
            decode_canonical::<DecodedNominalRepresentationShapeV1>(bytes, DecodeLimits::default())
                .is_err()
        );
    }
}
