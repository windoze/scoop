use std::sync::Arc;

use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DecodedPersistentId,
    DefinitionOrigin, DefinitionOwnerChain, LocalValueSelector, NormalizedSourcePath, PackagePath,
    PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver, PersistentKeyResolver,
    PersistentSourceContextId, PersistentTypeId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
    SyntheticLocalRole,
};
use scoop_wire::{
    DecodeLimits, Encoder, WireEncode, WireErrorKind, WireType, decode_canonical, encode,
};

use super::*;

#[test]
fn synthetic_record_has_fixed_wire() {
    let record = synthetic_local(0);
    let table = CanonicalTemplateLocalTableV1::try_new(vec![record]).unwrap();

    assert_eq!(
        hex(&encode(&table).unwrap()),
        "81a401a300060181a2010b0200020102a3000701000200030104a10002"
    );
}

#[test]
fn producer_sorts_records_and_supports_typed_lookup() {
    let this = source_local(LocalValueSelector::This, binder());
    let parameter = source_local(
        LocalValueSelector::Parameter {
            declaration_index: 0,
        },
        binder(),
    );
    let table =
        CanonicalTemplateLocalTableV1::try_new(vec![parameter.clone(), this.clone()]).unwrap();

    assert_eq!(table.len_u32(), 2);
    assert!(!table.is_empty());
    assert_eq!(table.records(), &[this.clone(), parameter]);
    assert_eq!(table.get(this.selector()), Some(&this));
    assert_eq!(table.index_of(this.selector()), Some(0));
    assert_eq!(
        table.index_of(&LocalValueSelector::Parameter {
            declaration_index: 9
        }),
        None
    );
}

#[test]
fn producer_rejects_duplicate_and_invalid_definition_shapes() {
    let this = source_local(LocalValueSelector::This, binder());
    assert_eq!(
        CanonicalTemplateLocalTableV1::try_new(vec![this.clone(), this]),
        Err(TemplateLocalTableBuildError::Duplicate(
            LocalValueSelector::This
        ))
    );

    let synthetic = synthetic_selector(0);
    assert!(matches!(
        TemplateLocalRecordV1::try_new(
            synthetic.clone(),
            binder(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(origin()),
        ),
        Err(TemplateLocalRecordBuildError::SyntheticDefinitionRequired(selector))
            if selector == synthetic
    ));
    assert_eq!(
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::This,
            binder(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Synthetic,
        ),
        Err(TemplateLocalRecordBuildError::SourceDefinitionRequired(
            LocalValueSelector::This
        ))
    );
    let suspension = LocalValueSelector::SuspensionResult { site: path(1) };
    assert_eq!(
        TemplateLocalRecordV1::try_new(
            suspension.clone(),
            binder(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Synthetic,
        ),
        Err(TemplateLocalRecordBuildError::UnsupportedSelector(
            suspension
        ))
    );
}

#[test]
fn decoded_table_resolves_types_and_definition_origins() {
    let nominal = nominal("Token");
    let expected = CanonicalTemplateLocalTableV1::try_new(vec![source_local(
        LocalValueSelector::LocalDeclaration { path: path(2) },
        SignatureTypeKey::Nominal(nominal.id()),
    )])
    .unwrap();
    let decoded = decode_table(&expected);

    assert_eq!(
        decoded.resolve(&mut Resolver::new(nominal.id())).unwrap(),
        expected
    );
}

#[test]
fn reader_rejects_duplicate_noncanonical_and_invalid_records() {
    let this = source_local(LocalValueSelector::This, binder());
    let parameter = source_local(
        LocalValueSelector::Parameter {
            declaration_index: 0,
        },
        binder(),
    );

    let duplicate = decode_table(&LocalSequence(vec![this.clone(), this]));
    assert!(matches!(
        duplicate.resolve(&mut Resolver::without_nominal()),
        Err(TemplateLocalTableValidationError::Duplicate { index: 1, .. })
    ));

    let reversed = decode_table(&LocalSequence(vec![
        parameter,
        source_local(LocalValueSelector::This, binder()),
    ]));
    assert!(matches!(
        reversed.resolve(&mut Resolver::without_nominal()),
        Err(TemplateLocalTableValidationError::NonCanonicalOrder { index: 1 })
    ));

    let invalid = decode_table(&InvalidSyntheticSourceRecord);
    assert!(matches!(
        invalid.resolve(&mut Resolver::without_nominal()),
        Err(TemplateLocalTableValidationError::Record {
            index: 0,
            error: TemplateLocalRecordResolutionError::Shape(
                TemplateLocalRecordBuildError::SyntheticDefinitionRequired(_)
            ),
        })
    ));
}

#[test]
fn definition_decoder_rejects_unknown_tags_and_native_booleans() {
    let unknown = decode_canonical::<DecodedTemplateLocalDefinitionV1>(
        &[0xa1, 0x00, 0x03],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error = decode_canonical::<DecodedTemplateLocalRecordV1>(
        &hex_bytes("a401a300060181a2010b0200020102a300070100020003f404a10002"),
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::WrongType {
            expected: WireType::Unsigned,
        }
    );
}

fn source_local(
    selector: LocalValueSelector,
    value_type: SignatureTypeKey,
) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        selector,
        value_type,
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Source(origin()),
    )
    .unwrap()
}

fn synthetic_local(ordinal: u32) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        synthetic_selector(ordinal),
        binder(),
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Synthetic,
    )
    .unwrap()
}

fn synthetic_selector(ordinal: u32) -> LocalValueSelector {
    LocalValueSelector::Synthetic {
        path: path(ordinal),
        role: SyntheticLocalRole::Temporary,
    }
}

fn path(ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::SyntheticValue, ordinal),
        [],
    )
}

fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
}

fn origin() -> ExportDefinitionSourceV1 {
    let source = source_identity();
    let context = context_key();
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(2, 5).unwrap(), &context).unwrap(),
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

fn nominal(name: &str) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
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
}

struct Resolver {
    nominal: Option<PersistentTypeId>,
}

impl Resolver {
    fn new(nominal: PersistentTypeId) -> Self {
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
        let expected = self.nominal.ok_or(ResolutionError)?;
        id.verify(expected)
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
        formatter.write_str("identity is absent from the test resolver")
    }
}

impl std::error::Error for ResolutionError {}

struct LocalSequence(Vec<TemplateLocalRecordV1>);

impl WireEncode for LocalSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

struct InvalidSyntheticSourceRecord;

impl WireEncode for InvalidSyntheticSourceRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(1)?;
        encoder.map(4)?;
        encoder.field(1)?;
        synthetic_selector(0).encode(encoder)?;
        encoder.field(2)?;
        binder().encode(encoder)?;
        encoder.field(3)?;
        CanonicalBooleanV1::False.encode(encoder)?;
        encoder.field(4)?;
        TemplateLocalDefinitionV1::Source(origin()).encode(encoder)
    }
}

fn decode_table(value: &impl WireEncode) -> DecodedCanonicalTemplateLocalTableV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

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
