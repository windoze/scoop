use crate::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerAtom, DefinitionOwnerChain, IdentityReferenceError, NominalDeclarationOwner,
    PackagePath, PersistentExtensionPropertyId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;
use crate::{TypeParameterBinderV1, TypeParameterBoundsV1};

mod source_forms;

#[test]
fn property_record_has_fixed_field_wire_and_accessors() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let expected = [
        b"\xa3\x01\xa8\x01".as_slice(),
        encode(&record.declaration()).unwrap().as_slice(),
        b"\x02".as_slice(),
        encode(&record.owner()).unwrap().as_slice(),
        b"\x03".as_slice(),
        encode(record.type_parameters()).unwrap().as_slice(),
        b"\x04".as_slice(),
        encode(&OptionalSignatureType::from_option(
            record.receiver().cloned(),
        ))
        .unwrap()
        .as_slice(),
        b"\x05".as_slice(),
        encode(record.value_type()).unwrap().as_slice(),
        b"\x06".as_slice(),
        encode(&record.accessors()).unwrap().as_slice(),
        b"\x07".as_slice(),
        encode(&record.representation()).unwrap().as_slice(),
        b"\x08".as_slice(),
        encode(&record.declared_visibility()).unwrap().as_slice(),
        b"\x02".as_slice(),
        encode(&record.access()).unwrap().as_slice(),
        b"\x03".as_slice(),
        encode(&record.capability().setter_access().unwrap())
            .unwrap()
            .as_slice(),
    ]
    .concat();

    assert_eq!(encode(&record).unwrap(), expected);
    assert_eq!(record.declaration(), fixture.declaration());
    assert_eq!(record.owner(), PublicDeclarationOwnerV1::Extension);
    assert_eq!(record.type_parameters().len_u32(), 1);
    assert_eq!(record.receiver(), Some(&binder()));
    assert_eq!(record.value_type(), &binder());
    assert_eq!(record.capability().getter(), fixture.getter.id());
    assert_eq!(record.capability().setter(), Some(fixture.setter.id()));
    assert_eq!(
        record.representation(),
        PropertyRepresentationV1::RuntimeAccessor
    );
    assert_eq!(record.access(), PropertyPublicAccessV1::DirectOnly);
}

#[test]
fn decoded_record_resolves_every_typed_constituent() {
    let fixture = Fixture::new();
    let expected = fixture.record();
    let decoded = decode_record(&expected);
    let mut authority = fixture.authority();

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);

    let expected = fixture.nominal_record();
    let decoded = decode_record(&expected);
    let mut authority = fixture.authority();
    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn producer_rejects_declaration_binder_owner_and_receiver_mismatches() {
    let fixture = Fixture::new();
    let ordinary = fixture.ordinary_declaration();
    assert_eq!(
        build_record(
            ordinary,
            PublicDeclarationOwnerV1::TopLevel,
            one_binder(),
            None,
            read_only(fixture.getter.id()),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::DirectOnly,
        ),
        Err(PropertyInterfaceRecordBuildError::UnexpectedTypeParameters(
            ordinary
        ))
    );
    assert_eq!(
        build_record(
            ordinary,
            PublicDeclarationOwnerV1::Extension,
            empty_binders(),
            None,
            read_only(fixture.getter.id()),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::DirectOnly,
        ),
        Err(PropertyInterfaceRecordBuildError::UnexpectedExtensionOwner(
            ordinary
        ))
    );
    assert_eq!(
        build_record(
            ordinary,
            PublicDeclarationOwnerV1::TopLevel,
            empty_binders(),
            Some(binder()),
            read_only(fixture.getter.id()),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::DirectOnly,
        ),
        Err(PropertyInterfaceRecordBuildError::UnexpectedReceiver(
            PublicDeclarationOwnerV1::TopLevel
        ))
    );
    assert_eq!(
        build_record(
            fixture.declaration(),
            PublicDeclarationOwnerV1::TopLevel,
            one_binder(),
            Some(binder()),
            fixture.capability(),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::DirectOnly,
        ),
        Err(PropertyInterfaceRecordBuildError::ExtensionOwnerRequired {
            declaration: fixture.declaration(),
            actual: PublicDeclarationOwnerV1::TopLevel,
        })
    );
    assert_eq!(
        build_record(
            fixture.declaration(),
            PublicDeclarationOwnerV1::Extension,
            one_binder(),
            None,
            fixture.capability(),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::DirectOnly,
        ),
        Err(PropertyInterfaceRecordBuildError::MissingExtensionReceiver)
    );
}

