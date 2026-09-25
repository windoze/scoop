use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOwnerAtom,
    DefinitionOwnerChain, EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey,
    FieldIdentityKey, PackagePath, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentFieldId, PersistentGenericTypeId, PersistentIdResolver, PersistentPropertyId,
    PersistentTypeId, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn enum_member_references_round_trip_with_owner_type() {
    let fixture = Fixture::new();
    let variant = DefaultEnumVariantRefV1::new(fixture.variant, binder(0));
    let field = DefaultEnumVariantFieldRefV1::new(fixture.variant_field, binder(1));

    let variant_bytes = encode(&variant).unwrap();
    let decoded: DecodedDefaultEnumVariantRefV1 = decode_canonical(&variant_bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(variant));

    let field_bytes = encode(&field).unwrap();
    let decoded: DecodedDefaultEnumVariantFieldRefV1 = decode_canonical(&field_bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(field));
}

#[test]
fn field_reference_variants_have_fixed_tags_and_round_trip() {
    let fixture = Fixture::new();
    let cases = [
        DefaultFieldRefV1::Struct {
            declaration: fixture.struct_field,
            owner_type: binder(0),
        },
        DefaultFieldRefV1::Tuple {
            declaration_index: 42,
        },
        DefaultFieldRefV1::Class {
            declaration: fixture.class_field,
            owner_type: binder(1),
        },
    ];

    for (expected_tag, expected) in [1, 2, 3].into_iter().zip(cases) {
        let bytes = encode(&expected).unwrap();
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultFieldRefV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(expected));
    }
}

#[test]
fn tuple_field_has_fixed_wire() {
    let field = DefaultFieldRefV1::Tuple {
        declaration_index: 42,
    };
    assert_eq!(hex(&encode(&field).unwrap()), "a2000201182a");
}

#[test]
fn field_resolution_distinguishes_owner_type_failures() {
    let fixture = Fixture::new();
    let field = DefaultFieldRefV1::Class {
        declaration: fixture.class_field,
        owner_type: SignatureTypeKey::Nominal(fixture.missing_type),
    };
    let decoded: DecodedDefaultFieldRefV1 = decode_canonical(&encode(&field).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut fixture.resolver()),
        Err(DefaultFieldRefResolutionError::ClassOwnerType(
            ResolutionError::Unknown("type")
        ))
    );
}

#[test]
fn field_decoder_rejects_unknown_tags_and_variant_specific_shapes() {
    let error =
        decode_canonical::<DecodedDefaultFieldRefV1>(&[0xa2, 0x00, 0x04, 0x01, 0x00]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 4 });

    let error =
        decode_canonical::<DecodedDefaultFieldRefV1>(&[0xa3, 0x00, 0x02, 0x01, 0x00, 0x02, 0x00])
            .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 3,
        }
    );
}

struct Fixture {
    struct_field: PersistentFieldId,
    class_field: PersistentFieldId,
    variant: PersistentEnumVariantId,
    variant_field: PersistentEnumVariantFieldId,
    allowed_type: PersistentTypeId,
    missing_type: PersistentTypeId,
}

impl Fixture {
    fn new() -> Self {
        let (structure, struct_type) = nominal("Record", SourceNominalKind::Struct);
        let (class, class_type) = nominal("Box", SourceNominalKind::Class);
        let (enumeration, _) = nominal("Choice", SourceNominalKind::Enum);
        let struct_field = PersistentFieldId::from_key(
            &FieldIdentityKey::source_declared(&structure, identifier("value")).unwrap(),
        )
        .unwrap();
        let property =
            PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
                owned_site(DefinitionOwnerAtom::Type(class_type)),
                identifier("stored"),
            ))
            .unwrap();
        let class_field = PersistentFieldId::from_key(
            &FieldIdentityKey::source_property_backing(&class, property).unwrap(),
        )
        .unwrap();
        let variant_key = EnumVariantIdentityKey::source(&enumeration, identifier("Only")).unwrap();
        let variant = PersistentEnumVariantId::from_key(&variant_key).unwrap();
        let variant_field = PersistentEnumVariantFieldId::from_key(&EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ))
        .unwrap();
        let (_, missing_type) = nominal("Missing", SourceNominalKind::Struct);
        Self {
            struct_field,
            class_field,
            variant,
            variant_field,
            allowed_type: struct_type,
            missing_type,
        }
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            struct_field: self.struct_field,
            class_field: self.class_field,
            variant: self.variant,
            variant_field: self.variant_field,
            allowed_type: self.allowed_type,
        }
    }
}

struct Resolver {
    struct_field: PersistentFieldId,
    class_field: PersistentFieldId,
    variant: PersistentEnumVariantId,
    variant_field: PersistentEnumVariantFieldId,
    allowed_type: PersistentTypeId,
}

impl PersistentIdResolver<PersistentFieldId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFieldId>,
    ) -> Result<PersistentFieldId, Self::Error> {
        if id.as_array() == self.struct_field.as_array() {
            Ok(self.struct_field)
        } else if id.as_array() == self.class_field.as_array() {
            Ok(self.class_field)
        } else {
            Err(ResolutionError::Unknown("field"))
        }
    }
}

macro_rules! resolve_fixture_identity {
    ($identity:ty, $field:ident, $name:literal) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                id.verify(self.$field)
                    .map_err(|_| ResolutionError::Unknown($name))
            }
        }
    };
}

resolve_fixture_identity!(PersistentEnumVariantId, variant, "enum variant");
resolve_fixture_identity!(
    PersistentEnumVariantFieldId,
    variant_field,
    "enum variant field"
);
resolve_fixture_identity!(PersistentTypeId, allowed_type, "type");

impl PersistentIdResolver<PersistentGenericTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        _id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err(ResolutionError::Unknown("generic type"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolutionError {
    Unknown(&'static str),
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(kind) => write!(formatter, "unknown {kind}"),
        }
    }
}

impl std::error::Error for ResolutionError {}

fn nominal(name: &str, kind: SourceNominalKind) -> (SourceDeclarationKey, PersistentTypeId) {
    let declaration = SourceDeclarationKey::nominal(top_level_site(), identifier(name), kind, 0);
    let id = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    (declaration, id)
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

const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
