use std::collections::BTreeMap;

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOrigin, DefinitionOwnerChain, NormalizedSourcePath, PackagePath,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentSourceContextId,
    PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
    ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalBinderListV1, CanonicalBooleanV1, CanonicalConstValueKindV1, CanonicalConstValueV1,
    CanonicalPropertyInterfacesV1, ConstPropertyDeclarationSourceV1,
    ExportConstValueClosureValidationError, ExportConstValueSemanticAuthority,
    ExportConstValueSemanticValidationError, ExportDefinitionSourceV1, PropertyCapabilityV1,
    PropertyInterfaceRecordV1, PropertyPublicAccessV1, PropertyRepresentationV1,
    PublicDeclarationOwnerV1,
};

#[test]
fn producer_sorts_records_rejects_duplicates_and_has_stable_wire() {
    let fixture = Fixture::new();
    let values =
        CanonicalExportConstValuesV1::try_new(vec![fixture.second.clone(), fixture.first.clone()])
            .unwrap();

    assert!(values.records()[0].property() < values.records()[1].property());
    assert_eq!(values.get(fixture.first.property()), Some(&fixture.first));
    assert!(!values.is_empty());
    assert_eq!(
        CanonicalExportConstValuesV1::try_new(vec![fixture.first.clone(), fixture.first.clone(),]),
        Err(ExportConstValueSetBuildError::DuplicateProperty(
            fixture.first.property()
        ))
    );
    assert_eq!(
        encode(&values).unwrap(),
        [
            b"\x82".as_slice(),
            encode(&values.records()[0]).unwrap().as_slice(),
            encode(&values.records()[1]).unwrap().as_slice(),
        ]
        .concat()
    );
}

