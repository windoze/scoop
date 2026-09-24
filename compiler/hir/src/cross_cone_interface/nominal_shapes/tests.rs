use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, FieldIdentityKey,
    IdentityReferenceError, PackagePath, PendingIdentityValidation, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentObjectValueId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;

mod interior_mutability;
mod intrinsic;
mod policy;

#[test]
fn source_shape_and_variant_style_tags_have_fixed_wire() {
    let empty_struct = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(Vec::new(), crate::NominalCLayoutPolicyV1::Ordinary, false)
            .unwrap(),
    );
    let empty_enum = NominalSourceShapeV1::Enum(EnumSourceShapeV1::try_new(Vec::new()).unwrap());

    assert_eq!(
        encode(&NominalSourceShapeV1::Class(Default::default())).unwrap(),
        [0xa2, 0x00, 0x07, 0x01, 0x80]
    );
    assert_eq!(
        encode(&NominalSourceShapeV1::Interface).unwrap(),
        [0xa1, 0x00, 0x02]
    );
    assert_eq!(
        encode(&empty_struct).unwrap(),
        [
            0xa4, 0x00, 0x03, 0x01, 0x80, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x00
        ]
    );
    assert_eq!(encode(&empty_enum).unwrap(), [0xa2, 0x00, 0x04, 0x01, 0x80]);

    for (style, tag) in [
        (EnumSourceVariantStyleV1::Unit, 1),
        (EnumSourceVariantStyleV1::Positional, 2),
        (EnumSourceVariantStyleV1::Named, 3),
        (EnumSourceVariantStyleV1::Constructor, 4),
    ] {
        assert_eq!(encode(&style).unwrap(), [tag]);
        assert_eq!(
            decode_canonical::<EnumSourceVariantStyleV1>(&[tag], DecodeLimits::default()).unwrap(),
            style
        );
    }
}

#[test]
fn struct_shape_preserves_declaration_order_and_has_fixed_wire() {
    let fixture = fixture();
    let first = struct_field(fixture.struct_first.id(), 0);
    let second = struct_field(fixture.struct_second.id(), 1);
    let shape = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![second.clone(), first.clone()],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false,
        )
        .unwrap(),
    );

    let NominalSourceShapeV1::Struct(structure) = &shape else {
        panic!("constructed a struct source shape");
    };
    assert_eq!(structure.fields(), &[second, first]);
    assert_eq!(
        encode(&shape).unwrap(),
        [
            b"\xa4\x00\x03\x01\x82\xa2\x01\x58\x20".as_slice(),
            fixture.struct_second.id().as_array(),
            b"\x02\xa3\x00\x07\x01\x00\x02\x01\xa2\x01\x58\x20".as_slice(),
            fixture.struct_first.id().as_array(),
            b"\x02\xa3\x00\x07\x01\x00\x02\x00\x02\xa1\x00\x01\x03\x00".as_slice(),
        ]
        .concat()
    );
}

#[test]
fn producer_rejects_duplicate_fields_variants_and_unit_payloads() {
    let fixture = fixture();
    let struct_field = struct_field(fixture.struct_first.id(), 0);
    assert_eq!(
        StructSourceShapeV1::try_new(
            vec![struct_field.clone(), struct_field],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false
        ),
        Err(NominalSourceShapeBuildError::DuplicateNominalField(
            fixture.struct_first.id()
        ))
    );

    let payload_field = enum_field(fixture.enum_first_field.id(), 0);
    assert_eq!(
        EnumSourceVariantV1::try_new(
            fixture.enum_first.id(),
            EnumSourceVariantStyleV1::Unit,
            vec![payload_field.clone()],
        ),
        Err(EnumSourceVariantBuildError::UnitFields)
    );
    assert_eq!(
        EnumSourceVariantV1::try_new(
            fixture.enum_first.id(),
            EnumSourceVariantStyleV1::Positional,
            vec![payload_field.clone(), payload_field],
        ),
        Err(EnumSourceVariantBuildError::DuplicateField(
            fixture.enum_first_field.id()
        ))
    );

    let variant = EnumSourceVariantV1::try_new(
        fixture.enum_first.id(),
        EnumSourceVariantStyleV1::Positional,
        vec![enum_field(fixture.enum_first_field.id(), 0)],
    )
    .unwrap();
    assert_eq!(
        EnumSourceShapeV1::try_new(vec![variant.clone(), variant]),
        Err(NominalSourceShapeBuildError::DuplicateEnumVariant(
            fixture.enum_first.id()
        ))
    );
}

