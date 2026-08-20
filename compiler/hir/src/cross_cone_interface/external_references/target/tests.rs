use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOwnerChain,
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, ExactTypeKey,
    FieldIdentityKey, GeneratedCallableKey, PackagePath, PersistentConstructorId,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentExtensionPropertyId, PersistentFieldId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn target_variants_have_frozen_tags_and_round_trip() {
    let fixture = Fixture::new();
    let targets = [
        ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(fixture.nominal)),
        ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(fixture.function)),
        ExternalHirTargetV1::Property(PropertyOwner::Property(fixture.property)),
        ExternalHirTargetV1::ObjectValue(fixture.object),
        ExternalHirTargetV1::TypeAlias(fixture.alias),
        ExternalHirTargetV1::Field(fixture.field),
        ExternalHirTargetV1::EnumVariantField(fixture.variant_field),
        ExternalHirTargetV1::GeneratedCallable(fixture.generated),
        ExternalHirTargetV1::Annotation(fixture.annotation),
    ];
    let mut resolver = fixture.resolver();

    for (tag, target) in (1_u8..=9).zip(targets) {
        let bytes = encode(&target).unwrap();
        assert_eq!(bytes[2], tag);
        let decoded: DecodedExternalHirTargetV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.resolve(&mut resolver), Ok(target));
    }
}

#[test]
fn decoder_rejects_unknown_tags_and_wrong_sum_length() {
    let unknown = decode_canonical::<DecodedExternalHirTargetV1>(&[0xa2, 0x00, 0x0a, 0x01, 0x00])
        .unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 10 });

    let wrong_length =
        decode_canonical::<DecodedExternalHirTargetV1>(&[0xa1, 0x00, 0x01]).unwrap_err();
    assert_eq!(
        wrong_length.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn resolution_error_preserves_the_target_kind() {
    let fixture = Fixture::new();
    let missing = nominal("Missing", SourceNominalKind::Struct);
    let target = ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(missing));
    let decoded: DecodedExternalHirTargetV1 = decode_canonical(&encode(&target).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut fixture.resolver()),
        Err(ExternalHirTargetResolutionError::Nominal(
            ResolutionError::Unknown("type")
        ))
    );
}

struct Fixture {
    nominal: PersistentTypeId,
    function: PersistentFunctionId,
    property: PersistentPropertyId,
    object: PersistentObjectValueId,
    alias: PersistentTypeAliasId,
    annotation: PersistentAnnotationId,
    field: PersistentFieldId,
    variant: PersistentEnumVariantId,
    variant_field: PersistentEnumVariantFieldId,
    generated: PersistentGeneratedCallableId,
}

impl Fixture {
    fn new() -> Self {
        let record_declaration = nominal_declaration("Record", SourceNominalKind::Struct);
        let nominal = PersistentTypeId::from_source_declaration(&record_declaration).unwrap();
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                top_level_site(),
                identifier("run"),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let property = PersistentPropertyId::from_source_declaration(
            &SourceDeclarationKey::property(top_level_site(), identifier("value")),
        )
        .unwrap();
        let object_declaration = nominal_declaration("Singleton", SourceNominalKind::Object);
        let object = PersistentObjectValueId::from_source_object(&object_declaration).unwrap();
        let alias = PersistentTypeAliasId::from_source_declaration(
            &SourceDeclarationKey::type_alias(top_level_site(), identifier("Alias")),
        )
        .unwrap();
        let field = PersistentFieldId::from_key(
            &FieldIdentityKey::source_declared(&record_declaration, identifier("field")).unwrap(),
        )
        .unwrap();
        let enum_declaration = nominal_declaration("Choice", SourceNominalKind::Enum);
        let variant_key =
            EnumVariantIdentityKey::source(&enum_declaration, identifier("Only")).unwrap();
        let variant = PersistentEnumVariantId::from_key(&variant_key).unwrap();
        let variant_field = PersistentEnumVariantFieldId::from_key(&EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ))
        .unwrap();
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
        let generated =
            PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::DerivedEquality {
                exact_owner: exact,
            })
            .unwrap();
        Self {
            nominal,
            function,
            property,
            object,
            alias,
            annotation: PersistentAnnotationId::from_source_declaration(&nominal_declaration(
                "Label",
                SourceNominalKind::AnnotationClass,
            ))
            .unwrap(),
            field,
            variant,
            variant_field,
            generated,
        }
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            nominal: self.nominal,
            function: self.function,
            property: self.property,
            object: self.object,
            alias: self.alias,
            annotation: self.annotation,
            field: self.field,
            variant: self.variant,
            variant_field: self.variant_field,
            generated: self.generated,
        }
    }
}

struct Resolver {
    nominal: PersistentTypeId,
    function: PersistentFunctionId,
    property: PersistentPropertyId,
    object: PersistentObjectValueId,
    alias: PersistentTypeAliasId,
    annotation: PersistentAnnotationId,
    field: PersistentFieldId,
    variant: PersistentEnumVariantId,
    variant_field: PersistentEnumVariantFieldId,
    generated: PersistentGeneratedCallableId,
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

resolve_fixture_identity!(PersistentTypeId, nominal, "type");
resolve_fixture_identity!(PersistentFunctionId, function, "function");
resolve_fixture_identity!(PersistentPropertyId, property, "property");
resolve_fixture_identity!(PersistentObjectValueId, object, "object value");
resolve_fixture_identity!(PersistentTypeAliasId, alias, "type alias");
resolve_fixture_identity!(PersistentAnnotationId, annotation, "annotation");
resolve_fixture_identity!(PersistentFieldId, field, "field");
resolve_fixture_identity!(PersistentEnumVariantId, variant, "enum variant");
resolve_fixture_identity!(
    PersistentEnumVariantFieldId,
    variant_field,
    "enum variant field"
);
resolve_fixture_identity!(
    PersistentGeneratedCallableId,
    generated,
    "generated callable"
);

macro_rules! reject_identity {
    ($identity:ty, $name:literal) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                _id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                Err(ResolutionError::Unknown($name))
            }
        }
    };
}

reject_identity!(PersistentGenericTypeId, "generic type");
reject_identity!(PersistentGenericFunctionId, "generic function");
reject_identity!(PersistentConstructorId, "constructor");
reject_identity!(PersistentPropertyAccessorId, "property accessor");
reject_identity!(PersistentExtensionPropertyId, "extension property");

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

fn nominal(name: &str, kind: SourceNominalKind) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(&nominal_declaration(name, kind)).unwrap()
}

fn nominal_declaration(name: &str, kind: SourceNominalKind) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(top_level_site(), identifier(name), kind, 0)
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
