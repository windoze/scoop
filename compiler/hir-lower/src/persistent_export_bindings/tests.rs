use std::collections::BTreeMap;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{
    BindableEntity, BindingRole, CanonicalIdentifier, DeclarationName, ExportBindingKey,
    PackagePath, PersistentExportBindingId,
};

use crate::tests::{
    class_decl, core_file, core_source_identity, extension_expr, file, fun, fun_expr,
    generic_struct_decl, ident, int_lit, method_expr, sp, test_source_identity, ty_named,
};

fn explicit_public() -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    }
}

fn public(mut declaration: ast::Decl) -> ast::Decl {
    match &mut declaration {
        ast::Decl::Global(declaration) => declaration.visibility = explicit_public(),
        ast::Decl::Function(declaration) => declaration.visibility = explicit_public(),
        ast::Decl::TypeAlias(declaration) => declaration.visibility = explicit_public(),
        ast::Decl::Struct(declaration) => declaration.visibility = explicit_public(),
        ast::Decl::Enum(declaration) => declaration.visibility = explicit_public(),
        ast::Decl::Class(declaration) => declaration.visibility = explicit_public(),
        ast::Decl::Interface(declaration) => declaration.visibility = explicit_public(),
        ast::Decl::Object(declaration) => declaration.visibility = explicit_public(),
    }
    declaration
}

fn package(mut source: ast::SourceFile, name: &str) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: ast::QualifiedNameSyntax {
            first: ident(name),
            rest: Vec::new(),
            span: sp(),
        },
        span: sp(),
    };
    source
}

fn object(name: &str) -> ast::Decl {
    ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    })
}

fn property(name: &str, receiver: Option<ast::TypeRef>) -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: receiver,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(int_lit(1))),
                span: sp(),
            }),
            setter: None,
        }),
        span: sp(),
    })
}

fn source_file(unrelated_prefix: bool) -> ast::SourceFile {
    let mut member = method_expr("member", Vec::new(), Some(ty_named("Int")), int_lit(1));
    member.visibility = explicit_public();
    let mut declarations = vec![
        public(generic_struct_decl("Box", vec!["T"], Vec::new())),
        public(class_decl(
            ast::ClassModifier::Final,
            "Holder",
            Vec::new(),
            None,
            Vec::new(),
            vec![member],
        )),
        public(object("Registry")),
        public(fun_expr(
            "ping",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        )),
        public(extension_expr(
            ty_named("Int"),
            "tag",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(2),
        )),
        public(property("answer", None)),
        public(property("extensionAnswer", Some(ty_named("Int")))),
        public(ast::Decl::TypeAlias(ast::TypeAliasDecl {
            visibility: ast::VisibilitySyntax::Omitted,
            name: ident("Number"),
            target: ty_named("Int"),
            span: sp(),
        })),
    ];
    if unrelated_prefix {
        declarations.insert(0, fun("unrelated", Vec::new()));
    }
    package(file(declarations), "api")
}

fn lower(unrelated_prefix: bool) -> hir::Output {
    let core = core_file();
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(
            test_source_identity("src/bindings.scoop"),
            source_file(unrelated_prefix),
        ),
        Vec::new(),
    ))
    .expect("export binding test source is valid");
    let input = crate::LegacyCombinedSources::try_new(
        vec![crate::ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::CurrentSourceDetails {
            display_locator: "/checkout/bindings.scoop",
            source_text: "",
        },
    )
    .expect("export binding test sources are valid");
    crate::lower_combined_sources(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("export binding fixture lowers")
}

fn current_records(output: &hir::Output) -> Vec<(PersistentExportBindingId, ExportBindingKey)> {
    let cone = test_source_identity("src/bindings.scoop").cone();
    output
        .export
        .export_binding_identities
        .iter()
        .filter(|record| record.key().exporter() == cone)
        .map(|record| (record.id(), record.key().clone()))
        .collect()
}

#[test]
fn direct_public_package_bindings_use_typed_targets_and_ignore_members() {
    let first = lower(false);
    let records = current_records(&first);
    assert_eq!(records.len(), 9);
    let expected_package =
        PackagePath::from_segments(vec![CanonicalIdentifier::new("api").unwrap()]);
    assert!(
        records
            .iter()
            .all(|(_, key)| key.package() == &expected_package)
    );
    assert!(
        !records
            .iter()
            .any(|(_, key)| key.name().as_str() == "member")
    );

    let mut roles = BTreeMap::new();
    for (_, key) in &records {
        *roles.entry(key.role()).or_insert(0_usize) += 1;
    }
    assert_eq!(roles.get(&BindingRole::TypeName), Some(&3));
    assert_eq!(roles.get(&BindingRole::ObjectValue), Some(&1));
    assert_eq!(roles.get(&BindingRole::Function), Some(&1));
    assert_eq!(roles.get(&BindingRole::ExtensionFunction), Some(&1));
    assert_eq!(roles.get(&BindingRole::Property), Some(&1));
    assert_eq!(roles.get(&BindingRole::ExtensionProperty), Some(&1));
    assert_eq!(roles.get(&BindingRole::TypeAlias), Some(&1));
    assert!(
        records
            .iter()
            .any(|(_, key)| matches!(key.target(), BindableEntity::GenericType(_)))
    );
    assert!(
        records
            .iter()
            .any(|(_, key)| matches!(key.target(), BindableEntity::ObjectValue(_)))
    );

    assert_eq!(records, current_records(&lower(true)));
}

#[test]
fn export_binding_relation_rejects_duplicate_public_surface_entries() {
    let output = lower(false);
    let module = &output.export;
    let mut surface = module.public_surface.clone();
    let ping = surface
        .functions
        .iter()
        .copied()
        .find(|function| {
            matches!(
                module.function_identities[*function]
                    .source_identity()
                    .map(|identity| identity.declaration().name()),
                Some(DeclarationName::Named(name)) if name.as_str() == "ping"
            )
        })
        .expect("public ping function");
    surface.functions.push(ping);

    let result =
        hir::HirExportBindingIdentities::from_public_surface(hir::HirExportBindingIdentityInputs {
            surface: &surface,
            structs: &module.structs,
            enums: &module.enums,
            classes: &module.classes,
            interfaces: &module.interfaces,
            objects: &module.objects,
            singleton_values: &module.singleton_values,
            functions: &module.functions,
            properties: &module.properties,
            type_aliases: &module.type_aliases,
            nominal_identities: &module.nominal_identities,
            object_value_identities: &module.object_value_identities,
            function_identities: &module.function_identities,
            property_identities: &module.property_identities,
            type_alias_identities: &module.type_alias_identities,
        });
    assert!(matches!(
        result,
        Err(hir::HirExportBindingIdentityError::DuplicateIdentity { .. })
    ));
}
