use std::sync::Arc;

use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DecodedPersistentId,
    DefinitionOrigin, DefinitionOwnerChain, LocalValueSelector, NormalizedSourcePath, PackagePath,
    PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver, PersistentKeyResolver,
    PersistentSourceContextId, PersistentTypeId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn capture_uses_canonical_local_index_and_round_trips() {
    let capture = DefaultCaptureV1::new(parameter(0), binder(0), origin());
    let mut locals = LocalResolver::new(vec![LocalValueSelector::This, parameter(0)]);
    let bytes = encode(&capture.index_local(&mut locals).unwrap()).unwrap();

    assert_eq!(&bytes[..3], &[0xa3, 0x01, 0x01]);
    let decoded: DecodedDefaultCaptureV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver::without_nominal(), &mut locals),
        Ok(capture)
    );
}

#[test]
fn capture_reports_local_index_and_type_resolution_failures() {
    let missing_local = DefaultCaptureV1::new(parameter(3), binder(0), origin());
    let mut locals = LocalResolver::new(vec![parameter(0)]);
    assert_eq!(
        missing_local.index_local(&mut locals).unwrap_err(),
        DefaultCaptureIndexError::Source(LocalError::MissingSelector(parameter(3)))
    );

    let nominal = nominal("Missing");
    let capture = DefaultCaptureV1::new(parameter(0), SignatureTypeKey::Nominal(nominal), origin());
    let bytes = encode(&capture.index_local(&mut locals).unwrap()).unwrap();
    let decoded: DecodedDefaultCaptureV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver::without_nominal(), &mut locals),
        Err(DefaultCaptureResolutionError::ValueType(ResolutionError))
    );
}

#[test]
fn callable_body_type_argument_modes_have_fixed_wire_and_round_trip() {
    let lexical = DefaultCallableBodyTypeArgumentsV1::lexical();
    assert!(lexical.is_lexical());
    assert_eq!(lexical.explicit_arguments(), None);
    assert_eq!(hex(&encode(&lexical).unwrap()), "a10001");

    let explicit = DefaultCallableBodyTypeArgumentsV1::try_explicit(vec![binder(2)]).unwrap();
    assert!(!explicit.is_lexical());
    assert_eq!(explicit.explicit_arguments(), Some([binder(2)].as_slice()));
    let bytes = encode(&explicit).unwrap();
    assert_eq!(hex(&bytes), "a200020181a3000701000202");

    for expected in [lexical, explicit] {
        let decoded: DecodedDefaultCallableBodyTypeArgumentsV1 =
            decode_canonical(&encode(&expected).unwrap()).unwrap();
        assert_eq!(
            decoded.resolve(&mut Resolver::without_nominal()),
            Ok(expected)
        );
    }
}

#[test]
fn explicit_type_arguments_report_the_failing_position() {
    let allowed = nominal("Allowed");
    let missing = nominal("Missing");
    let arguments = DefaultCallableBodyTypeArgumentsV1::try_explicit(vec![
        SignatureTypeKey::Nominal(allowed),
        SignatureTypeKey::Nominal(missing),
    ])
    .unwrap();
    let decoded: DecodedDefaultCallableBodyTypeArgumentsV1 =
        decode_canonical(&encode(&arguments).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver::new(allowed)),
        Err(DefaultCallableBodyTypeArgumentsResolutionError::Argument {
            index: 1,
            error: ResolutionError,
        })
    );
}

#[test]
fn type_argument_decoder_rejects_unknown_tags_and_non_exact_maps() {
    let error = decode_canonical::<DecodedDefaultCallableBodyTypeArgumentsV1>(&[0xa1, 0x00, 0x03])
        .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error = decode_canonical::<DecodedDefaultCallableBodyTypeArgumentsV1>(&[
        0xa2, 0x00, 0x01, 0x01, 0x80,
    ])
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );
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

struct Resolver {
    nominal: Option<PersistentTypeId>,
}

impl Resolver {
    const fn new(nominal: PersistentTypeId) -> Self {
        Self {
            nominal: Some(nominal),
        }
    }

    const fn without_nominal() -> Self {
        Self { nominal: None }
    }
}

impl PersistentIdResolver<ConeIdentity> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE)
            .map_err(|_: PersistentIdMismatch<ConeIdentity>| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        id.verify(self.nominal.ok_or(ResolutionError)?)
            .map_err(|_: PersistentIdMismatch<PersistentTypeId>| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        _id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err(ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        let key = context_key();
        id.verify(PersistentSourceContextId::from_key(&key).unwrap())
            .map(|_| Arc::new(key))
            .map_err(|_: PersistentIdMismatch<PersistentSourceContextId>| ResolutionError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test resolver")
    }
}

impl std::error::Error for ResolutionError {}

fn parameter(declaration_index: u32) -> LocalValueSelector {
    LocalValueSelector::Parameter { declaration_index }
}

const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

fn nominal(name: &str) -> PersistentTypeId {
    CborIdentityRecord::<PersistentTypeId, _>::from_key(SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
    .id()
}

fn origin() -> ExportDefinitionSourceV1 {
    let source = source_identity();
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(2, 5).unwrap(), &context_key()).unwrap(),
    )
}

fn source_identity() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/Defaults.scoop").unwrap(),
    )
    .unwrap()
}

fn context_key() -> SourceContextKey {
    SourceContextKey::File {
        source: source_identity(),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
