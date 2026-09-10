use la_arena::Arena;
use scoop_hir as hir;

fn test_source_nominal_identity(
    name: &str,
    kind: scoop_identity::SourceNominalKind,
    type_parameter_count: usize,
) -> hir::HirNominalIdentity {
    let source = scoop_identity::SourceIdentity::single_file();
    let site = scoop_identity::SourceDeclarationSite::new(
        source.cone(),
        scoop_identity::PackagePath::root(),
        scoop_identity::DefinitionOwnerChain::top_level(),
        scoop_identity::DeclarationScope::SourceScoped(source),
    )
    .expect("the single-file test declaration site is valid");
    let name = scoop_identity::CanonicalIdentifier::new(name)
        .expect("synthetic test declaration names follow the source identifier grammar");
    hir::HirNominalIdentity::from_source_declaration(scoop_identity::SourceDeclarationKey::nominal(
        site,
        name,
        kind,
        u32::try_from(type_parameter_count)
            .expect("test declaration type parameter count fits u32"),
    ))
    .expect("the test source nominal identity is valid")
}

/// The MIR unit harness assembles nominal arenas directly rather than parsing
/// source declarations. Give each fixture entity a valid, arena-aligned typed
/// identity without treating its ABI-oriented display name as source syntax.
pub(in crate::tests) fn test_nominal_identities_without_objects(
    structs: &Arena<hir::StructDecl>,
    enums: &Arena<hir::EnumDecl>,
    classes: &Arena<hir::ClassDecl>,
    interfaces: &Arena<hir::InterfaceDecl>,
) -> hir::HirNominalIdentities {
    let struct_identities = structs
        .iter()
        .map(|(id, declaration)| {
            test_source_nominal_identity(
                &format!("TestStruct{}", id.into_raw().into_u32()),
                scoop_identity::SourceNominalKind::Struct,
                declaration.type_params.len(),
            )
        })
        .collect();
    let enum_identities = enums
        .iter()
        .map(|(id, declaration)| {
            test_source_nominal_identity(
                &format!("TestEnum{}", id.into_raw().into_u32()),
                scoop_identity::SourceNominalKind::Enum,
                declaration.type_params.len(),
            )
        })
        .collect();
    let class_identities = classes
        .iter()
        .map(|(id, declaration)| {
            test_source_nominal_identity(
                &format!("TestClass{}", id.into_raw().into_u32()),
                scoop_identity::SourceNominalKind::Class,
                declaration.type_params.len(),
            )
        })
        .collect();
    let interface_identities = interfaces
        .iter()
        .map(|(id, declaration)| {
            test_source_nominal_identity(
                &format!("TestInterface{}", id.into_raw().into_u32()),
                scoop_identity::SourceNominalKind::Interface,
                declaration.type_params.len(),
            )
        })
        .collect();
    hir::HirNominalIdentities::checked(
        structs,
        struct_identities,
        enums,
        enum_identities,
        classes,
        class_identities,
        interfaces,
        interface_identities,
        &Arena::new(),
        Vec::new(),
    )
    .expect("the MIR test fixture provides one identity per nominal declaration")
}
