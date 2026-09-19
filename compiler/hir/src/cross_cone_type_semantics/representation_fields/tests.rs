use std::sync::Arc;

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DecodedPersistentId,
    DefinitionOwnerChain, EnumVariantFieldSelector, EnumVariantIdentityKey, GeneratedNominalKey,
    PackagePath, PersistentGenericTypeId, PersistentIdResolver, PersistentKeyResolver,
    PersistentPropertyId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

fn declaration(kind: SourceNominalKind) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(site(), CanonicalIdentifier::new("Owner").unwrap(), kind, 0)
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
fn unit() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
}
fn property() -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        site(),
        CanonicalIdentifier::new("value").unwrap(),
    ))
    .unwrap()
}
fn struct_key() -> FieldIdentityKey {
    FieldIdentityKey::source_declared(
        &declaration(SourceNominalKind::Struct),
        CanonicalIdentifier::new("value").unwrap(),
    )
    .unwrap()
}
fn class_key() -> FieldIdentityKey {
    FieldIdentityKey::source_property_backing(&declaration(SourceNominalKind::Class), property())
        .unwrap()
}

struct Resolver {
    field: FieldIdentityKey,
    enumeration: EnumVariantFieldKey,
}
impl PersistentIdResolver<PersistentTypeId> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        id.verify(CoreBuiltinNominal::Unit.identity_record().id())
            .map_err(|_| "unknown nominal")
    }
}
impl PersistentIdResolver<PersistentGenericTypeId> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        _: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err("unknown generic nominal")
    }
}
impl PersistentKeyResolver<PersistentFieldId, FieldIdentityKey> for Resolver {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        _: DecodedPersistentId<PersistentFieldId>,
    ) -> Result<Arc<FieldIdentityKey>, Self::Error> {
        Ok(Arc::new(self.field.clone()))
    }
}
impl PersistentKeyResolver<PersistentEnumVariantFieldId, EnumVariantFieldKey> for Resolver {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        _: DecodedPersistentId<PersistentEnumVariantFieldId>,
    ) -> Result<Arc<EnumVariantFieldKey>, Self::Error> {
        Ok(Arc::new(self.enumeration.clone()))
    }
}

fn resolver(field: FieldIdentityKey) -> Resolver {
    let variant = PersistentEnumVariantId::from_key(
        &EnumVariantIdentityKey::source(
            &declaration(SourceNominalKind::Enum),
            CanonicalIdentifier::new("Value").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    Resolver {
        field,
        enumeration: EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ),
    }
}

#[test]
fn source_field_wrappers_reject_each_others_identity_roles() {
    assert!(StructRepresentationFieldV1::try_new(&class_key(), unit()).is_err());
    assert!(ClassRepresentationFieldV1::try_new(&struct_key(), unit()).is_err());
    let structure = StructRepresentationFieldV1::try_new(&struct_key(), unit()).unwrap();
    let class = ClassRepresentationFieldV1::try_new(&class_key(), unit()).unwrap();
    assert_ne!(structure.field(), class.field());
    assert!(matches!(
        class.owner(),
        ClassRepresentationFieldOwnerV1::SourceClass(_)
    ));
}

#[test]
fn object_backing_fields_are_accepted_without_accepting_arbitrary_generated_fields() {
    let object =
        PersistentTypeId::from_source_declaration(&declaration(SourceNominalKind::Object)).unwrap();
    let owner = GeneratedNominalKey::ObjectBackingClass { object };
    let key = FieldIdentityKey::object_backing_property(&owner, property()).unwrap();
    let field = ClassRepresentationFieldV1::try_new(&key, unit()).unwrap();
    assert_eq!(
        field.owner(),
        ClassRepresentationFieldOwnerV1::ObjectBackingClass(
            PersistentTypeId::from_generated_key(&owner).unwrap()
        )
    );
    assert!(StructRepresentationFieldV1::try_new(&key, unit()).is_err());
    let exact = scoop_identity::PersistentExactTypeId::from_key(
        &scoop_identity::ExactTypeKey::Nominal(object),
    )
    .unwrap();
    let box_key =
        FieldIdentityKey::box_payload(&GeneratedNominalKey::BoxedValue { payload: exact }).unwrap();
    assert!(ClassRepresentationFieldV1::try_new(&box_key, unit()).is_err());
}

#[test]
fn field_wire_contains_only_id_and_signature_and_rebuilds_derived_owner() {
    let mut resolver = resolver(struct_key());
    let field = StructRepresentationFieldV1::try_new(&resolver.field, unit()).unwrap();
    assert_eq!(
        encode(&field).unwrap(),
        [
            b"\xa2\x01\x58\x20".as_slice(),
            field.field().as_array(),
            b"\x02",
            &encode(&unit()).unwrap()
        ]
        .concat()
    );
    let decoded: DecodedStructRepresentationFieldV1 =
        decode_canonical(&encode(&field).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.clone().resolve(&mut resolver).unwrap(), field);
    resolver.field = class_key();
    assert!(decoded.resolve(&mut resolver).is_err());
    let class = ClassRepresentationFieldV1::try_new(&resolver.field, unit()).unwrap();
    let decoded: DecodedClassRepresentationFieldV1 =
        decode_canonical(&encode(&class).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut resolver).unwrap(), class);
    let enumeration = EnumRepresentationFieldV1::try_new(&resolver.enumeration, unit()).unwrap();
    let decoded: DecodedEnumRepresentationFieldV1 =
        decode_canonical(&encode(&enumeration).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut resolver).unwrap(), enumeration);
}

#[test]
fn reader_rechecks_key_identity_even_if_a_resolver_returns_another_valid_key() {
    let key = struct_key();
    let field = StructRepresentationFieldV1::try_new(&key, unit()).unwrap();
    let other = FieldIdentityKey::source_declared(
        &declaration(SourceNominalKind::Struct),
        CanonicalIdentifier::new("other").unwrap(),
    )
    .unwrap();
    let decoded: DecodedStructRepresentationFieldV1 =
        decode_canonical(&encode(&field).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut resolver(other)),
        Err(RepresentationFieldResolutionError::Identity(_))
    ));
}
