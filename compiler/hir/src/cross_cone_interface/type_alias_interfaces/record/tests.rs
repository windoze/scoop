use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, IdentityReferenceError, NormalizedSourcePath, PackagePath,
    PersistentSourceContextId, PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan, ValidatedIdentityGraph,
};
use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn record_has_fixed_wire_and_exposes_all_fields() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let expected = [
        b"\xa4\x01".as_slice(),
        encode(&record.alias()).unwrap().as_slice(),
        b"\x02".as_slice(),
        encode(record.target()).unwrap().as_slice(),
        b"\x03".as_slice(),
        encode(&record.access()).unwrap().as_slice(),
        b"\x04".as_slice(),
        encode(record.definition_origin()).unwrap().as_slice(),
    ]
    .concat();

    assert_eq!(encode(&record).unwrap(), expected);
    assert_eq!(record.alias(), fixture.alias.id());
    assert_eq!(
        record.target(),
        &TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(fixture.target.id()))
    );
    assert_eq!(record.access(), PublicLookupAccessV1::DirectOnly);
    assert_eq!(record.definition_origin(), &fixture.origin);
}

#[test]
fn decoded_record_resolves_all_typed_references() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let mut authority = fixture.authority();

    assert_eq!(
        decode_record(&record).resolve(&mut authority).unwrap(),
        record
    );
}

#[test]
fn constructor_rejects_slot_access_and_direct_self_reference() {
    let fixture = Fixture::new();
    assert_eq!(
        TypeAliasInterfaceRecordV1::try_new(
            fixture.alias.id(),
            TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(fixture.target.id())),
            PublicLookupAccessV1::PublicSlot,
            fixture.origin.clone(),
        ),
        Err(TypeAliasInterfaceRecordBuildError::DirectAccessRequired {
            alias: fixture.alias.id(),
            actual: PublicLookupAccessV1::PublicSlot,
        })
    );
    assert_eq!(
        TypeAliasInterfaceRecordV1::try_new(
            fixture.alias.id(),
            TypeAliasTargetV1::Alias(fixture.alias.id()),
            PublicLookupAccessV1::DirectOnly,
            fixture.origin.clone(),
        ),
        Err(TypeAliasInterfaceRecordBuildError::DirectSelfReference(
            fixture.alias.id()
        ))
    );
}

#[test]
fn reader_replays_record_invariants_and_exact_map_shape() {
    let fixture = Fixture::new();
    let invalid_access = decode_invalid(&InvalidRecord {
        alias: fixture.alias.id(),
        target: TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(fixture.target.id())),
        access: PublicLookupAccessV1::PublicSlot,
        definition_origin: fixture.origin.clone(),
    });
    let mut authority = fixture.authority();
    assert!(matches!(
        invalid_access.resolve(&mut authority),
        Err(TypeAliasInterfaceRecordResolutionError::Record(
            TypeAliasInterfaceRecordBuildError::DirectAccessRequired { .. }
        ))
    ));

    let self_reference = decode_invalid(&InvalidRecord {
        alias: fixture.alias.id(),
        target: TypeAliasTargetV1::Alias(fixture.alias.id()),
        access: PublicLookupAccessV1::DirectOnly,
        definition_origin: fixture.origin.clone(),
    });
    let mut authority = fixture.authority();
    assert!(matches!(
        self_reference.resolve(&mut authority),
        Err(TypeAliasInterfaceRecordResolutionError::Record(
            TypeAliasInterfaceRecordBuildError::DirectSelfReference(_)
        ))
    ));

    let wrong_shape = decode_canonical::<DecodedTypeAliasInterfaceRecordV1>(&[0xa0]).unwrap_err();
    assert_eq!(
        wrong_shape.kind(),
        &WireErrorKind::InvalidLength {
            expected: 4,
            actual: 0,
        }
    );
}

#[test]
fn reader_reports_missing_alias_identity_authority() {
    let fixture = Fixture::new();
    let mut empty = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();

    assert!(matches!(
        decode_record(&fixture.record()).resolve(&mut empty),
        Err(TypeAliasInterfaceRecordResolutionError::Alias(
            IdentityReferenceError::Missing { .. }
        ))
    ));
}

struct Fixture {
    alias: CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>,
    target: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    context: CborIdentityRecord<PersistentSourceContextId, SourceContextKey>,
    origin: ExportDefinitionSourceV1,
}

impl Fixture {
    fn new() -> Self {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/Alias.scoop").unwrap(),
        )
        .unwrap();
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: source.clone(),
        })
        .unwrap();
        let origin = ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(0, 10).unwrap(), context.key()).unwrap(),
        );
        Self {
            alias: CborIdentityRecord::from_key(SourceDeclarationKey::type_alias(
                site(),
                identifier("Alias"),
            ))
            .unwrap(),
            target: CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
                site(),
                identifier("Target"),
                SourceNominalKind::Struct,
                0,
            ))
            .unwrap(),
            context,
            origin,
        }
    }

    fn record(&self) -> TypeAliasInterfaceRecordV1 {
        TypeAliasInterfaceRecordV1::try_new(
            self.alias.id(),
            TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(self.target.id())),
            PublicLookupAccessV1::DirectOnly,
            self.origin.clone(),
        )
        .unwrap()
    }

    fn authority(&self) -> ValidatedIdentityGraph {
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending
            .register_external_canonical_authority(self.alias.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.target.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.context.clone())
            .unwrap();
        pending.finish().unwrap()
    }
}

struct InvalidRecord {
    alias: PersistentTypeAliasId,
    target: TypeAliasTargetV1,
    access: PublicLookupAccessV1,
    definition_origin: ExportDefinitionSourceV1,
}

impl WireEncode for InvalidRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.alias.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.access.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

fn decode_record(value: &TypeAliasInterfaceRecordV1) -> DecodedTypeAliasInterfaceRecordV1 {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

fn decode_invalid(value: &InvalidRecord) -> DecodedTypeAliasInterfaceRecordV1 {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
