use super::*;

pub(super) const SOURCE_PATH: &str = "src/Metered.scoop";
pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
pub(super) fn source_identity(cone: ConeIdentity, path: &str) -> SourceIdentity {
    SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap()
}
pub(super) fn source(start: u64) -> ExportDefinitionSourceV1 {
    let source = source_identity(ConeIdentity::CORE, SOURCE_PATH);
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(start, start + 1).unwrap(), &context)
            .unwrap(),
    )
}
pub(super) struct Foundation {
    pub context: Arc<SourceContextKey>,
    pub cone_calls: usize,
    pub context_calls: usize,
}
impl Foundation {
    pub fn new(path: &str) -> Self {
        Self {
            context: Arc::new(SourceContextKey::File {
                source: source_identity(ConeIdentity::CORE, path),
            }),
            cone_calls: 0,
            context_calls: 0,
        }
    }
}
impl Default for Foundation {
    fn default() -> Self {
        Self::new(SOURCE_PATH)
    }
}
impl PersistentIdResolver<ConeIdentity> for Foundation {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        self.cone_calls += 1;
        id.verify(ConeIdentity::CORE)
            .map_err(|_| "unknown foundation cone")
    }
}
impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Foundation {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        self.context_calls += 1;
        id.verify(PersistentSourceContextId::from_key(&self.context).unwrap())
            .map_err(|_| "unknown foundation source context")?;
        Ok(Arc::clone(&self.context))
    }
}
pub(super) struct Sequence(pub Vec<ExportDefinitionSourceV1>);
impl WireEncode for Sequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for source in &self.0 {
            source.encode(encoder)?;
        }
        Ok(())
    }
}
pub(super) fn decoded(value: &impl WireEncode) -> DecodedCanonicalExportDefinitionSourcesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}
pub(super) fn one() -> DecodedCanonicalExportDefinitionSourcesV1 {
    decoded(&Sequence(vec![source(1)]))
}
pub(super) fn resource(
    error: MeteredDefinitionSourcesResolutionError<&'static str>,
    expected: ResourceKind,
    path: &WirePath,
) {
    let MeteredDefinitionSourcesResolutionError::Resource(error) = error else {
        panic!("expected resolution resource failure: {error:?}");
    };
    assert!(
        matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected),
        "{error:?}"
    );
    assert_eq!(error.path(), path);
    assert_eq!(error.byte_offset(), None);
}
