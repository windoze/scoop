use scoop_identity::{
    DecodedPersistentId, LocalValueSelector, PersistentGenericTypeId, PersistentIdResolver,
    PersistentTypeId, SignatureTypeKey,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn receiver_requires_this_selector() {
    assert_eq!(
        TemplateReceiverV1::try_new(
            LocalValueSelector::Parameter {
                declaration_index: 0,
            },
            binder(),
        ),
        Err(TemplateReceiverBuildError::ExpectedThis(
            LocalValueSelector::Parameter {
                declaration_index: 0,
            }
        ))
    );
}

#[test]
fn optional_receiver_has_fixed_indexed_wire_and_resolves() {
    let expected = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, binder()).unwrap(),
    );
    let mut locals = LocalResolver::new(vec![
        LocalValueSelector::This,
        LocalValueSelector::Parameter {
            declaration_index: 0,
        },
    ]);

    let bytes = encode(&expected.index_local(&mut locals).unwrap()).unwrap();
    assert_eq!(hex(&bytes), "a2000201a2010002a3000701000200");

    let decoded: DecodedOptionalTemplateReceiverV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut TypeResolver, &mut locals).unwrap(),
        expected
    );

    assert_eq!(
        encode(
            &OptionalTemplateReceiverV1::Absent
                .index_local(&mut locals)
                .unwrap()
        )
        .unwrap(),
        vec![0xa1, 0x00, 0x01]
    );
}

#[test]
fn receiver_resolution_and_indexing_report_local_failures() {
    let decoded: DecodedOptionalTemplateReceiverV1 =
        decode_canonical(&hex_bytes("a2000201a2010002a3000701000200")).unwrap();
    let mut wrong_local = LocalResolver::new(vec![LocalValueSelector::Parameter {
        declaration_index: 0,
    }]);
    assert_eq!(
        decoded.resolve(&mut TypeResolver, &mut wrong_local),
        Err(TemplateReceiverResolutionError::Record(
            TemplateReceiverBuildError::ExpectedThis(LocalValueSelector::Parameter {
                declaration_index: 0,
            })
        ))
    );

    let receiver = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, binder()).unwrap(),
    );
    let mut missing = LocalResolver::new(Vec::new());
    assert_eq!(
        receiver.index_local(&mut missing).unwrap_err(),
        TemplateReceiverIndexError::Local(LocalError::MissingSelector(LocalValueSelector::This))
    );
}

#[test]
fn optional_receiver_decoder_rejects_unknown_tags_and_wrong_shapes() {
    let unknown =
        decode_canonical::<DecodedOptionalTemplateReceiverV1>(&[0xa1, 0x00, 0x03]).unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let wrong_shape =
        decode_canonical::<DecodedOptionalTemplateReceiverV1>(&[0xa2, 0x00, 0x01, 0x01, 0x00])
            .unwrap_err();
    assert_eq!(
        wrong_shape.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );
}

fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
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

struct TypeResolver;

impl PersistentIdResolver<PersistentTypeId> for TypeResolver {
    type Error = TypeResolutionError;

    fn resolve(
        &mut self,
        _id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        Err(TypeResolutionError)
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for TypeResolver {
    type Error = TypeResolutionError;

    fn resolve(
        &mut self,
        _id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err(TypeResolutionError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TypeResolutionError;

impl std::fmt::Display for TypeResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("type is absent")
    }
}

impl std::error::Error for TypeResolutionError {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_bytes(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let digits = std::str::from_utf8(digits).unwrap();
            u8::from_str_radix(digits, 16).unwrap()
        })
        .collect()
}