#[test]
fn decoded_table_resolves_records_through_typed_authority() {
    let fixture = Fixture::new();
    let expected = fixture.values();
    let decoded = decode_table(&expected);

    assert_eq!(
        decoded.resolve(&mut fixture.identity_authority()).unwrap(),
        expected
    );
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_property_order() {
    let fixture = Fixture::new();
    let (low, high) = ordered(&fixture);

    let duplicate = decode_table(&RecordSequence(vec![low.clone(), low.clone()]));
    assert!(matches!(
        duplicate.resolve(&mut fixture.identity_authority()),
        Err(ExportConstValueSetValidationError::DuplicateProperty {
            index: 1,
            property,
        }) if property == low.property()
    ));

    let reversed = decode_table(&RecordSequence(vec![high.clone(), low.clone()]));
    assert!(matches!(
        reversed.resolve(&mut fixture.identity_authority()),
        Err(ExportConstValueSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn table_semantics_reports_the_failing_canonical_record_index() {
    let fixture = Fixture::new();
    let values = fixture.values();
    let mut authority = fixture.semantic_authority();
    assert_eq!(values.validate_semantics(&mut authority), Ok(()));

    let failing = values.records()[1].property();
    authority.sources.remove(&failing);
    assert!(matches!(
        values.validate_semantics(&mut authority),
        Err(ExportConstValueSetSemanticValidationError::Record {
            index: 1,
            error: ExportConstValueSemanticValidationError::Declaration(
                TestAuthorityError::Declaration(property)
            ),
        }) if property == failing
    ));
}

#[test]
fn property_closure_accepts_every_const_and_ignores_runtime_properties() {
    let fixture = Fixture::new();
    let values = fixture.values();
    let properties = fixture.properties(true, true);

    assert_eq!(values.validate_property_closure(&properties), Ok(()));
}

#[test]
fn property_closure_rejects_a_missing_const_value() {
    let fixture = Fixture::new();
    let empty = CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap();
    let properties = fixture.properties(true, true);
    let expected = properties
        .records()
        .iter()
        .find(|record| record.representation() == PropertyRepresentationV1::Const)
        .and_then(|record| match record.declaration() {
            PropertyOwner::Property(property) => Some(property),
            PropertyOwner::ExtensionProperty(_) => None,
        })
        .unwrap();

    assert_eq!(
        empty.validate_property_closure(&properties),
        Err(ExportConstValueClosureValidationError::MissingConstValue(
            expected
        ))
    );
}

#[test]
fn property_closure_rejects_an_orphan_const_value() {
    let fixture = Fixture::new();
    let values = fixture.values();
    let runtime_only = fixture.properties(false, false);

    assert_eq!(
        values.validate_property_closure(&runtime_only),
        Err(ExportConstValueClosureValidationError::OrphanConstValue {
            index: 0,
            property: values.records()[0].property(),
        })
    );
}

struct Fixture {
    first_identity: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>,
    second_identity: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>,
    runtime_identity: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>,
    boolean_identity: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    string_identity: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    context: CborIdentityRecord<PersistentSourceContextId, SourceContextKey>,
    first: ExportConstValueV1,
    second: ExportConstValueV1,
    first_interface: PropertyInterfaceRecordV1,
    second_interface: PropertyInterfaceRecordV1,
    runtime_interface: PropertyInterfaceRecordV1,
    sources: BTreeMap<PersistentPropertyId, ConstPropertyDeclarationSourceV1>,
}

impl Fixture {
    fn new() -> Self {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/Constants.scoop").unwrap(),
        )
        .unwrap();
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: source.clone(),
        })
        .unwrap();
        let first_identity = property_identity("First");
        let second_identity = property_identity("Second");
        let runtime_identity = property_identity("Runtime");
        let boolean_identity = nominal_identity("Boolean", SourceNominalKind::Struct);
        let string_identity = nominal_identity("String", SourceNominalKind::Class);
        let first_origin = DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(0, 5).unwrap(),
            context.key(),
        )
        .unwrap();
        let second_origin =
            DefinitionOrigin::new(source, SourceSpan::new(6, 12).unwrap(), context.key()).unwrap();
        let first = ExportConstValueV1::new(
            first_identity.id(),
            SignatureTypeKey::Nominal(boolean_identity.id()),
            CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
            ExportDefinitionSourceV1::new(first_origin.clone()),
        );
        let second = ExportConstValueV1::new(
            second_identity.id(),
            SignatureTypeKey::Nominal(string_identity.id()),
            CanonicalConstValueV1::String("second".to_owned()),
            ExportDefinitionSourceV1::new(second_origin.clone()),
        );
        let first_interface = property_interface(
            first_identity.id(),
            boolean_identity.id(),
            PropertyRepresentationV1::Const,
        );
        let second_interface = property_interface(
            second_identity.id(),
            string_identity.id(),
            PropertyRepresentationV1::Const,
        );
        let runtime_interface = property_interface(
            runtime_identity.id(),
            boolean_identity.id(),
            PropertyRepresentationV1::RuntimeAccessor,
        );
        let sources = BTreeMap::from([
            (
                first_identity.id(),
                ConstPropertyDeclarationSourceV1::new(first_identity.key().clone(), first_origin),
            ),
            (
                second_identity.id(),
                ConstPropertyDeclarationSourceV1::new(second_identity.key().clone(), second_origin),
            ),
        ]);
        Self {
            first_identity,
            second_identity,
            runtime_identity,
            boolean_identity,
            string_identity,
            context,
            first,
            second,
            first_interface,
            second_interface,
            runtime_interface,
            sources,
        }
    }

    fn values(&self) -> CanonicalExportConstValuesV1 {
        CanonicalExportConstValuesV1::try_new(vec![self.first.clone(), self.second.clone()])
            .unwrap()
    }

    fn properties(
        &self,
        include_first: bool,
        include_second: bool,
    ) -> CanonicalPropertyInterfacesV1 {
        let mut records = vec![self.runtime_interface.clone()];
        if include_first {
            records.push(self.first_interface.clone());
        }
        if include_second {
            records.push(self.second_interface.clone());
        }
        CanonicalPropertyInterfacesV1::try_new(records).unwrap()
    }

    fn identity_authority(&self) -> ValidatedIdentityGraph {
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        for record in [
            &self.first_identity,
            &self.second_identity,
            &self.runtime_identity,
        ] {
            pending
                .register_external_canonical_authority(record.clone())
                .unwrap();
        }
        pending
            .register_external_canonical_authority(self.boolean_identity.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.string_identity.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.context.clone())
            .unwrap();
        pending.finish().unwrap()
    }

    fn semantic_authority(&self) -> TestAuthority {
        TestAuthority {
            sources: self.sources.clone(),
            interfaces: BTreeMap::from([
                (self.first.property(), self.first_interface.clone()),
                (self.second.property(), self.second_interface.clone()),
            ]),
            core_types: BTreeMap::from([
                (
                    CanonicalConstValueKindV1::Boolean,
                    self.boolean_identity.id(),
                ),
                (CanonicalConstValueKindV1::String, self.string_identity.id()),
            ]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestAuthorityError {
    Declaration(PersistentPropertyId),
    Interface(PersistentPropertyId),
    Core(CanonicalConstValueKindV1),
}

impl std::fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing test authority: {self:?}")
    }
}

impl std::error::Error for TestAuthorityError {}

struct TestAuthority {
    sources: BTreeMap<PersistentPropertyId, ConstPropertyDeclarationSourceV1>,
    interfaces: BTreeMap<PersistentPropertyId, PropertyInterfaceRecordV1>,
    core_types: BTreeMap<CanonicalConstValueKindV1, PersistentTypeId>,
}

impl ExportConstValueSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn current_cone(&self) -> ConeIdentity {
        ConeIdentity::CORE
    }

    fn const_property_declaration_source(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<ConstPropertyDeclarationSourceV1, TestAuthorityError> {
        self.sources
            .get(&property)
            .cloned()
            .ok_or(TestAuthorityError::Declaration(property))
    }

    fn validated_property_interface(
        &mut self,
        property: PersistentPropertyId,
    ) -> Result<&PropertyInterfaceRecordV1, TestAuthorityError> {
        self.interfaces
            .get(&property)
            .ok_or(TestAuthorityError::Interface(property))
    }

    fn validate_const_value_type(
        &mut self,
        value_type: PersistentTypeId,
        kind: CanonicalConstValueKindV1,
    ) -> Result<(), TestAuthorityError> {
        self.core_types
            .get(&kind)
            .filter(|&&expected| expected == value_type)
            .map(|_| ())
            .ok_or(TestAuthorityError::Core(kind))
    }
}

struct RecordSequence(Vec<ExportConstValueV1>);

impl WireEncode for RecordSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

fn ordered(fixture: &Fixture) -> (&ExportConstValueV1, &ExportConstValueV1) {
    if fixture.first.property() < fixture.second.property() {
        (&fixture.first, &fixture.second)
    } else {
        (&fixture.second, &fixture.first)
    }
}

fn decode_table(value: &impl WireEncode) -> DecodedCanonicalExportConstValuesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

fn property_interface(
    property: PersistentPropertyId,
    value_type: PersistentTypeId,
    representation: PropertyRepresentationV1,
) -> PropertyInterfaceRecordV1 {
    let declaration = PropertyOwner::Property(property);
    let getter = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        declaration,
        AccessorRole::Getter,
    ))
    .unwrap();
    PropertyInterfaceRecordV1::try_new(
        declaration,
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        SignatureTypeKey::Nominal(value_type),
        PropertyCapabilityV1::read_only(getter),
        representation,
        PropertyPublicAccessV1::DirectOnly,
    )
    .unwrap()
}

fn property_identity(name: &str) -> CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::property(site(), identifier(name))).unwrap()
}

fn nominal_identity(
    name: &str,
    kind: SourceNominalKind,
) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(),
        identifier(name),
        kind,
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