#[test]
fn every_source_shape_round_trips_through_typed_authority() {
    let fixture = fixture();
    let shapes = [
        NominalSourceShapeV1::Class(Default::default()),
        NominalSourceShapeV1::Interface,
        NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(
                vec![
                    struct_field(fixture.struct_first.id(), 0),
                    struct_field(fixture.struct_second.id(), 1),
                ],
                crate::NominalCLayoutPolicyV1::Ordinary,
                false,
            )
            .unwrap(),
        ),
        NominalSourceShapeV1::Enum(
            EnumSourceShapeV1::try_new(vec![
                EnumSourceVariantV1::try_new(
                    fixture.enum_first.id(),
                    EnumSourceVariantStyleV1::Positional,
                    vec![enum_field(fixture.enum_first_field.id(), 0)],
                )
                .unwrap(),
                EnumSourceVariantV1::try_new(
                    fixture.enum_second.id(),
                    EnumSourceVariantStyleV1::Unit,
                    Vec::new(),
                )
                .unwrap(),
            ])
            .unwrap(),
        ),
        NominalSourceShapeV1::Object(ObjectSourceShapeV1::new(
            fixture.object_value.id(),
            Default::default(),
        )),
    ];
    let mut authority = authority(&fixture);

    for expected in shapes {
        let decoded = decode_shape(&expected);
        assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
    }
}

#[test]
fn reader_rejects_duplicate_struct_enum_and_variant_field_ids() {
    let fixture = fixture();

    let field = struct_field(fixture.struct_first.id(), 0);
    let duplicate_struct = InvalidStructShape(vec![field.clone(), field]);
    let mut struct_authority = authority(&fixture);
    assert!(matches!(
        decode_shape(&duplicate_struct).resolve(&mut struct_authority),
        Err(NominalSourceShapeResolutionError::DuplicateNominalField {
            index: 1,
            field,
        }) if field == fixture.struct_first.id()
    ));

    let variant = EnumSourceVariantV1::try_new(
        fixture.enum_second.id(),
        EnumSourceVariantStyleV1::Unit,
        Vec::new(),
    )
    .unwrap();
    let duplicate_enum = InvalidEnumShape(vec![variant.clone(), variant]);
    let mut enum_authority = authority(&fixture);
    assert!(matches!(
        decode_shape(&duplicate_enum).resolve(&mut enum_authority),
        Err(NominalSourceShapeResolutionError::DuplicateEnumVariant {
            index: 1,
            variant,
        }) if variant == fixture.enum_second.id()
    ));

    let field = enum_field(fixture.enum_first_field.id(), 0);
    let duplicate_variant = InvalidEnumVariantShape {
        variant: fixture.enum_first.id(),
        style: EnumSourceVariantStyleV1::Positional,
        fields: vec![field.clone(), field],
    };
    let mut field_authority = authority(&fixture);
    assert!(matches!(
        decode_shape(&duplicate_variant).resolve(&mut field_authority),
        Err(NominalSourceShapeResolutionError::EnumVariant {
            index: 0,
            error: EnumSourceVariantResolutionError::DuplicateField {
                index: 1,
                field,
            },
        }) if field == fixture.enum_first_field.id()
    ));
}

#[test]
fn reader_rejects_missing_typed_authority() {
    let fixture = fixture();
    let shape = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![struct_field(fixture.struct_first.id(), 0)],
            crate::NominalCLayoutPolicyV1::Ordinary,
            false,
        )
        .unwrap(),
    );
    let mut authority = PendingIdentityValidation::new().finish().unwrap();

    assert!(matches!(
        decode_shape(&shape).resolve(&mut authority),
        Err(NominalSourceShapeResolutionError::NominalField {
            index: 0,
            error: NominalSourceFieldResolutionError::Field(IdentityReferenceError::Missing { .. }),
        })
    ));
}

