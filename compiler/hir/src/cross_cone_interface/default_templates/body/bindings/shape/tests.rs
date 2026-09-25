use std::num::NonZeroU32;

use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;
use crate::CanonicalBooleanV1;

use super::super::test_support::{LocalError, LocalResolver, Resolver, binder, hex, parameter};

#[test]
fn nested_shape_canonicalizes_fields_and_preserves_component_order() {
    let class = DefaultBindingShapeV1::try_class(
        binder(2),
        vec![
            DefaultBindingClassComponentV1::new(
                nonzero(3),
                DefaultBindingShapeV1::binding(DefaultBindingLeafV1::new(
                    parameter(1),
                    binder(3),
                    CanonicalBooleanV1::True,
                )),
            ),
            DefaultBindingClassComponentV1::new(nonzero(1), DefaultBindingShapeV1::wildcard()),
        ],
    )
    .unwrap();
    let tuple =
        DefaultBindingShapeV1::try_tuple(vec![class, DefaultBindingShapeV1::wildcard()]).unwrap();
    let expected = DefaultBindingShapeV1::try_struct(
        binder(1),
        vec![
            DefaultBindingStructFieldV1::new(
                2,
                DefaultBindingShapeV1::binding(DefaultBindingLeafV1::new(
                    parameter(0),
                    binder(0),
                    CanonicalBooleanV1::False,
                )),
            ),
            DefaultBindingStructFieldV1::new(0, tuple),
        ],
    )
    .unwrap();

    let DefaultBindingShapeViewV1::Struct { fields, .. } = expected.view() else {
        panic!("expected struct binding shape");
    };
    assert_eq!(
        fields
            .iter()
            .map(DefaultBindingStructFieldV1::declaration_index)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    let DefaultBindingShapeViewV1::Tuple(elements) = fields[0].shape().view() else {
        panic!("expected tuple binding shape");
    };
    let DefaultBindingShapeViewV1::Class { components, .. } = elements[0].view() else {
        panic!("expected class binding shape");
    };
    assert_eq!(
        components
            .iter()
            .map(DefaultBindingClassComponentV1::index)
            .collect::<Vec<_>>(),
        vec![nonzero(3), nonzero(1)]
    );

    let mut locals = LocalResolver::new(vec![parameter(0), parameter(1)]);
    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();
    assert_eq!(bytes[2], 4);
    let decoded: DecodedDefaultBindingShapeV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut Resolver, &mut locals), Ok(expected));
}

#[test]
fn wildcard_shape_has_fixed_wire() {
    let mut locals = LocalResolver::new(Vec::new());
    assert_eq!(
        hex(&encode(
            &DefaultBindingShapeV1::wildcard()
                .index_locals(&mut locals)
                .unwrap()
        )
        .unwrap()),
        "a10002"
    );
}

#[test]
fn producer_rejects_duplicate_struct_fields_and_class_components() {
    assert_eq!(
        DefaultBindingShapeV1::try_struct(
            binder(0),
            vec![
                DefaultBindingStructFieldV1::new(1, DefaultBindingShapeV1::wildcard()),
                DefaultBindingStructFieldV1::new(1, DefaultBindingShapeV1::wildcard()),
            ],
        ),
        Err(DefaultBindingShapeBuildError::DuplicateStructField {
            declaration_index: 1,
        })
    );
    assert_eq!(
        DefaultBindingShapeV1::try_class(
            binder(0),
            vec![
                DefaultBindingClassComponentV1::new(nonzero(2), DefaultBindingShapeV1::wildcard(),),
                DefaultBindingClassComponentV1::new(nonzero(2), DefaultBindingShapeV1::wildcard(),),
            ],
        ),
        Err(DefaultBindingShapeBuildError::DuplicateClassComponent { index: nonzero(2) })
    );
}

#[test]
fn reader_rejects_noncanonical_fields_and_duplicate_components() {
    let mut locals = LocalResolver::new(Vec::new());
    let decoded: DecodedDefaultBindingShapeV1 =
        decode_canonical(&encode(&RawStructShape { fields: [2, 1] }).unwrap()).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver, &mut locals),
        Err(DefaultBindingShapeResolutionError::Shape(
            DefaultBindingShapeBuildError::NonCanonicalStructFieldOrder {
                index: 1,
                previous: 2,
                actual: 1,
            }
        ))
    );

    let decoded: DecodedDefaultBindingShapeV1 =
        decode_canonical(&encode(&RawClassShape { components: [2, 2] }).unwrap()).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver, &mut locals),
        Err(DefaultBindingShapeResolutionError::Shape(
            DefaultBindingShapeBuildError::DuplicateClassComponent { index: nonzero(2) }
        ))
    );
}

#[test]
fn shape_indexing_error_preserves_nested_field_location() {
    let expected = DefaultBindingShapeV1::try_struct(
        binder(0),
        vec![DefaultBindingStructFieldV1::new(
            3,
            DefaultBindingShapeV1::binding(DefaultBindingLeafV1::new(
                parameter(4),
                binder(0),
                CanonicalBooleanV1::False,
            )),
        )],
    )
    .unwrap();
    let mut locals = LocalResolver::new(Vec::new());

    assert_eq!(
        expected.index_locals(&mut locals).unwrap_err(),
        DefaultBindingShapeIndexError::StructField {
            index: 0,
            error: Box::new(DefaultBindingShapeIndexError::Binding(
                DefaultBindingLeafIndexError::Local(LocalError::MissingSelector(parameter(4)))
            )),
        }
    );
}

#[test]
fn shape_decoder_rejects_unknown_tags_and_zero_component_indices() {
    let error = decode_canonical::<DecodedDefaultBindingShapeV1>(&[0xa1, 0x00, 0x06]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 6 });

    let error = decode_canonical::<DecodedDefaultBindingClassComponentV1>(&[
        0xa2, 0x01, 0x00, 0x02, 0xa1, 0x00, 0x02,
    ])
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::IntegerOutOfRange);
}

struct RawStructShape {
    fields: [u32; 2],
}

impl WireEncode for RawStructShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_shape_header(encoder, 4)?;
        for index in self.fields {
            encode_shape_entry(encoder, index)?;
        }
        Ok(())
    }
}

struct RawClassShape {
    components: [u32; 2],
}

impl WireEncode for RawClassShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_shape_header(encoder, 5)?;
        for index in self.components {
            encode_shape_entry(encoder, index)?;
        }
        Ok(())
    }
}

fn encode_shape_header(
    encoder: &mut Encoder,
    tag: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    binder(0).encode(encoder)?;
    encoder.field(2)?;
    encoder.array(2)
}

fn encode_shape_entry(
    encoder: &mut Encoder,
    index: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(index))?;
    encoder.field(2)?;
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(2)
}

fn nonzero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).unwrap()
}
