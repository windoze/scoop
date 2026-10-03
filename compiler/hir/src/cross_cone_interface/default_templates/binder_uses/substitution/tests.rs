use super::*;
use scoop_identity::{
    CallingConvention, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    Effect, NonEmptyVec, PackagePath, PersistentGenericTypeId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}

fn nominal_key(arity: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Nominal").unwrap(),
        SourceNominalKind::Class,
        arity,
    )
}

#[test]
fn substitution_preserves_composite_shapes_and_mapping_scope() {
    let mapping = CanonicalBinderUseListV1::try_new(vec![binder(0, 7), binder(1, 9)]).unwrap();
    let provider = DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap();
    let source = SignatureTypeKey::Tuple(
        NonEmptyVec::new(vec![
            SignatureTypeKey::Nominal(
                PersistentTypeId::from_source_declaration(&nominal_key(0)).unwrap(),
            ),
            SignatureTypeKey::NominalApplication {
                origin: PersistentGenericTypeId::from_source_declaration(&nominal_key(1)).unwrap(),
                arguments: NonEmptyVec::from_first(binder(1, 0), []),
            },
            SignatureTypeKey::Function {
                effect: Effect::Suspend,
                parameters: vec![binder(1, 0)],
                result: Box::new(binder(0, 0)),
            },
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters: vec![binder(0, 0)],
                result: Box::new(binder(1, 0)),
            },
            SignatureTypeKey::RawPointer(Box::new(binder(0, 0))),
        ])
        .unwrap(),
    );
    assert_eq!(
        mapping.substitute_provider_type(provider, &source).unwrap(),
        SignatureTypeKey::Tuple(
            NonEmptyVec::new(vec![
                SignatureTypeKey::Nominal(
                    PersistentTypeId::from_source_declaration(&nominal_key(0)).unwrap()
                ),
                SignatureTypeKey::NominalApplication {
                    origin: PersistentGenericTypeId::from_source_declaration(&nominal_key(1))
                        .unwrap(),
                    arguments: NonEmptyVec::from_first(binder(0, 7), [])
                },
                SignatureTypeKey::Function {
                    effect: Effect::Suspend,
                    parameters: vec![binder(0, 7)],
                    result: Box::new(binder(1, 9))
                },
                SignatureTypeKey::NativeFunctionPointer {
                    calling_convention: CallingConvention::C,
                    parameters: vec![binder(1, 9)],
                    result: Box::new(binder(0, 7))
                },
                SignatureTypeKey::RawPointer(Box::new(binder(1, 9))),
            ])
            .unwrap()
        )
    );
    for provider in [
        DefaultTemplateProviderShapeV1::try_new(2, 0).unwrap(),
        DefaultTemplateProviderShapeV1::try_new(0, 2).unwrap(),
    ] {
        assert_eq!(
            mapping
                .substitute_provider_type(provider, &binder(0, 1))
                .unwrap(),
            binder(1, 9)
        );
    }
}

#[test]
fn substitution_rejects_mapping_arity_and_provider_scope_without_repair() {
    let mapping = CanonicalBinderUseListV1::try_new(vec![binder(0, 1)]).unwrap();
    assert!(matches!(
        mapping.substitute_provider_type(
            DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap(),
            &binder(0, 0)
        ),
        Err(DefaultTemplateTypeSubstitutionError::MappingArity { .. })
    ));
    assert!(matches!(
        mapping.substitute_provider_type(
            DefaultTemplateProviderShapeV1::try_new(0, 1).unwrap(),
            &binder(1, 0)
        ),
        Err(DefaultTemplateTypeSubstitutionError::ProviderBinder(_))
    ));
}
