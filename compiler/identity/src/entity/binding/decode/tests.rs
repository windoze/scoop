use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::{
    BindingIdentityResolutionError, DecodedBindableEntity, DecodedExportBindingKey,
    DecodedLocalBindingKey,
};
use crate::{
    BindingNamespace, BindingRole, BindingTarget, BindingTargetError, CanonicalIdentifier,
    CborIdentityRecord, ConeIdentity, DeclarationScope, DecodedCborIdentityRecord,
    DecodedPersistentId, DefinitionOwnerChain, ExportBindingKey, LocalBindingKey, LocalBindingRole,
    PackagePath, PersistentAnnotationId, PersistentEnumVariantId, PersistentExportBindingId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentId, PersistentIdMismatch, PersistentIdResolver,
    PersistentKeyResolver, PersistentLocalBindingId, PersistentObjectValueId, PersistentPropertyId,
    PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey, SourceDeclarationIdentityError,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver;

impl PersistentIdResolver<ConeIdentity> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE]
            .into_iter()
            .find(|expected| expected.as_array() == id.as_array())
            .ok_or(ResolutionError)
    }
}

macro_rules! source_key_resolver {
    ($id:ty, $derive:path) => {
        impl PersistentKeyResolver<$id, SourceDeclarationKey> for Resolver {
            type Error = ResolutionError;

            fn resolve_key(
                &mut self,
                id: DecodedPersistentId<$id>,
            ) -> Result<std::sync::Arc<SourceDeclarationKey>, Self::Error> {
                resolve_source_key(id, $derive).map(std::sync::Arc::new)
            }
        }
    };
}

source_key_resolver!(
    PersistentAnnotationId,
    PersistentAnnotationId::from_source_declaration
);
source_key_resolver!(PersistentTypeId, PersistentTypeId::from_source_declaration);
source_key_resolver!(
    PersistentGenericTypeId,
    PersistentGenericTypeId::from_source_declaration
);
source_key_resolver!(
    PersistentObjectValueId,
    PersistentObjectValueId::from_source_object
);
source_key_resolver!(
    PersistentFunctionId,
    PersistentFunctionId::from_source_declaration
);
source_key_resolver!(
    PersistentGenericFunctionId,
    PersistentGenericFunctionId::from_source_declaration
);
source_key_resolver!(
    PersistentPropertyId,
    PersistentPropertyId::from_source_declaration
);
source_key_resolver!(
    PersistentExtensionPropertyId,
    PersistentExtensionPropertyId::from_source_declaration
);
source_key_resolver!(
    PersistentTypeAliasId,
    PersistentTypeAliasId::from_source_declaration
);

impl PersistentIdResolver<PersistentEnumVariantId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentEnumVariantId>,
    ) -> Result<PersistentEnumVariantId, Self::Error> {
        id.verify(enum_variant())
            .map_err(|_: PersistentIdMismatch<PersistentEnumVariantId>| ResolutionError)
    }
}

