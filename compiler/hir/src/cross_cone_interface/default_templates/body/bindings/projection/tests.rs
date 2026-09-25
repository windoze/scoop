use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOwnerChain,
    FieldIdentityKey, PackagePath, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentFieldId, PersistentGenericTypeId, PersistentIdResolver, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::super::test_support::{binder, hex};
use super::*;

#[test]
fn projection_variants_have_fixed_wire_and_round_trip() {
    let tuple = DefaultBindingProjectionV1::tuple_index(42);
    assert_eq!(hex(&encode(&tuple).unwrap()), "a2000101182a");
    let decoded: DecodedDefaultBindingProjectionV1 =
        decode_canonical(&encode(&tuple).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut Resolver::rejecting()), Ok(tuple));

    let fixture = Fixture::new();
    let structure = DefaultBindingProjectionV1::struct_field(fixture.field, binder(0));
    let bytes = encode(&structure).unwrap();
    assert_eq!(bytes[2], 2);
    let decoded: DecodedDefaultBindingProjectionV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(structure));
}

#[test]
fn projection_constructor_and_reader_reject_non_struct_fields() {
    assert_eq!(
        DefaultBindingProjectionV1::try_struct_field(DefaultFieldRefV1::Tuple {
            declaration_index: 1,
        }),
        Err(DefaultBindingProjectionBuildError::ExpectedStructField)
    );

    let decoded: DecodedDefaultBindingProjectionV1 =
        decode_canonical(&[0xa2, 0x00, 0x02, 0x01, 0xa2, 0x00, 0x02, 0x01, 0x01]).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver::rejecting()),
        Err(DefaultBindingProjectionResolutionError::Shape(
            DefaultBindingProjectionBuildError::ExpectedStructField
        ))
    );
}

#[test]
fn projection_decoder_rejects_unknown_tags_and_non_exact_maps() {
    let error =
        decode_canonical::<DecodedDefaultBindingProjectionV1>(&[0xa2, 0x00, 0x03, 0x01, 0x00])
            .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error =
        decode_canonical::<DecodedDefaultBindingProjectionV1>(&[0xa1, 0x00, 0x01]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

struct Fixture {
    field: PersistentFieldId,
}

impl Fixture {
    fn new() -> Self {
        let structure = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Record"),
            SourceNominalKind::Struct,
            0,
        );
        let field = PersistentFieldId::from_key(
            &FieldIdentityKey::source_declared(&structure, identifier("value")).unwrap(),
        )
        .unwrap();
        Self { field }
    }

    const fn resolver(&self) -> Resolver {
        Resolver {
            field: Some(self.field),
        }
    }
}

struct Resolver {
    field: Option<PersistentFieldId>,
}

impl Resolver {
    const fn rejecting() -> Self {
        Self { field: None }
    }
}

impl PersistentIdResolver<PersistentFieldId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFieldId>,
    ) -> Result<PersistentFieldId, Self::Error> {
        id.verify(self.field.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

macro_rules! reject_identity {
    ($identity:ty) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                _id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                Err(ResolutionError)
            }
        }
    };
}

reject_identity!(PersistentTypeId);
reject_identity!(PersistentGenericTypeId);
reject_identity!(PersistentEnumVariantId);
reject_identity!(PersistentEnumVariantFieldId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent")
    }
}

impl std::error::Error for ResolutionError {}

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
