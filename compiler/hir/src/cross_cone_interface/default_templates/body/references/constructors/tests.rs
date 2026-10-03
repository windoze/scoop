use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOwnerAtom,
    DefinitionOwnerChain, EnumVariantIdentityKey, GeneratedCallableKey, PackagePath,
    PersistentConstructorId, PersistentEnumVariantId, PersistentGeneratedCallableId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn class_constructor_id_variants_have_fixed_tags_and_resolve() {
    let fixture = Fixture::new();
    let cases = [
        DefaultClassConstructorIdV1::Source(fixture.class_constructor),
        DefaultClassConstructorIdV1::Generated(fixture.generated_constructor),
    ];

    for (expected_tag, expected) in [1, 2].into_iter().zip(cases) {
        let bytes = encode(&expected).unwrap();
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultClassConstructorIdV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(expected));
    }
}

#[test]
fn constructor_reference_variants_round_trip_with_complete_owner_types() {
    let fixture = Fixture::new();
    let cases = [
        DefaultConstructorRefV1::Struct {
            declaration: fixture.struct_constructor,
            owner_type: binder(0),
        },
        DefaultConstructorRefV1::Class {
            declaration: DefaultClassConstructorIdV1::Source(fixture.class_constructor),
            owner_type: binder(1),
        },
        DefaultConstructorRefV1::Class {
            declaration: DefaultClassConstructorIdV1::Generated(fixture.generated_constructor),
            owner_type: binder(2),
        },
        DefaultConstructorRefV1::Variant {
            declaration: fixture.variant,
            owner_type: binder(3),
        },
    ];

    for (expected_tag, expected) in [1, 2, 2, 3].into_iter().zip(cases) {
        let bytes = encode(&expected).unwrap();
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultConstructorRefV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(expected));
    }
}

#[test]
fn struct_constructor_reference_has_fixed_wire() {
    let fixture = Fixture::new();
    let reference = DefaultConstructorRefV1::Struct {
        declaration: fixture.struct_constructor,
        owner_type: binder(0),
    };

    assert_eq!(
        hex(&encode(&reference).unwrap()),
        format!("a30001015820{}02a3000701000200", fixture.struct_constructor)
    );
}

#[test]
fn constructor_resolution_reports_owner_type_failure() {
    let fixture = Fixture::new();
    let reference = DefaultConstructorRefV1::Struct {
        declaration: fixture.struct_constructor,
        owner_type: SignatureTypeKey::Nominal(fixture.missing_type),
    };
    let decoded: DecodedDefaultConstructorRefV1 =
        decode_canonical(&encode(&reference).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut fixture.resolver()),
        Err(DefaultConstructorRefResolutionError::OwnerType(
            ResolutionError::Unknown("type")
        ))
    );
}

#[test]
fn constructor_decoders_reject_unknown_tags_and_non_exact_maps() {
    let mut unknown = vec![0xa2, 0x00, 0x03, 0x01, 0x58, 0x20];
    unknown.extend([0; 32]);
    let error = decode_canonical::<DecodedDefaultClassConstructorIdV1>(&unknown).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error =
        decode_canonical::<DecodedDefaultConstructorRefV1>(&[0xa1, 0x00, 0x01]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 3,
            actual: 1,
        }
    );
}

struct Fixture {
    struct_constructor: PersistentConstructorId,
    class_constructor: PersistentConstructorId,
    generated_constructor: PersistentGeneratedCallableId,
    variant: PersistentEnumVariantId,
    allowed_type: PersistentTypeId,
    missing_type: PersistentTypeId,
}

impl Fixture {
    fn new() -> Self {
        let (_, struct_type) = nominal("Record", SourceNominalKind::Struct);
        let (_, class_type) = nominal("Box", SourceNominalKind::Class);
        let (enumeration, _) = nominal("Choice", SourceNominalKind::Enum);
        let struct_constructor = constructor(struct_type);
        let class_constructor = constructor(class_type);
        let generated_constructor = PersistentGeneratedCallableId::from_key(
            &GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                constructor: class_constructor,
            },
        )
        .unwrap();
        let variant = PersistentEnumVariantId::from_key(
            &EnumVariantIdentityKey::source(&enumeration, identifier("Only")).unwrap(),
        )
        .unwrap();
        let (_, missing_type) = nominal("Missing", SourceNominalKind::Struct);
        Self {
            struct_constructor,
            class_constructor,
            generated_constructor,
            variant,
            allowed_type: struct_type,
            missing_type,
        }
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            struct_constructor: self.struct_constructor,
            class_constructor: self.class_constructor,
            generated_constructor: self.generated_constructor,
            variant: self.variant,
            allowed_type: self.allowed_type,
        }
    }
}

struct Resolver {
    struct_constructor: PersistentConstructorId,
    class_constructor: PersistentConstructorId,
    generated_constructor: PersistentGeneratedCallableId,
    variant: PersistentEnumVariantId,
    allowed_type: PersistentTypeId,
}

impl PersistentIdResolver<PersistentConstructorId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentConstructorId>,
    ) -> Result<PersistentConstructorId, Self::Error> {
        if id.as_array() == self.struct_constructor.as_array() {
            Ok(self.struct_constructor)
        } else if id.as_array() == self.class_constructor.as_array() {
            Ok(self.class_constructor)
        } else {
            Err(ResolutionError::Unknown("constructor"))
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

resolve_fixture_identity!(
    PersistentGeneratedCallableId,
    generated_constructor,
    "generated constructor"
);
resolve_fixture_identity!(PersistentEnumVariantId, variant, "enum variant");
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

fn constructor(owner: PersistentTypeId) -> PersistentConstructorId {
    PersistentConstructorId::from_source_declaration(&SourceDeclarationKey::constructor(
        owned_site(DefinitionOwnerAtom::Type(owner)),
        Vec::new(),
    ))
    .unwrap()
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
