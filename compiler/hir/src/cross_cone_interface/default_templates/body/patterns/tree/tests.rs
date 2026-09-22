use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOwnerChain,
    EnumVariantIdentityKey, LocalValueSelector, OptionalSignatureType, PackagePath,
    PersistentConstructorId, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentFieldId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentIdResolver,
    PersistentPropertyAccessorId, PersistentTypeId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;
use crate::{DecodedDefaultLiteralEqualityV1, DefaultCallableRefV1, TemplateLocalSelectorResolver};

#[test]
fn nested_pattern_canonicalizes_fields_and_round_trips_local_indices() {
    let fixture = Fixture::new();
    let literal = DefaultPatternV1::literal(
        CanonicalConstValueV1::Integer(crate::CanonicalIntegerConstantV1::Signed32(7)),
        DefaultLiteralEqualityV1::Integer {
            kind: crate::DefaultIntegerKindV1::Signed32,
        },
        binder(0),
    );
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
    let decoded: DecodedDefaultPatternV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
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
    let decoded: DecodedDefaultPatternV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();

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

    let expected = DefaultPatternV1::literal(
        CanonicalConstValueV1::String("value".to_owned()),
        equality,
        binder(0),
    );
    let mut locals = LocalResolver::new(Vec::new());
    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();
    let decoded: DecodedDefaultPatternV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();

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
    let decoded: DecodedDefaultPatternV1 = decode_canonical(
        &encode(&RawStructPattern { fields: [2, 1] }).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
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
    let decoded: DecodedDefaultPatternV1 = decode_canonical(
        &encode(&RawStructPattern { fields: [1, 1] }).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
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
    let error =
        decode_canonical::<DecodedDefaultPatternV1>(&[0xa1, 0x00, 0x07], DecodeLimits::default())
            .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 7 });

    let error = decode_canonical::<DecodedDefaultPatternV1>(
        &[0xa2, 0x00, 0x02, 0x01, 0x00],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );

    let error = decode_canonical::<DecodedDefaultLiteralEqualityV1>(
        &[0xa1, 0x00, 0x03],
        DecodeLimits::default(),
    )
    .unwrap_err();
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

struct Fixture {
    function: PersistentFunctionId,
    variant: PersistentEnumVariantId,
}

impl Fixture {
    fn new() -> Self {
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                top_level_site(),
                identifier("equals"),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let enumeration = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Choice"),
            SourceNominalKind::Enum,
            0,
        );
        let variant = PersistentEnumVariantId::from_key(
            &EnumVariantIdentityKey::source(&enumeration, identifier("Only")).unwrap(),
        )
        .unwrap();
        Self { function, variant }
    }

    fn callable(&self) -> DefaultCallableRefV1 {
        DefaultCallableRefV1::try_new(
            crate::DefaultCallableDeclarationV1::Function(self.function),
            OptionalSignatureType::Absent,
            Vec::new(),
        )
        .unwrap()
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            function: self.function,
            variant: self.variant,
        }
    }
}

struct Resolver {
    function: PersistentFunctionId,
    variant: PersistentEnumVariantId,
}

macro_rules! resolve_fixture_identity {
    ($identity:ty, $field:ident) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                id.verify(self.$field).map_err(|_| ResolutionError)
            }
        }
    };
}

macro_rules! reject_identity {
    ($identity:ty) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                _id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                Err(ResolutionError)
            }
        }
    };
}

resolve_fixture_identity!(PersistentFunctionId, function);
resolve_fixture_identity!(PersistentEnumVariantId, variant);
reject_identity!(PersistentGenericFunctionId);
reject_identity!(PersistentConstructorId);
reject_identity!(PersistentPropertyAccessorId);
reject_identity!(PersistentGeneratedCallableId);
reject_identity!(PersistentTypeId);
reject_identity!(PersistentGenericTypeId);
reject_identity!(PersistentEnumVariantFieldId);
reject_identity!(PersistentFieldId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent")
    }
}

impl std::error::Error for ResolutionError {}

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

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

fn parameter(declaration_index: u32) -> LocalValueSelector {
    LocalValueSelector::Parameter { declaration_index }
}

const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
