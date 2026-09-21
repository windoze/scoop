use scoop_hir::{ExportDefaultTemplateKeyV1, SourceNominalId};
use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, CoreNativeBoundaryNominal,
    DeclarationScope, DefinitionOwnerChain, DependencyCallableDeclarationId, PackagePath,
    PersistentFunctionId, PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

pub(super) fn assert_ordinary_interfaces(
    production: &scoop_slib::ValidatedCrossConeSemanticsProduction,
) {
    let interface = production.hir_interface();
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
                vec![SignatureTypeKey::Nominal(
                    CoreNativeBoundaryNominal::Signed32.concrete_id().unwrap(),
                )],
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
