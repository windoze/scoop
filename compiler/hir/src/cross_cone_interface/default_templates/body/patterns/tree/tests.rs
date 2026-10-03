use scoop_identity::{LocalValueSelector, SignatureTypeKey};
use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;
use crate::cross_cone_interface::default_templates::body::expressions::test_support::Fixture;
use crate::{DecodedDefaultLiteralEqualityV1, TemplateLocalSelectorResolver};

#[test]
fn nested_pattern_canonicalizes_fields_and_round_trips_local_indices() {
    let fixture = Fixture::new();
    let literal = DefaultPatternV1::try_literal(
        literal_expression(
            DefaultExpressionKindV1::IntegerLiteral(crate::CanonicalIntegerConstantV1::Signed32(7)),
            &fixture,
        ),
        DefaultLiteralEqualityV1::Integer {
            kind: crate::DefaultIntegerKindV1::Signed32,
        },
        binder(0),
    )
    .unwrap();
    let tuple = DefaultPatternV1::try_tuple(vec![DefaultPatternV1::wildcard(), literal]).unwrap();
    let expected = DefaultPatternV1::try_struct(
        binder(1),
        vec![
            DefaultPatternFieldV1::new(2, DefaultPatternV1::binding(parameter(0))),
            DefaultPatternFieldV1::new(0, tuple),
        ],
    )
    .unwrap();

    let DefaultPatternViewV1::Struct { fields, .. } = expected.view() else {
        panic!("expected struct pattern");
    };
    assert_eq!(
        fields
            .iter()
            .map(DefaultPatternFieldV1::declaration_index)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );

    let mut locals = LocalResolver::new(vec![parameter(0)]);
    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();
    assert_eq!(bytes[2], 6);
    let decoded: DecodedDefaultPatternV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut locals),
        Ok(expected)
    );
}

#[test]
fn variant_pattern_preserves_canonical_field_mapping() {
    let fixture = Fixture::new();
    let expected = DefaultPatternV1::try_variant(
        DefaultEnumVariantRefV1::new(fixture.variant, binder(0)),
        vec![DefaultPatternFieldV1::new(0, DefaultPatternV1::wildcard())],
    )
    .unwrap();
    let mut locals = LocalResolver::new(Vec::new());
    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();
    let decoded: DecodedDefaultPatternV1 = decode_canonical(&bytes).unwrap();

    assert_eq!(bytes[2], 4);
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut locals),
        Ok(expected)
    );
}

#[test]
fn ordinary_literal_equality_round_trips() {
    let fixture = Fixture::new();
    let equality = DefaultLiteralEqualityV1::Ordinary {
        target: fixture.callable(),
    };
    assert_eq!(encode(&equality).unwrap()[2], 2);

    let expected = DefaultPatternV1::try_literal(
        literal_expression(
            DefaultExpressionKindV1::StringLiteral {
                value: "value".to_owned(),
                owner: crate::DefaultStringOwnerV1::CurrentInstantiation,
            },
            &fixture,
        ),
        equality,
        binder(0),
    )
    .unwrap();
    let mut locals = LocalResolver::new(Vec::new());
    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();
    let decoded: DecodedDefaultPatternV1 = decode_canonical(&bytes).unwrap();

    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut locals),
        Ok(expected)
    );
}

#[test]
fn binding_pattern_has_fixed_indexed_wire() {
    let pattern = DefaultPatternV1::binding(parameter(0));
    let mut locals = LocalResolver::new(vec![LocalValueSelector::This, parameter(0)]);

    assert_eq!(
        hex(&encode(&pattern.index_locals(&mut locals).unwrap()).unwrap()),
        "a200010101"
    );
}

#[test]
fn producer_rejects_duplicate_pattern_fields() {
    assert_eq!(
        DefaultPatternV1::try_struct(
            binder(0),
            vec![
                DefaultPatternFieldV1::new(1, DefaultPatternV1::wildcard()),
                DefaultPatternFieldV1::new(1, DefaultPatternV1::wildcard()),
            ],
        ),
        Err(DefaultPatternBuildError::DuplicateField {
            declaration_index: 1
        })
    );
}

#[test]
fn reader_rejects_noncanonical_pattern_field_order() {
    let decoded: DecodedDefaultPatternV1 =
        decode_canonical(&encode(&RawStructPattern { fields: [2, 1] }).unwrap()).unwrap();
    let mut locals = LocalResolver::new(Vec::new());

    assert_eq!(
        decoded.resolve(&mut Fixture::new().resolver(), &mut locals),
        Err(DefaultPatternResolutionError::Shape(
            DefaultPatternBuildError::NonCanonicalFieldOrder {
                index: 1,
                previous: 2,
                actual: 1,
            }
        ))
    );
}

