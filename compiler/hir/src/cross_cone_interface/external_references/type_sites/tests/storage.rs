use super::*;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, EnumVariantFieldKey,
    EnumVariantFieldSelector, EnumVariantIdentityKey, FieldIdentityKey, InitializationUnitKey,
    PackagePath, PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId,
    PersistentInitializationUnitId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

#[test]
fn semantic_storage_sites_keep_distinct_tags_and_typed_identity_domains() {
    let fixture = Fixture::new();
    let site = SourceDeclarationSite::new(
        fixture.current,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let structure = SourceDeclarationKey::nominal(
        site.clone(),
        CanonicalIdentifier::new("Storage").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let field = PersistentFieldId::from_key(
        &FieldIdentityKey::source_declared(&structure, CanonicalIdentifier::new("field").unwrap())
            .unwrap(),
    )
    .unwrap();
    let enumeration = SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new("Payload").unwrap(),
        SourceNominalKind::Enum,
        0,
    );
    let variant = PersistentEnumVariantId::from_key(
        &EnumVariantIdentityKey::source(&enumeration, CanonicalIdentifier::new("Value").unwrap())
            .unwrap(),
    )
    .unwrap();
    let payload = PersistentEnumVariantFieldId::from_key(&EnumVariantFieldKey::new(
        variant,
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    ))
    .unwrap();
    let root = fixture.site(0, vec![0]).unwrap().position().root;
    let unit = PersistentInitializationUnitId::from_key(&InitializationUnitKey::Object(
        scoop_identity::CoreBuiltinNominal::Any
            .identity_record()
            .id(),
    ))
    .unwrap();
    for (tag, original) in [
        (
            6,
            HirDependencyTypeSiteV1::FieldStorage {
                owner: scoop_identity::PersistentExactTypeId::from_key(
                    &scoop_identity::ExactTypeKey::Nominal(
                        scoop_identity::PersistentTypeId::from_source_declaration(&structure)
                            .unwrap(),
                    ),
                )
                .unwrap(),
                field,
                exact: fixture.unit,
            },
        ),
        (
            7,
            HirDependencyTypeSiteV1::EnumVariantFieldStorage {
                owner: scoop_identity::PersistentExactTypeId::from_key(
                    &scoop_identity::ExactTypeKey::Nominal(
                        scoop_identity::PersistentTypeId::from_source_declaration(&enumeration)
                            .unwrap(),
                    ),
                )
                .unwrap(),
                field: payload,
                exact: fixture.unit,
            },
        ),
        (
            8,
            HirDependencyTypeSiteV1::ConstructorInitializerResult {
                constructor: root,
                exact: fixture.unit,
            },
        ),
        (
            9,
            HirDependencyTypeSiteV1::InitializationCycleMessage {
                unit,
                exact: fixture.unit,
            },
        ),
    ] {
        let bytes = encode(&original).unwrap();
        let fields = if matches!(tag, 6 | 7) { 4 } else { 3 };
        assert_eq!(&bytes[..3], &[0xa0 + fields, 0, tag]);
        let raw: DecodedHirDependencyTypeSiteV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&raw).unwrap(), bytes);
        if tag == 8 {
            assert_eq!(raw.resolve(&mut fixture.graph()).unwrap(), original);
        } else {
            assert!(matches!(
                raw.resolve(&mut fixture.graph()),
                Err(HirDependencyTypeSiteResolutionError::Identity(_))
            ));
        }
        for length in [fields - 1, fields + 1] {
            let mut wrong = bytes.clone();
            wrong[0] = 0xa0 + length;
            assert!(decode_canonical::<DecodedHirDependencyTypeSiteV1>(&wrong).is_err());
        }
    }
    assert!(
        decode_canonical::<DecodedHirDependencyTypeSiteV1>(&[0xa3, 0, 10, 1, 0, 2, 0]).is_err()
    );
}