#[test]
fn reader_rejects_unknown_tags_and_wrong_sum_lengths() {
    for retired in [1, 5] {
        let error = decode_canonical::<DecodedNominalSourceShapeV1>(
            &[0xa1, 0x00, retired],
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error.kind(), WireErrorKind::UnknownTag { tag } if *tag == u64::from(retired))
        );
    }
    let unknown_shape = decode_canonical::<DecodedNominalSourceShapeV1>(
        &[0xa1, 0x00, 0x09],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        unknown_shape.kind(),
        WireErrorKind::UnknownTag { tag: 9 }
    ));

    let unknown_style =
        decode_canonical::<EnumSourceVariantStyleV1>(&[0x05], DecodeLimits::default()).unwrap_err();
    assert!(matches!(
        unknown_style.kind(),
        WireErrorKind::UnknownTag { tag: 5 }
    ));

    let wrong_length = decode_canonical::<DecodedNominalSourceShapeV1>(
        &[0xa1, 0x00, 0x07],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        wrong_length.kind(),
        WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    ));
}

struct Fixture {
    struct_first: CborIdentityRecord<PersistentFieldId, FieldIdentityKey>,
    struct_second: CborIdentityRecord<PersistentFieldId, FieldIdentityKey>,
    enum_first: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>,
    enum_first_field: CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>,
    enum_second: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>,
    object_value: CborIdentityRecord<PersistentObjectValueId, SourceDeclarationKey>,
}

fn fixture() -> Fixture {
    let structure = nominal("Pair", SourceNominalKind::Struct, 1);
    let enumeration = nominal("Choice", SourceNominalKind::Enum, 1);
    let object = nominal("Registry", SourceNominalKind::Object, 0);
    let struct_first = CborIdentityRecord::from_key(
        FieldIdentityKey::source_declared(&structure, identifier("first")).unwrap(),
    )
    .unwrap();
    let struct_second = CborIdentityRecord::from_key(
        FieldIdentityKey::source_declared(&structure, identifier("second")).unwrap(),
    )
    .unwrap();
    let enum_first = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(&enumeration, identifier("Present")).unwrap(),
    )
    .unwrap();
    let enum_first_field = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
        enum_first.id(),
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    ))
    .unwrap();
    let enum_second = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(&enumeration, identifier("Absent")).unwrap(),
    )
    .unwrap();
    let object_value = CborIdentityRecord::from_key(object).unwrap();

    Fixture {
        struct_first,
        struct_second,
        enum_first,
        enum_first_field,
        enum_second,
        object_value,
    }
}

fn nominal(name: &str, kind: SourceNominalKind, type_parameter_count: u32) -> SourceDeclarationKey {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    SourceDeclarationKey::nominal(site, identifier(name), kind, type_parameter_count)
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

fn struct_field(field: PersistentFieldId, index: u32) -> NominalSourceFieldV1 {
    NominalSourceFieldV1::new(field, SignatureTypeKey::Binder { depth: 0, index })
}

fn enum_field(field: PersistentEnumVariantFieldId, index: u32) -> EnumSourceFieldV1 {
    EnumSourceFieldV1::new(field, SignatureTypeKey::Binder { depth: 0, index })
}

fn authority(fixture: &Fixture) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_canonical_authority(fixture.struct_first.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.struct_second.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.enum_first.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.enum_first_field.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.enum_second.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.object_value.clone())
        .unwrap();
    pending.finish().unwrap()
}

fn decode_shape<T: WireEncode>(value: &T) -> DecodedNominalSourceShapeV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

struct InvalidStructShape(Vec<NominalSourceFieldV1>);

impl WireEncode for InvalidStructShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(0)?;
        encoder.unsigned(3)?;
        encoder.field(1)?;
        encode_values(encoder, &self.0)?;
        encoder.field(2)?;
        NominalCLayoutPolicyV1::Ordinary.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(0)
    }
}

struct InvalidEnumShape(Vec<EnumSourceVariantV1>);

impl WireEncode for InvalidEnumShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_shape_sequence(encoder, 4, &self.0)
    }
}

struct InvalidEnumVariantShape {
    variant: PersistentEnumVariantId,
    style: EnumSourceVariantStyleV1,
    fields: Vec<EnumSourceFieldV1>,
}

impl WireEncode for InvalidEnumVariantShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(4)?;
        encoder.field(1)?;
        encoder.array(1)?;
        encoder.map(3)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        self.style.encode(encoder)?;
        encoder.field(3)?;
        encode_values(encoder, &self.fields)
    }
}

fn encode_shape_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    tag: u64,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    encode_values(encoder, values)
}

fn encode_values<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}