#[test]
fn reader_rejects_duplicate_pattern_fields() {
    let decoded: DecodedDefaultPatternV1 =
        decode_canonical(&encode(&RawStructPattern { fields: [1, 1] }).unwrap()).unwrap();
    let mut locals = LocalResolver::new(Vec::new());

    assert_eq!(
        decoded.resolve(&mut Fixture::new().resolver(), &mut locals),
        Err(DefaultPatternResolutionError::Shape(
            DefaultPatternBuildError::DuplicateField {
                declaration_index: 1,
            }
        ))
    );
}

#[test]
fn indexing_error_preserves_nested_element_location() {
    let pattern = DefaultPatternV1::try_tuple(vec![
        DefaultPatternV1::wildcard(),
        DefaultPatternV1::binding(parameter(2)),
    ])
    .unwrap();
    let mut locals = LocalResolver::new(Vec::new());

    assert_eq!(
        pattern.index_locals(&mut locals).unwrap_err(),
        DefaultPatternIndexError::Element {
            index: 1,
            error: Box::new(DefaultPatternIndexError::Local(
                LocalError::MissingSelector(parameter(2))
            )),
        }
    );
}

#[test]
fn pattern_decoder_rejects_unknown_tags_and_variant_shapes() {
    let error = decode_canonical::<DecodedDefaultPatternV1>(&[0xa1, 0x00, 0x07]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 7 });

    let error =
        decode_canonical::<DecodedDefaultPatternV1>(&[0xa2, 0x00, 0x02, 0x01, 0x00]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );

    let error =
        decode_canonical::<DecodedDefaultLiteralEqualityV1>(&[0xa1, 0x00, 0x03]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

struct RawStructPattern {
    fields: [u32; 2],
}

impl WireEncode for RawStructPattern {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(6)?;
        encoder.field(1)?;
        binder(0).encode(encoder)?;
        encoder.field(2)?;
        encoder.array(2)?;
        encode_wildcard_field(encoder, self.fields[0])?;
        encode_wildcard_field(encoder, self.fields[1])
    }
}

fn encode_wildcard_field(
    encoder: &mut Encoder,
    declaration_index: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(declaration_index))?;
    encoder.field(2)?;
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(2)
}

struct LocalResolver {
    selectors: Vec<LocalValueSelector>,
}

impl LocalResolver {
    const fn new(selectors: Vec<LocalValueSelector>) -> Self {
        Self { selectors }
    }
}

impl TemplateLocalSelectorResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_selector(
        &mut self,
        index: u32,
    ) -> Result<LocalValueSelector, Self::Error> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.selectors.get(index))
            .cloned()
            .ok_or(LocalError::MissingIndex(index))
    }
}

impl TemplateLocalIndexResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        self.selectors
            .iter()
            .position(|candidate| candidate == selector)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| LocalError::MissingSelector(selector.clone()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LocalError {
    MissingIndex(u32),
    MissingSelector(LocalValueSelector),
}

impl std::fmt::Display for LocalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LocalError {}

fn parameter(declaration_index: u32) -> LocalValueSelector {
    LocalValueSelector::Parameter { declaration_index }
}

const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn literal_expression(kind: DefaultExpressionKindV1, fixture: &Fixture) -> DefaultExpressionV1 {
    use scoop_identity::{EvaluationOrigin, SourceContextKey, SourceSpan};
    let definition = fixture.origin();
    let source = definition.origin().source().clone();
    let evaluation = EvaluationOrigin::new(
        source.clone(),
        SourceSpan::new(11, 16).unwrap(),
        &SourceContextKey::File { source },
    )
    .unwrap();
    assert_ne!(evaluation.span(), definition.origin().span());
    DefaultExpressionV1::try_new(kind, fixture.value_type(), definition, evaluation).unwrap()
}

#[test]
fn literal_pattern_requires_a_literal_expression() {
    let fixture = Fixture::new();
    assert_eq!(
        DefaultPatternV1::try_literal(
            literal_expression(DefaultExpressionKindV1::UnitLiteral, &fixture),
            DefaultLiteralEqualityV1::Ordinary {
                target: fixture.callable()
            },
            binder(0),
        ),
        Err(DefaultPatternBuildError::InvalidLiteralExpression),
    );
}

#[test]
fn reader_rejects_retired_constant_only_literal_payload() {
    let bytes = encode(&RetiredLiteralPattern).unwrap();
    let error = decode_canonical::<DecodedDefaultPatternV1>(&bytes).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 4,
            actual: 2
        }
    );
}

struct RetiredLiteralPattern;

impl WireEncode for RetiredLiteralPattern {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_literal(
            encoder,
            &crate::CanonicalConstValueV1::Boolean(crate::CanonicalBooleanV1::True),
            &DefaultLiteralEqualityV1::Ordinary {
                target: Fixture::new().callable(),
            },
            &binder(0),
        )
    }
}
