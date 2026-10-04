use super::*;

pub(in crate::cross_cone_compile_decode::tests) fn enum_variant_callable_surface(
    cone: scoop_identity::ConeIdentity,
) -> (CanonicalHirFoundation, Vec<u8>, PersistentEnumVariantId) {
    let source =
        SourceIdentity::new(cone, NormalizedSourcePath::new("src/Choice.scoop").unwrap()).unwrap();
    let context_key = SourceContextKey::File {
        source: source.clone(),
    };
    let context =
        CborIdentityRecord::<PersistentSourceContextId, _>::from_key(context_key.clone()).unwrap();
    let origin =
        DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 6).unwrap(), &context_key)
            .unwrap();
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Choice").unwrap(),
        SourceNominalKind::Enum,
        0,
    );
    let nominal = CborIdentityRecord::<PersistentTypeId, _>::from_key(declaration).unwrap();
    let variant_key =
        EnumVariantIdentityKey::source(nominal.key(), CanonicalIdentifier::new("Only").unwrap())
            .unwrap();
    let variant = CborIdentityRecord::<PersistentEnumVariantId, _>::from_key(variant_key).unwrap();

    let mut foundation = base_hir_foundation();
    foundation
        .set_sources(vec![
            scoop_hir::SourceRecord::from_utf8(source, "enum Choice", [0, 6]).unwrap(),
        ])
        .unwrap();
    foundation.set_source_contexts(vec![context]).unwrap();
    foundation
        .set_definition_origins(vec![
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Type(nominal.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(DefinitionOriginSubject::EnumVariant(variant.id()), origin),
        ])
        .unwrap();
    foundation
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
            nominal.clone(),
        ])
        .unwrap();
    foundation.set_enum_variants(vec![variant.clone()]).unwrap();

    let nominal_record = crate::nominal_interface_fixture::public_record(
        SourceNominalId::Concrete(nominal.id()),
        PublicNominalKindV1::Enum,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        NominalSourceShapeV1::Enum(
            EnumSourceShapeV1::try_new(vec![
                EnumSourceVariantV1::try_new(
                    variant.id(),
                    EnumSourceVariantStyleV1::Unit,
                    Vec::new(),
                )
                .unwrap(),
            ])
            .unwrap(),
        ),
    )
    .unwrap();
    let callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::VariantConstructor(variant.id()),
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal.id())),
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(nominal.id()),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        scoop_hir::CanonicalPersistentIdsV1::empty(),
        Vec::new(),
    )
    .unwrap();
    (
        foundation,
        interface_with_declarations(vec![nominal_record], vec![callable], Vec::new()),
        variant.id(),
    )
}
