use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, NormalizedSourcePath, PackagePath, PersistentGenericTypeId,
    PersistentSourceContextId, PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::{
    ExportDefinitionSourceV1, NominalInterfaceShapeAuthority, PublicLookupAccessV1,
    PublicNominalKindV1, PublicNominalShapeV1, TypeAliasDeclarationSourceV1,
    TypeAliasInterfaceSemanticAuthority, TypeAliasTargetV1,
};

#[test]
fn producer_sorts_records_rejects_duplicates_and_has_stable_wire() {
    let fixture = Fixture::new();
    let interfaces = CanonicalTypeAliasInterfacesV1::try_new(vec![
        fixture.second.clone(),
        fixture.first.clone(),
    ])
    .unwrap();

    assert!(interfaces.records()[0].alias() < interfaces.records()[1].alias());
    assert_eq!(interfaces.get(fixture.first.alias()), Some(&fixture.first));
    assert!(!interfaces.is_empty());
    assert_eq!(
        CanonicalTypeAliasInterfacesV1::try_new(
            vec![fixture.first.clone(), fixture.first.clone(),]
        ),
        Err(TypeAliasInterfaceSetBuildError::DuplicateAlias(
            fixture.first.alias()
        ))
    );
    assert_eq!(
        encode(&interfaces).unwrap(),
        [
            b"\x82".as_slice(),
            encode(&interfaces.records()[0]).unwrap().as_slice(),
            encode(&interfaces.records()[1]).unwrap().as_slice(),
        ]
        .concat()
    );
}

#[test]
fn decoded_table_resolves_records_through_typed_authority() {
    let fixture = Fixture::new();
    let expected = fixture.interfaces();
    let decoded = decode_table(&expected);
    let mut authority = fixture.identity_authority();

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_record_order() {
    let fixture = Fixture::new();
    let (low, high) = ordered(&fixture);

    let duplicate = decode_table(&RecordSequence(vec![low.clone(), low.clone()]));
    let mut authority = fixture.identity_authority();
    assert!(matches!(
        duplicate.resolve(&mut authority),
        Err(TypeAliasInterfaceSetValidationError::DuplicateAlias {
            index: 1,
            alias,
        }) if alias == low.alias()
    ));

    let reversed = decode_table(&RecordSequence(vec![high.clone(), low.clone()]));
    let mut authority = fixture.identity_authority();
    assert!(matches!(
        reversed.resolve(&mut authority),
        Err(TypeAliasInterfaceSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn table_semantics_reports_the_failing_canonical_record_index() {
    let fixture = Fixture::new();
    let interfaces = fixture.interfaces();
    let mut authority = fixture.semantic_authority();
    assert!(interfaces.validate_semantics(&mut authority).is_ok());

    let failing = interfaces.records()[1].alias();
    authority.sources.remove(&failing);
    assert!(matches!(
        interfaces.validate_semantics(&mut authority),
        Err(TypeAliasInterfaceSetSemanticValidationError::Record {
            index: 1,
            error: TypeAliasInterfaceSemanticValidationError::Declaration(
                TestAuthorityError::Alias(alias)
            ),
        }) if alias == failing
    ));
}

struct Fixture {
    first_identity: CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>,
    second_identity: CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>,
    target_identity: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    context: CborIdentityRecord<PersistentSourceContextId, SourceContextKey>,
    first: TypeAliasInterfaceRecordV1,
    second: TypeAliasInterfaceRecordV1,
    sources: BTreeMap<PersistentTypeAliasId, TypeAliasDeclarationSourceV1>,
}

impl Fixture {
    fn new() -> Self {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/Aliases.scoop").unwrap(),
        )
        .unwrap();
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: source.clone(),
        })
        .unwrap();
        let first_identity = alias_identity("First");
        let second_identity = alias_identity("Second");
        let target_identity = nominal_identity("Target");
        let first_origin = DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(0, 5).unwrap(),
            context.key(),
        )
        .unwrap();
        let second_origin =
            DefinitionOrigin::new(source, SourceSpan::new(6, 12).unwrap(), context.key()).unwrap();
        let first = record(
            first_identity.id(),
            target_identity.id(),
            first_origin.clone(),
        );
        let second = record(
            second_identity.id(),
            target_identity.id(),
            second_origin.clone(),
        );
        let sources = BTreeMap::from([
            (
                first_identity.id(),
                TypeAliasDeclarationSourceV1::new(
                    first_identity.key().clone(),
                    PublicLookupAccessV1::DirectOnly,
                    first_origin,
                ),
            ),
            (
                second_identity.id(),
                TypeAliasDeclarationSourceV1::new(
                    second_identity.key().clone(),
                    PublicLookupAccessV1::DirectOnly,
                    second_origin,
                ),
            ),
        ]);
        Self {
            first_identity,
            second_identity,
            target_identity,
            context,
            first,
            second,
            sources,
        }
    }

    fn interfaces(&self) -> CanonicalTypeAliasInterfacesV1 {
        CanonicalTypeAliasInterfacesV1::try_new(vec![self.first.clone(), self.second.clone()])
            .unwrap()
    }

    fn identity_authority(&self) -> ValidatedIdentityGraph {
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending
            .register_external_canonical_authority(self.first_identity.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.second_identity.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.target_identity.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.context.clone())
            .unwrap();
        pending.finish().unwrap()
    }

    fn semantic_authority(&self) -> TestAuthority {
        TestAuthority {
            sources: self.sources.clone(),
            target: self.target_identity.id(),
        }
    }
}

struct TestAuthority {
    sources: BTreeMap<PersistentTypeAliasId, TypeAliasDeclarationSourceV1>,
    target: PersistentTypeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestAuthorityError {
    Alias(PersistentTypeAliasId),
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
}

impl std::fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing test authority: {self:?}")
    }
}

impl std::error::Error for TestAuthorityError {}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for TestAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        if declaration == self.target {
            Ok(PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0))
        } else {
            Err(TestAuthorityError::Concrete(declaration))
        }
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::Generic(declaration))
    }
}

impl TypeAliasInterfaceSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn current_cone(&self) -> ConeIdentity {
        ConeIdentity::CORE
    }

    fn type_alias_declaration_source(
        &mut self,
        alias: PersistentTypeAliasId,
    ) -> Result<TypeAliasDeclarationSourceV1, TestAuthorityError> {
        self.sources
            .get(&alias)
            .cloned()
            .ok_or(TestAuthorityError::Alias(alias))
    }
}

struct RecordSequence(Vec<TypeAliasInterfaceRecordV1>);

impl WireEncode for RecordSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

fn ordered(fixture: &Fixture) -> (&TypeAliasInterfaceRecordV1, &TypeAliasInterfaceRecordV1) {
    if fixture.first.alias() < fixture.second.alias() {
        (&fixture.first, &fixture.second)
    } else {
        (&fixture.second, &fixture.first)
    }
}

fn decode_table(value: &impl WireEncode) -> DecodedCanonicalTypeAliasInterfacesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

fn record(
    alias: PersistentTypeAliasId,
    target: PersistentTypeId,
    origin: DefinitionOrigin,
) -> TypeAliasInterfaceRecordV1 {
    TypeAliasInterfaceRecordV1::try_new(
        alias,
        TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(target)),
        PublicLookupAccessV1::DirectOnly,
        ExportDefinitionSourceV1::new(origin),
    )
    .unwrap()
}

fn alias_identity(name: &str) -> CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::type_alias(site(), identifier(name)))
        .unwrap()
}

fn nominal_identity(name: &str) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(),
        identifier(name),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
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
