use super::*;

fn class_source(
    name: &str,
    type_params: Vec<&str>,
    members: Vec<ast::ClassMember>,
) -> ast::ClassDecl {
    let Decl::Class(mut declaration) = class_decl(
        ast::ClassModifier::Final,
        name,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        unreachable!("class builder returns a class")
    };
    declaration.type_params = type_params.into_iter().map(type_param).collect();
    declaration.members = members;
    declaration
}

fn nested_class(declaration: ast::ClassDecl) -> ast::ClassMember {
    ast::ClassMember::Nested(Box::new(ast::NestedNominalDecl::Class(Box::new(
        declaration,
    ))))
}

fn qualified_type(path: &[&str]) -> TypeRef {
    TypeRef {
        kind: TypeRefKind::Qualified {
            path: path.iter().map(|name| ident(name)).collect(),
            arguments: Vec::new(),
        },
        span: sp(),
    }
}

#[test]
fn static_nested_nominals_keep_typed_owners_in_both_hir_graphs() {
    let first = class_source(
        "First",
        vec!["T"],
        vec![nested_class(class_source("Nested", Vec::new(), Vec::new()))],
    );
    let second = class_source(
        "Second",
        Vec::new(),
        vec![nested_class(class_source("Nested", Vec::new(), Vec::new()))],
    );
    let output = lower_user_output(file(vec![
        Decl::Class(first),
        Decl::Class(second),
        fun("main", Vec::new()),
    ]))
    .expect("same-named nested declarations belong to distinct typed owners");

    let first_id = output
        .export
        .classes
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "First").then_some(id))
        .expect("First class");
    let second_id = output
        .export
        .classes
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "Second").then_some(id))
        .expect("Second class");
    let nested = output
        .export
        .classes
        .iter()
        .filter(|(_, declaration)| declaration.name == "Nested")
        .map(|(id, declaration)| (id, declaration.owner))
        .collect::<Vec<_>>();
    assert_eq!(nested.len(), 2);
    assert!(
        nested
            .iter()
            .any(|(_, owner)| { *owner == Some(hir::NominalOwner::Class(first_id)) })
    );
    assert!(
        nested
            .iter()
            .any(|(_, owner)| { *owner == Some(hir::NominalOwner::Class(second_id)) })
    );
    assert_ne!(nested[0].0, nested[1].0);

    let local_nested = output
        .local
        .classes
        .iter()
        .filter(|(_, declaration)| declaration.name.ends_with(".Nested"))
        .collect::<Vec<_>>();
    assert_eq!(local_nested.len(), 2);
    assert!(
        local_nested
            .iter()
            .all(|(_, declaration)| declaration.owner.is_some())
    );
}

#[test]
fn static_nested_declaration_cannot_capture_outer_type_parameter() {
    let Decl::Class(nested) = class_decl(
        ast::ClassModifier::Final,
        "Nested",
        vec![(false, "value", ty_named("T"))],
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        unreachable!("class builder returns a class")
    };
    let outer = class_source("Outer", vec!["T"], vec![nested_class(nested)]);
    let errors = lower_user(file(vec![Decl::Class(outer), fun("main", Vec::new())]))
        .expect_err("nested declaration must not inherit outer type parameters");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "static nested declaration cannot use outer type parameter `T`"
    }));
}

#[test]
fn private_nested_type_is_visible_only_in_its_lexical_owner_tree() {
    let mut secret = class_source("Secret", Vec::new(), Vec::new());
    secret.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let mut reader_method = method_full(
        false,
        false,
        "keep",
        vec![("value", ty_named("Secret"))],
        Some(ty_named("Secret")),
        FunctionBody::Expr(Box::new(var("value"))),
    );
    reader_method.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let reader = class_source(
        "Reader",
        Vec::new(),
        vec![ast::ClassMember::Function(reader_method)],
    );
    let vault = class_source(
        "Vault",
        Vec::new(),
        vec![nested_class(secret), nested_class(reader)],
    );
    let use_secret = fun_expr(
        "useSecret",
        Vec::new(),
        vec![("value", qualified_type(&["Vault", "Secret"]))],
        None,
        unit_lit(),
    );
    let errors = lower_user(file(vec![
        Decl::Class(vault),
        use_secret,
        fun("main", Vec::new()),
    ]))
    .expect_err("file peer must not see a private nested type");
    assert_eq!(errors.len(), 1);
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "type `Vault.Secret` is not accessible from this source location"
    }));
    assert!(
        !errors
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unknown type `Secret`") })
    );
}
