use std::sync::Arc;

use scoop_identity::{
    ConeCoordinate, ConeIdentity, DecodedPersistentId, DefinitionOrigin, NormalizedSourcePath,
    PersistentIdMismatch, PersistentIdResolver, PersistentKeyResolver, PersistentSourceContextId,
    SourceContextKey, SourceIdentity, SourceSpan,
};
use scoop_wire::{Encoder, WireEncode, decode_canonical, encode};

use super::*;

#[test]
fn source_reuses_definition_origin_wire_and_resolves_typed_context() {
    let source = source(3, 7);
    assert_eq!(encode(&source).unwrap(), encode(source.origin()).unwrap());

    let decoded: DecodedExportDefinitionSourceV1 =
        decode_canonical(&encode(&source).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), source);
}

#[test]
fn producer_sorts_sources_rejects_duplicates_and_has_stable_wire() {
    let first = source(1, 2);
    let second = source(3, 7);
    let sources =
        CanonicalExportDefinitionSourcesV1::try_new(vec![second.clone(), first.clone()]).unwrap();

    assert_eq!(sources.sources(), &[first.clone(), second.clone()]);
    assert!(sources.contains(&first));
    assert!(!sources.is_empty());
    assert_eq!(
        CanonicalExportDefinitionSourcesV1::try_new(vec![first.clone(), first.clone()]),
        Err(ExportDefinitionSourceSetBuildError::Duplicate(first))
    );
    assert_eq!(
        encode(&sources).unwrap(),
        [
            b"\x82".as_slice(),
            encode(&sources.sources()[0]).unwrap().as_slice(),
            encode(&sources.sources()[1]).unwrap().as_slice(),
        ]
        .concat()
    );
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_source_order() {
    let first = source(1, 2);
    let second = source(3, 7);

    let duplicate = decode_sources(&SourceSequence(vec![first.clone(), first]));
    assert!(matches!(
        duplicate.resolve(&mut Resolver),
        Err(ExportDefinitionSourceSetValidationError::Duplicate { index: 1 })
    ));

    let reversed = decode_sources(&SourceSequence(vec![second, source(1, 2)]));
    assert!(matches!(
        reversed.resolve(&mut Resolver),
        Err(ExportDefinitionSourceSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn reader_reports_unresolved_source_identity() {
    let foreign_cone = ConeCoordinate::new("example", "foreign", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let foreign = SourceIdentity::new(
        foreign_cone,
        NormalizedSourcePath::new("foreign.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: foreign.clone(),
    };
    let origin = DefinitionOrigin::new(foreign, SourceSpan::new(0, 1).unwrap(), &context).unwrap();
    let decoded: DecodedExportDefinitionSourceV1 =
        decode_canonical(&encode(&ExportDefinitionSourceV1::new(origin)).unwrap()).unwrap();

    assert!(decoded.resolve(&mut Resolver).is_err());
}

fn source(start: u64, end: u64) -> ExportDefinitionSourceV1 {
    let source = core_source();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(start, end).unwrap(), &context).unwrap(),
    )
}

fn core_source() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/Alias.scoop").unwrap(),
    )
    .unwrap()
}

fn context_key() -> SourceContextKey {
    SourceContextKey::File {
        source: core_source(),
    }
}

struct Resolver;

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

impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        let key = context_key();
        let expected = PersistentSourceContextId::from_key(&key).unwrap();
        id.verify(expected)
            .map(|_| Arc::new(key))
            .map_err(|_: PersistentIdMismatch<PersistentSourceContextId>| ResolutionError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct SourceSequence(Vec<ExportDefinitionSourceV1>);

impl WireEncode for SourceSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for source in &self.0 {
            source.encode(encoder)?;
        }
        Ok(())
    }
}

fn decode_sources(value: &impl WireEncode) -> DecodedCanonicalExportDefinitionSourcesV1 {
    decode_canonical(&encode(value).unwrap()).unwrap()
}