#[test]
fn every_binding_target_shape_round_trips_and_resolves() {
    for binding in binding_targets() {
        let key = ExportBindingKey::new(
            ConeIdentity::SINGLE_FILE,
            package(),
            identifier("VisibleName"),
            binding,
        );
        let record = CborIdentityRecord::<PersistentExportBindingId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentExportBindingId, DecodedExportBindingKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn every_local_binding_source_role_round_trips_and_resolves() {
    for source_role in [
        LocalBindingRole::Declaration,
        LocalBindingRole::ExactImport,
        LocalBindingRole::StarImport,
        LocalBindingRole::AliasImport,
    ] {
        let key = LocalBindingKey::new(
            SourceIdentity::single_file(),
            package(),
            identifier("LocalName"),
            BindingTarget::enum_variant(enum_variant()),
            source_role,
        );
        let record = CborIdentityRecord::<PersistentLocalBindingId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentLocalBindingId, DecodedLocalBindingKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn binding_resolution_rejects_namespace_role_target_mismatches() {
    let binding = BindingTarget::type_name(&plain_type()).unwrap();
    let key = ExportBindingKey::new(
        ConeIdentity::SINGLE_FILE,
        package(),
        identifier("VisibleName"),
        binding,
    );
    let mut bytes = encode(&key).unwrap();
    assert_eq!(*bytes.last().unwrap(), 1);
    *bytes.last_mut().unwrap() = 5;
    let decoded = decode_canonical::<DecodedExportBindingKey>(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(BindingIdentityResolutionError::InvalidTarget {
            namespace: BindingNamespace::Type,
            role: BindingRole::Property,
        })
    );
}

#[test]
fn function_binding_resolution_rechecks_receiver_presence() {
    let binding = BindingTarget::extension_function(&extension_function(0)).unwrap();
    let key = ExportBindingKey::new(
        ConeIdentity::SINGLE_FILE,
        package(),
        identifier("extension"),
        binding,
    );
    let mut bytes = encode(&key).unwrap();
    assert_eq!(*bytes.last().unwrap(), 4);
    *bytes.last_mut().unwrap() = 3;
    let decoded = decode_canonical::<DecodedExportBindingKey>(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(BindingIdentityResolutionError::Target(
            BindingTargetError::ExpectedOrdinaryFunction
        ))
    );
}

#[test]
fn local_binding_resolution_rejects_origin_source_mismatch() {
    let key = LocalBindingKey::new(
        SourceIdentity::single_file(),
        package(),
        identifier("LocalName"),
        BindingTarget::enum_variant(enum_variant()),
        LocalBindingRole::ExactImport,
    );
    let mut bytes = encode(&key).unwrap();
    assert_eq!(&bytes[..4], b"\xa8\x01\x58\x20");
    bytes[4..36].copy_from_slice(ConeIdentity::CORE.as_array());
    let decoded = decode_canonical::<DecodedLocalBindingKey>(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(BindingIdentityResolutionError::OriginMismatch)
    );
}

#[test]
fn binding_decoder_rejects_unknown_tags_and_incomplete_sums() {
    let namespace = decode_canonical::<BindingNamespace>(b"\x03").unwrap_err();
    assert_eq!(namespace.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let role = decode_canonical::<BindingRole>(b"\x0a").unwrap_err();
    assert_eq!(role.kind(), &WireErrorKind::UnknownTag { tag: 10 });

    let source_role = decode_canonical::<LocalBindingRole>(b"\x05").unwrap_err();
    assert_eq!(source_role.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let target = decode_canonical::<DecodedBindableEntity>(
        b"\xa2\x00\x0b\x01\x58\x20\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00",

    )
    .unwrap_err();
    assert_eq!(target.kind(), &WireErrorKind::UnknownTag { tag: 11 });

    let incomplete = decode_canonical::<DecodedBindableEntity>(b"\xa1\x00\x01").unwrap_err();
    assert_eq!(
        incomplete.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

fn binding_targets() -> Vec<BindingTarget> {
    vec![
        BindingTarget::type_name(&plain_type()).unwrap(),
        BindingTarget::type_name(&generic_type()).unwrap(),
        BindingTarget::object_value(&object_type()).unwrap(),
        BindingTarget::function(&ordinary_function(0)).unwrap(),
        BindingTarget::function(&ordinary_function(1)).unwrap(),
        BindingTarget::extension_function(&extension_function(0)).unwrap(),
        BindingTarget::extension_function(&extension_function(1)).unwrap(),
        BindingTarget::property(&property()).unwrap(),
        BindingTarget::extension_property(&extension_property()).unwrap(),
        BindingTarget::type_alias(&type_alias()).unwrap(),
        BindingTarget::annotation(&annotation()).unwrap(),
        BindingTarget::enum_variant(enum_variant()),
    ]
}

fn resolve_source_key<I: PersistentId>(
    id: DecodedPersistentId<I>,
    derive: fn(&SourceDeclarationKey) -> Result<I, SourceDeclarationIdentityError>,
) -> Result<SourceDeclarationKey, ResolutionError> {
    for key in source_declarations() {
        let Ok(expected) = derive(&key) else {
            continue;
        };
        if expected.as_array() == id.as_array() {
            return id
                .verify(expected)
                .map(|_| key)
                .map_err(|_| ResolutionError);
        }
    }
    Err(ResolutionError)
}

fn source_declarations() -> Vec<SourceDeclarationKey> {
    vec![
        plain_type(),
        generic_type(),
        object_type(),
        ordinary_function(0),
        ordinary_function(1),
        extension_function(0),
        extension_function(1),
        property(),
        extension_property(),
        type_alias(),
        annotation(),
    ]
}

fn annotation() -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        site(),
        identifier("Label"),
        SourceNominalKind::AnnotationClass,
        0,
    )
}

fn plain_type() -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(site(), identifier("PlainType"), SourceNominalKind::Class, 0)
}

fn generic_type() -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        site(),
        identifier("GenericType"),
        SourceNominalKind::Class,
        1,
    )
}

fn object_type() -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        site(),
        identifier("ObjectType"),
        SourceNominalKind::Object,
        0,
    )
}

fn ordinary_function(type_parameter_count: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        site(),
        identifier(if type_parameter_count == 0 {
            "ordinary"
        } else {
            "genericOrdinary"
        }),
        type_parameter_count,
        None,
        Vec::new(),
    )
}

fn extension_function(type_parameter_count: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        site(),
        identifier(if type_parameter_count == 0 {
            "extension"
        } else {
            "genericExtension"
        }),
        type_parameter_count,
        Some(receiver_type()),
        Vec::new(),
    )
}

fn property() -> SourceDeclarationKey {
    SourceDeclarationKey::property(site(), identifier("property"))
}

fn extension_property() -> SourceDeclarationKey {
    SourceDeclarationKey::extension_property(
        site(),
        identifier("extensionProperty"),
        0,
        receiver_type(),
    )
}

fn type_alias() -> SourceDeclarationKey {
    SourceDeclarationKey::type_alias(site(), identifier("Alias"))
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        package(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn receiver_type() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(PersistentTypeId([42; 32]))
}

fn package() -> PackagePath {
    PackagePath::from_segments(vec![identifier("pkg")])
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

const fn enum_variant() -> PersistentEnumVariantId {
    PersistentEnumVariantId([12; 32])
}
