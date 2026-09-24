use crate::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use std::collections::BTreeMap;

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, PackagePath, PersistentGenericTypeId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalBinderListV1, NominalInterfaceShapeAuthority, PropertyCapabilityV1,
    PropertyDeclarationIdentityShapeV1, PropertyDeclarationSourceShapeV1, PropertyPublicAccessV1,
    PropertyRepresentationV1, PublicDeclarationOwnerV1, PublicNominalKindV1, PublicNominalShapeV1,
};

mod support_wire;

#[test]
fn producer_sorts_records_rejects_duplicates_and_has_stable_wire() {
    let first = fixture("First");
    let second = fixture("Second");
    let interfaces =
        CanonicalPropertyInterfacesV1::try_new(vec![second.record.clone(), first.record.clone()])
            .unwrap();

    assert!(interfaces.records()[0].declaration() < interfaces.records()[1].declaration());
    assert_eq!(
        interfaces.get(first.record.declaration()),
        Some(&first.record)
    );
    assert!(!interfaces.is_empty());
    assert_eq!(
        CanonicalPropertyInterfacesV1::try_new(vec![first.record.clone(), first.record.clone()]),
        Err(PropertyInterfaceSetBuildError::DuplicateDeclaration(
            first.record.declaration()
        ))
    );

    let expected = [
        b"\xa2\x01\x82".as_slice(),
        encode(&interfaces.records()[0]).unwrap().as_slice(),
        encode(&interfaces.records()[1]).unwrap().as_slice(),
        b"\x02\x80".as_slice(),
    ]
    .concat();
    assert_eq!(encode(&interfaces).unwrap(), expected);
}

