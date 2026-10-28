use crate::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOrigin, DefinitionOwnerChain, NormalizedSourcePath, PackagePath,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentSourceContextId,
    PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
    ValidatedIdentityGraph,
};
use scoop_wire::{Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalBinderListV1, CanonicalBooleanV1, CanonicalConstValueV1,
    CanonicalPropertyInterfacesV1, ExportConstValueClosureValidationError,
    ExportDefinitionSourceV1, PropertyInterfaceRecordV1, PropertyPublicAccessV1,
    PropertyRepresentationV1, PublicDeclarationOwnerV1,
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
}

impl Fixture {
    fn new() -> Self {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/Constants.scoop").unwrap(),
        );
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
    decode_canonical(&encode(value).unwrap()).unwrap()
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
        Accessors::read_only(AccessorSource::new(
            getter,
            match representation {
                PropertyRepresentationV1::Const => AccessorForm::Constant,
                PropertyRepresentationV1::RuntimeAccessor => AccessorForm::Body,
                PropertyRepresentationV1::AbstractSlot => AccessorForm::AbstractSlot,
            },
        )),
        representation,
        PropertyPublicAccessV1::DirectOnly,
        crate::PropertySetterPublicAccessV1::Restricted,
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