#[test]
fn producer_rejects_invalid_access_and_representation_shapes() {
    let fixture = Fixture::new();
    assert!(matches!(
        build_record(
            fixture.declaration(),
            PublicDeclarationOwnerV1::Extension,
            one_binder(),
            Some(binder()),
            fixture.capability(),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::PublicSlot,
        ),
        Err(PropertyInterfaceRecordBuildError::DirectPropertyContract { .. })
    ));
    assert_eq!(
        build_record(
            fixture.ordinary_declaration(),
            PublicDeclarationOwnerV1::TopLevel,
            empty_binders(),
            None,
            fixture.capability(),
            PropertyRepresentationV1::Const,
            PropertyPublicAccessV1::DirectOnly,
        ),
        Err(PropertyInterfaceRecordBuildError::ConstMustBeReadOnly(
            fixture.ordinary_declaration()
        ))
    );
    assert!(matches!(
        build_record(
            fixture.ordinary_declaration(),
            PublicDeclarationOwnerV1::TopLevel,
            empty_binders(),
            None,
            read_only(fixture.getter.id()),
            PropertyRepresentationV1::AbstractSlot,
            PropertyPublicAccessV1::DirectOnly,
        ),
        Err(PropertyInterfaceRecordBuildError::AbstractNominalOwnerRequired { .. })
    ));

    let nominal_owner = fixture.nominal_owner();
    assert_eq!(
        build_record(
            fixture.ordinary_declaration(),
            nominal_owner,
            empty_binders(),
            None,
            read_only(fixture.ordinary_getter.id()),
            PropertyRepresentationV1::AbstractSlot,
            PropertyPublicAccessV1::DirectOnly,
        ),
        Err(
            PropertyInterfaceRecordBuildError::AbstractSlotAccessRequired(
                fixture.ordinary_declaration()
            )
        )
    );
    assert_eq!(
        build_record(
            fixture.ordinary_declaration(),
            nominal_owner,
            empty_binders(),
            None,
            read_only(fixture.ordinary_getter.id()),
            PropertyRepresentationV1::Const,
            PropertyPublicAccessV1::PublicSlot,
        ),
        Err(PropertyInterfaceRecordBuildError::ConstMustBeDirect(
            fixture.ordinary_declaration()
        ))
    );
}

#[test]
fn reader_replays_record_invariants_and_exact_map_shape() {
    let fixture = Fixture::new();
    let mut decoded = decode_record(&fixture.record());
    decoded.access = PropertyPublicAccessV1::PublicSlot;
    let mut authority = fixture.authority();
    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(PropertyInterfaceRecordResolutionError::Record(
            PropertyInterfaceRecordBuildError::DirectPropertyContract { .. }
        ))
    ));

    let error =
        decode_canonical::<DecodedPropertyInterfaceRecordV1>(&[0xa0], DecodeLimits::default())
            .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 3,
            actual: 0,
        }
    );
}

#[test]
fn reader_reports_missing_property_identity_authority() {
    let fixture = Fixture::new();
    let mut empty = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();

    assert!(matches!(
        decode_record(&fixture.record()).resolve(&mut empty),
        Err(PropertyInterfaceRecordResolutionError::Declaration(
            IdentityReferenceError::Missing { .. }
        ))
    ));
}

struct Fixture {
    nominal: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    extension: CborIdentityRecord<PersistentExtensionPropertyId, SourceDeclarationKey>,
    ordinary: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>,
    getter: CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey>,
    setter: CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey>,
    ordinary_getter: CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey>,
}

