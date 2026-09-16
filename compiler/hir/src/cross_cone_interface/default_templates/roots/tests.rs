use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope,
    DecodedPersistentId, DefinitionOwnerAtom, DefinitionOwnerChain, EnumVariantIdentityKey,
    PackagePath, PersistentConstructorId, PersistentEnumVariantId, PersistentFunctionId,
    PersistentGenericFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
    PersistentPropertyId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn roots_have_fixed_tags_and_resolve_through_kind_specific_authority() {
    let fixture = Fixture::new();
    let roots = [
        PersistentLexicalRootV1::Function(fixture.function),
        PersistentLexicalRootV1::GenericFunction(fixture.generic_function),
        PersistentLexicalRootV1::Constructor(fixture.constructor),
        PersistentLexicalRootV1::EnumVariantConstructor(fixture.variant),
    ];
    let mut resolver = fixture.resolver();

    for (index, root) in roots.into_iter().enumerate() {
        let bytes = encode(&root).unwrap();
        assert_eq!(bytes[2], u8::try_from(index + 1).unwrap());
        let decoded: DecodedPersistentLexicalRootV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(decoded.resolve(&mut resolver), Ok(root));
        assert_eq!(
            PersistentLexicalRootV1::try_from(root.declaration()),
            Ok(root)
        );
    }
}

#[test]
fn function_root_has_fixed_wire() {
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        top_level_site(),
        identifier("collect"),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let root = PersistentLexicalRootV1::Function(function);

    assert_eq!(
        hex(&encode(&root).unwrap()),
        "a2000101582012104f4f6e246d6533e24859a416718ca33e20fa88c78d1285c694aa56c37664"
    );
}

#[test]
fn property_accessors_cannot_be_lexical_roots() {
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        top_level_site(),
        identifier("value"),
    ))
    .unwrap();
    let accessor = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        PropertyOwner::Property(property),
        AccessorRole::Getter,
    ))
    .unwrap();

    assert_eq!(
        PersistentLexicalRootV1::try_from(CallableTemplateOrigin::Accessor(accessor)),
        Err(PersistentLexicalRootBuildError::PropertyAccessor(accessor))
    );
}

#[test]
fn decoder_rejects_unknown_tags_and_non_exact_maps() {
    let mut unknown_bytes = vec![0xa2, 0x00, 0x05, 0x01, 0x58, 0x20];
    unknown_bytes.extend([0; 32]);
    let unknown =
        decode_canonical::<DecodedPersistentLexicalRootV1>(&unknown_bytes, DecodeLimits::default())
            .unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let short = decode_canonical::<DecodedPersistentLexicalRootV1>(
        &[0xa1, 0x00, 0x01],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        short.kind(),
        WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    ));
}

struct Fixture {
    function: PersistentFunctionId,
    generic_function: PersistentGenericFunctionId,
    constructor: PersistentConstructorId,
    variant: PersistentEnumVariantId,
}

impl Fixture {
    fn new() -> Self {
        let function_key = SourceDeclarationKey::function(
            top_level_site(),
            identifier("plain"),
            0,
            None,
            Vec::new(),
        );
        let generic_function_key = SourceDeclarationKey::function(
            top_level_site(),
            identifier("generic"),
            1,
            None,
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
        );
        let structure = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Value"),
            SourceNominalKind::Struct,
            0,
        );
        let structure_id =
            scoop_identity::PersistentTypeId::from_source_declaration(&structure).unwrap();
        let constructor_key = SourceDeclarationKey::constructor(
            owned_site(DefinitionOwnerAtom::Type(structure_id)),
            Vec::new(),
        );
        let enumeration = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Choice"),
            SourceNominalKind::Enum,
            0,
        );
        let variant_key = EnumVariantIdentityKey::source(&enumeration, identifier("Only")).unwrap();
        Self {
            function: PersistentFunctionId::from_source_declaration(&function_key).unwrap(),
            generic_function: PersistentGenericFunctionId::from_source_declaration(
                &generic_function_key,
            )
            .unwrap(),
            constructor: PersistentConstructorId::from_source_declaration(&constructor_key)
                .unwrap(),
            variant: PersistentEnumVariantId::from_key(&variant_key).unwrap(),
        }
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            function: self.function,
            generic_function: self.generic_function,
            constructor: self.constructor,
            variant: self.variant,
        }
    }
}

struct Resolver {
    function: PersistentFunctionId,
    generic_function: PersistentGenericFunctionId,
    constructor: PersistentConstructorId,
    variant: PersistentEnumVariantId,
}

macro_rules! resolve_identity {
    ($identity:ty, $field:ident) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                id.verify(self.$field).map_err(|_| ResolutionError)
            }
        }
    };
}

resolve_identity!(PersistentFunctionId, function);
resolve_identity!(PersistentGenericFunctionId, generic_function);
resolve_identity!(PersistentConstructorId, constructor);
resolve_identity!(PersistentEnumVariantId, variant);

#[derive(Debug, Eq, PartialEq)]
struct ResolutionError;

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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