#[test]
fn decoded_table_resolves_records_through_typed_authority() {
    let first = fixture("First");
    let second = fixture("Second");
    let expected =
        CanonicalPropertyInterfacesV1::try_new(vec![first.record.clone(), second.record.clone()])
            .unwrap();
    let decoded = decode_table(&expected);
    let mut authority = identity_authority(&[&first, &second]);

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_record_order() {
    let first = fixture("First");
    let second = fixture("Second");
    let (low, high) = if first.record.declaration() < second.record.declaration() {
        (&first, &second)
    } else {
        (&second, &first)
    };

    let duplicate = decode_table(&RecordSequence(vec![
        low.record.clone(),
        low.record.clone(),
    ]));
    let mut duplicate_authority = identity_authority(&[&first, &second]);
    assert!(matches!(
        duplicate.resolve(&mut duplicate_authority),
        Err(PropertyInterfaceSetValidationError::DuplicateDeclaration { index: 1, .. })
    ));

    let reversed = decode_table(&RecordSequence(vec![
        high.record.clone(),
        low.record.clone(),
    ]));
    let mut reversed_authority = identity_authority(&[&first, &second]);
    assert_eq!(
        reversed
            .resolve(&mut reversed_authority)
            .unwrap_err()
            .to_string(),
        "non-canonical property interface order at index 1"
    );
}

#[test]
fn table_semantics_reports_the_failing_canonical_record_index() {
    let first = fixture("First");
    let second = fixture("Second");
    let interfaces =
        CanonicalPropertyInterfacesV1::try_new(vec![first.record.clone(), second.record.clone()])
            .unwrap();
    let mut authority = semantic_authority(&[&first, &second]);
    assert!(interfaces.validate_semantics(&mut authority).is_ok());

    let target = interfaces.records()[1].declaration();
    authority.identities.insert(
        target,
        PropertyDeclarationIdentityShapeV1::new(PublicDeclarationOwnerV1::TopLevel, 1, 0, None),
    );
    assert!(matches!(
        interfaces.validate_semantics(&mut authority),
        Err(PropertyInterfaceSetSemanticValidationError::Record {
            index: 1,
            error: PropertyInterfaceSemanticValidationError::TypeParameterArity {
                expected: 1,
                actual: 0,
            },
        })
    ));
}

struct Fixture {
    property: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>,
    getter: CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey>,
    value_type: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    record: PropertyInterfaceRecordV1,
    identity_shape: PropertyDeclarationIdentityShapeV1,
    source_shape: PropertyDeclarationSourceShapeV1,
}

fn fixture(name: &str) -> Fixture {
    let property = CborIdentityRecord::from_key(SourceDeclarationKey::property(
        top_level_site(),
        identifier(name),
    ))
    .unwrap();
    let declaration = PropertyOwner::Property(property.id());
    let getter =
        CborIdentityRecord::from_key(PropertyAccessorKey::new(declaration, AccessorRole::Getter))
            .unwrap();
    let value_type = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        top_level_site(),
        identifier(&format!("{name}Value")),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap();
    let capability = PropertyCapabilityV1::read_only(getter.id());
    let record = PropertyInterfaceRecordV1::try_new(
        declaration,
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        SignatureTypeKey::Nominal(value_type.id()),
        Accessors::read_only(AccessorSource::new(getter.id(), AccessorForm::Body)),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
        crate::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    Fixture {
        property,
        getter,
        value_type,
        record,
        identity_shape: PropertyDeclarationIdentityShapeV1::new(
            PublicDeclarationOwnerV1::TopLevel,
            0,
            0,
            None,
        ),
        source_shape: PropertyDeclarationSourceShapeV1::new(
            capability,
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::DirectOnly,
        ),
    }
}

fn identity_authority(fixtures: &[&Fixture]) -> ValidatedIdentityGraph {
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    for fixture in fixtures {
        pending
            .register_external_canonical_authority(fixture.property.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(fixture.getter.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(fixture.value_type.clone())
            .unwrap();
    }
    pending.finish().unwrap()
}

fn semantic_authority(fixtures: &[&Fixture]) -> SemanticAuthority {
    SemanticAuthority {
        identities: fixtures
            .iter()
            .map(|fixture| (fixture.record.declaration(), fixture.identity_shape.clone()))
            .collect(),
        sources: fixtures
            .iter()
            .map(|fixture| (fixture.record.declaration(), fixture.source_shape))
            .collect(),
        accessors: fixtures
            .iter()
            .map(|fixture| (fixture.getter.id(), fixture.getter.key().to_owned()))
            .collect(),
        nominals: fixtures
            .iter()
            .map(|fixture| {
                (
                    fixture.value_type.id(),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0),
                )
            })
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticAuthorityError {
    Property(PropertyDeclarationId),
    Source(PropertyDeclarationId),
    Accessor(PersistentPropertyAccessorId),
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
}

impl std::fmt::Display for SemanticAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing semantic authority: {self:?}")
    }
}

impl std::error::Error for SemanticAuthorityError {}

struct SemanticAuthority {
    identities: BTreeMap<PropertyDeclarationId, PropertyDeclarationIdentityShapeV1>,
    sources: BTreeMap<PropertyDeclarationId, PropertyDeclarationSourceShapeV1>,
    accessors: BTreeMap<PersistentPropertyAccessorId, PropertyAccessorKey>,
    nominals: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
}

impl NominalInterfaceShapeAuthority<SemanticAuthorityError> for SemanticAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, SemanticAuthorityError> {
        self.nominals
            .get(&declaration)
            .copied()
            .ok_or(SemanticAuthorityError::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, SemanticAuthorityError> {
        Err(SemanticAuthorityError::Generic(declaration))
    }
}

impl PropertyInterfaceSemanticAuthority<SemanticAuthorityError> for SemanticAuthority {
    fn property_declaration_identity_shape(
        &mut self,
        declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationIdentityShapeV1, SemanticAuthorityError> {
        self.identities
            .get(&declaration)
            .cloned()
            .ok_or(SemanticAuthorityError::Property(declaration))
    }

    fn property_declaration_source_shape(
        &mut self,
        declaration: PropertyDeclarationId,
    ) -> Result<PropertyDeclarationSourceShapeV1, SemanticAuthorityError> {
        self.sources
            .get(&declaration)
            .copied()
            .ok_or(SemanticAuthorityError::Source(declaration))
    }

    fn property_accessor_key(
        &mut self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<PropertyAccessorKey, SemanticAuthorityError> {
        self.accessors
            .get(&accessor)
            .copied()
            .ok_or(SemanticAuthorityError::Accessor(accessor))
    }
}

fn decode_table<T: WireEncode>(value: &T) -> DecodedCanonicalPropertyInterfacesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

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

struct RecordSequence(Vec<PropertyInterfaceRecordV1>);

impl WireEncode for RecordSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        encoder.field(2)?;
        encoder.array(0)?;
        Ok(())
    }
}