impl Fixture {
    fn new() -> Self {
        let nominal = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Container"),
            SourceNominalKind::Class,
            0,
        ))
        .unwrap();
        let extension = CborIdentityRecord::from_key(SourceDeclarationKey::extension_property(
            top_level_site(),
            identifier("content"),
            1,
            binder(),
        ))
        .unwrap();
        let ordinary = CborIdentityRecord::from_key(SourceDeclarationKey::property(
            owned_site(DefinitionOwnerAtom::Type(nominal.id())),
            identifier("count"),
        ))
        .unwrap();
        let owner = PropertyOwner::ExtensionProperty(extension.id());
        let getter =
            CborIdentityRecord::from_key(PropertyAccessorKey::new(owner, AccessorRole::Getter))
                .unwrap();
        let setter =
            CborIdentityRecord::from_key(PropertyAccessorKey::new(owner, AccessorRole::Setter))
                .unwrap();
        let ordinary_getter = CborIdentityRecord::from_key(PropertyAccessorKey::new(
            PropertyOwner::Property(ordinary.id()),
            AccessorRole::Getter,
        ))
        .unwrap();
        Self {
            nominal,
            extension,
            ordinary,
            getter,
            setter,
            ordinary_getter,
        }
    }

    fn declaration(&self) -> PropertyDeclarationId {
        PropertyOwner::ExtensionProperty(self.extension.id())
    }

    fn ordinary_declaration(&self) -> PropertyDeclarationId {
        PropertyOwner::Property(self.ordinary.id())
    }

    fn capability(&self) -> PropertyCapabilityV1 {
        PropertyCapabilityV1::try_read_write(
            self.getter.id(),
            self.setter.id(),
            crate::PropertySetterPublicAccessV1::Public,
        )
        .unwrap()
    }

    fn nominal_owner(&self) -> PublicDeclarationOwnerV1 {
        PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(self.nominal.id()))
    }

    fn record(&self) -> PropertyInterfaceRecordV1 {
        build_record(
            self.declaration(),
            PublicDeclarationOwnerV1::Extension,
            one_binder(),
            Some(binder()),
            self.capability(),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::DirectOnly,
        )
        .unwrap()
    }

    fn nominal_record(&self) -> PropertyInterfaceRecordV1 {
        PropertyInterfaceRecordV1::try_new(
            self.ordinary_declaration(),
            self.nominal_owner(),
            empty_binders(),
            None,
            SignatureTypeKey::Nominal(self.nominal.id()),
            Accessors::read_only(AccessorSource::new(
                self.ordinary_getter.id(),
                AccessorForm::AbstractSlot,
            )),
            PropertyRepresentationV1::AbstractSlot,
            PropertyPublicAccessV1::PublicSlot,
            crate::PropertySetterPublicAccessV1::Restricted,
        )
        .unwrap()
    }

    fn authority(&self) -> scoop_identity::ValidatedIdentityGraph {
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        pending
            .register_external_canonical_authority(self.nominal.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.extension.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.ordinary.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.getter.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.setter.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.ordinary_getter.clone())
            .unwrap();
        pending.finish().unwrap()
    }
}

#[allow(clippy::too_many_arguments)]
fn build_record(
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    type_parameters: CanonicalBinderListV1,
    receiver: Option<SignatureTypeKey>,
    capability: PropertyCapabilityV1,
    representation: PropertyRepresentationV1,
    access: PropertyPublicAccessV1,
) -> Result<PropertyInterfaceRecordV1, PropertyInterfaceRecordBuildError> {
    PropertyInterfaceRecordV1::try_new(
        declaration,
        owner,
        type_parameters,
        receiver,
        binder(),
        {
            let form = match representation {
                PropertyRepresentationV1::Const => AccessorForm::Constant,
                PropertyRepresentationV1::RuntimeAccessor => AccessorForm::Body,
                PropertyRepresentationV1::AbstractSlot => AccessorForm::AbstractSlot,
            };
            let getter = AccessorSource::new(capability.getter(), form);
            match capability.setter() {
                None => Accessors::read_only(getter),
                Some(setter) => {
                    Accessors::try_read_write(getter, AccessorSource::new(setter, form)).unwrap()
                }
            }
        },
        representation,
        access,
        capability
            .setter_access()
            .unwrap_or(crate::PropertySetterPublicAccessV1::Restricted),
    )
}

fn read_only(getter: PersistentPropertyAccessorId) -> PropertyCapabilityV1 {
    PropertyCapabilityV1::read_only(getter)
}

fn decode_record(value: &PropertyInterfaceRecordV1) -> DecodedPropertyInterfaceRecordV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

fn one_binder() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        identifier("T"),
        TypeParameterBoundsV1::Unconstrained,
    )])
    .unwrap()
}

fn empty_binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(Vec::new()).unwrap()
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

fn owned_site(owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

const fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
}
