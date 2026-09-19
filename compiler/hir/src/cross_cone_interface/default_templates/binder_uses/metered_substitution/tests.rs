use super::*;
use scoop_identity::{
    CallingConvention, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    Effect, NonEmptyVec, PackagePath, PersistentGenericTypeId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, ResourceKind};

fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
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
fn metered_substitution_matches_legacy_shapes_and_preserves_mapping_scope() {
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
        mapping
            .substitute_provider_type_metered(provider, &source, &mut meter(), &WirePath::root())
            .unwrap(),
        mapping.substitute_provider_type(provider, &source).unwrap()
    );
    for provider in [
        DefaultTemplateProviderShapeV1::try_new(2, 0).unwrap(),
        DefaultTemplateProviderShapeV1::try_new(0, 2).unwrap(),
    ] {
        assert_eq!(
            mapping
                .substitute_provider_type_metered(
                    provider,
                    &binder(0, 1),
                    &mut meter(),
                    &WirePath::root()
                )
                .unwrap(),
            binder(1, 9)
        );
    }
}

#[test]
fn substitution_charges_expanded_mapping_nodes_and_continues_the_actual_depth() {
    let wide = SignatureTypeKey::Tuple(NonEmptyVec::new(vec![binder(0, 9); 4096]).unwrap());
    let mapping = CanonicalBinderUseListV1::try_new(vec![wide]).unwrap();
    let provider = DefaultTemplateProviderShapeV1::try_new(0, 1).unwrap();
    let source = SignatureTypeKey::RawPointer(Box::new(binder(0, 0)));
    for (expected, limits) in [
        (
            ResourceKind::ValidationWorkUnits,
            DecodeLimits {
                validation_work_units: 8,
                ..DecodeLimits::default()
            },
        ),
        (
            ResourceKind::SemanticRecursion,
            DecodeLimits {
                semantic_recursion: 2,
                ..DecodeLimits::default()
            },
        ),
        (
            ResourceKind::DecodedNodes,
            DecodeLimits {
                decoded_nodes: 5,
                ..DecodeLimits::default()
            },
        ),
        (
            ResourceKind::DecodedEdges,
            DecodeLimits {
                decoded_edges: 5,
                ..DecodeLimits::default()
            },
        ),
        (
            ResourceKind::LogicalHeapBytes,
            DecodeLimits {
                logical_heap_bytes: 500,
                ..DecodeLimits::default()
            },
        ),
        (
            ResourceKind::SemanticTableEntries,
            DecodeLimits {
                semantic_table_entries: 100,
                ..DecodeLimits::default()
            },
        ),
    ] {
        let error = mapping
            .substitute_provider_type_metered(
                provider,
                &source,
                &mut BudgetMeter::new(limits),
                &WirePath::root(),
            )
            .unwrap_err();
        assert!(
            matches!(error, MeteredDefaultTemplateTypeSubstitutionError::Resource(ref wire) if matches!(wire.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected)),
            "{expected:?}: {error:?}"
        );
    }
    let mut shared = meter();
    let expected = mapping.substitute_provider_type(provider, &source).unwrap();
    assert_eq!(
        mapping
            .substitute_provider_type_metered(provider, &source, &mut shared, &WirePath::root())
            .unwrap(),
        expected
    );
    assert_eq!(shared.usage().decoded_nodes, 4099);
    let before = shared.usage();
    mapping
        .substitute_provider_type_metered(provider, &source, &mut shared, &WirePath::root())
        .unwrap();
    assert_eq!(shared.usage().decoded_nodes, before.decoded_nodes * 2);
}

#[test]
fn substitution_rejects_mapping_arity_and_provider_scope_without_repair() {
    let mapping = CanonicalBinderUseListV1::try_new(vec![binder(0, 1)]).unwrap();
    assert!(matches!(
        mapping.substitute_provider_type_metered(
            DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap(),
            &binder(0, 0),
            &mut meter(),
            &WirePath::root()
        ),
        Err(MeteredDefaultTemplateTypeSubstitutionError::Substitution(
            DefaultTemplateTypeSubstitutionError::MappingArity { .. }
        ))
    ));
    assert!(matches!(
        mapping.substitute_provider_type_metered(
            DefaultTemplateProviderShapeV1::try_new(0, 1).unwrap(),
            &binder(1, 0),
            &mut meter(),
            &WirePath::root()
        ),
        Err(MeteredDefaultTemplateTypeSubstitutionError::Substitution(
            DefaultTemplateTypeSubstitutionError::ProviderBinder(_)
        ))
    ));
}
