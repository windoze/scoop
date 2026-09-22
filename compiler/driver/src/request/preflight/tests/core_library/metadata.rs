use scoop_hir::{ExportDefaultTemplateKeyV1, SourceNominalId};
use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, DependencyCallableDeclarationId, PackagePath, PersistentFunctionId,
    PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

mod abi;

pub(super) fn assert_ordinary_interfaces(
    production: &scoop_slib::ValidatedCrossConeSemanticsProduction,
) {
    abi::assert_abis(production);
    let interface = production.hir_interface();
    for kind in scoop_hir::intrinsic_function_kinds()
        .into_iter()
        .filter(|kind| kind.integer_gc_effect().is_some())
    {
        let implementation = scoop_hir::CallableImplementationV1::Intrinsic(kind);
        let records = interface
            .callable_interfaces()
            .records()
            .iter()
            .filter(|record| record.effects().implementation() == implementation)
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 1, "{kind:?}");
        assert!(
            interface
                .source_interfaces()
                .get(records[0].declaration())
                .is_some()
        );
    }
    let value = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site(),
        name("UserCoreValue"),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap();
    assert!(
        interface
            .nominal_interfaces()
            .get(SourceNominalId::Concrete(value))
            .is_some()
    );
    let alias = PersistentTypeAliasId::from_source_declaration(&SourceDeclarationKey::type_alias(
        site(),
        name("UserCoreAlias"),
    ))
    .unwrap();
    assert!(interface.type_aliases().get(alias).is_some());
    let constant = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        site(),
        name("USER_CORE_DEFAULT"),
    ))
    .unwrap();
    assert!(interface.constants().get(constant).is_some());

    for function_name in [
        "userCoreOffset",
        "userCoreWithDefault",
        "userCoreDefaultChain",
    ] {
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                site(),
                name(function_name),
                0,
                None,
                vec![SignatureTypeKey::Nominal(intrinsic_type(
                    production,
                    scoop_hir::IntrinsicTypeKind::Integer(scoop_hir::IntegerKind::SIGNED_32),
                ))],
            ))
            .unwrap();
        let declaration = CallableTemplateOrigin::Function(function);
        assert!(interface.callable_interfaces().get(declaration).is_some());
        assert!(interface.source_interfaces().get(declaration).is_some());
        let exported = DependencyCallableDeclarationId::Function(function);
        assert!(production.mir_cross_cone().export(exported).is_some());
        assert!(production.lir_cross_cone().export(exported).is_some());
        if function_name != "userCoreOffset" {
            assert!(
                interface
                    .default_templates()
                    .get(ExportDefaultTemplateKeyV1::new(declaration, 0))
                    .is_some()
            );
        }
    }
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn name(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

fn intrinsic_type(
    production: &scoop_slib::ValidatedCrossConeSemanticsProduction,
    family: scoop_hir::IntrinsicTypeKind,
) -> PersistentTypeId {
    production
        .hir_interface()
        .nominal_interfaces()
        .records()
        .iter()
        .find_map(
            |record| match (record.source_shape(), record.declaration()) {
                (
                    scoop_hir::NominalSourceShapeV1::Intrinsic(representation),
                    SourceNominalId::Concrete(id),
                ) if representation.family() == family => Some(id),
                _ => None,
            },
        )
        .expect("the actual source interface retains the intrinsic declaration")
}
