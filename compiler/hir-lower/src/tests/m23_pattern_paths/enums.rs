use super::*;

fn choice(name: &str) -> Decl {
    enum_decl(
        name,
        vec!["T"],
        vec![variant_positional("Item", vec![ty_named("T")])],
    )
}

fn qualified_type(path: &[&str]) -> TypeRef {
    TypeRef {
        kind: TypeRefKind::Qualified {
            path: path.iter().map(|part| ident(part)).collect(),
            arguments: vec![ty_named("Int")],
        },
        span: sp(),
    }
}

fn consume(path: &[&str], ty: TypeRef) -> Decl {
    fun_sig(
        "consume",
        Vec::new(),
        vec![("value", ty)],
        None,
        vec![when_stmt(
            var("value"),
            vec![arm(
                pat_pos(path, vec![pat_bind("payload")], None),
                None,
                Vec::new(),
            )],
            None,
        )],
    )
}

#[test]
fn current_exact_and_star_enum_prefixes_reuse_subject_arguments() {
    lower_sources(vec![file(vec![
        choice("Choice"),
        consume(
            &["Choice", "Item"],
            ty_generic("Choice", vec![ty_named("Int")]),
        ),
    ])])
    .expect("a current generic enum prefix borrows the subject's arguments");
    for import in [exact("api", "Choice", Some("Selected")), star("api")] {
        let name = if matches!(import, ast::ImportSyntax::Exact { .. }) {
            "Selected"
        } else {
            "Choice"
        };
        let api = package(file(vec![choice("Choice")]), "api");
        let mut user = file(vec![consume(
            &[name, "Item"],
            ty_generic(name, vec![ty_named("Int")]),
        )]);
        user.imports.push(import);
        lower_sources(vec![api, user]).expect("imports select the generic enum declaration");
    }
}

#[test]
fn package_and_nested_enum_prefixes_share_qualified_type_lookup() {
    let Decl::Enum(nested) = choice("Choice") else {
        panic!("enum fixture")
    };
    let Decl::Struct(mut outer) = struct_decl("Outer", Vec::new()) else {
        panic!("struct fixture")
    };
    outer.members.push(ast::StructMember::Nested(Box::new(
        ast::NestedNominalDecl::Enum(Box::new(nested)),
    )));
    let api = package(file(vec![Decl::Struct(outer)]), "api");
    let user = file(vec![consume(
        &["api", "Outer", "Choice", "Item"],
        qualified_type(&["api", "Outer", "Choice"]),
    )]);
    lower_sources(vec![api, user]).expect("package and nested paths keep the original enum");
}

#[test]
fn enum_alias_prefixes_require_the_full_application() {
    let api = package(
        file(vec![
            choice("Choice"),
            type_alias("IntChoice", ty_generic("Choice", vec![ty_named("Int")])),
        ]),
        "api",
    );
    let mut user = file(vec![consume(
        &["Fixed", "Item"],
        ty_generic("Choice", vec![ty_named("String")]),
    )]);
    user.imports.extend([
        exact("api", "IntChoice", Some("Fixed")),
        exact("api", "Choice", None),
    ]);
    let errors =
        lower_sources(vec![api, user]).expect_err("an alias cannot change the subject's arguments");
    assert!(
        errors.iter().any(|error| error.message
            == "pattern `Fixed.Item` does not match a subject of type Choice<String>"),
        "{errors:?}"
    );
}

#[test]
fn enum_subject_spelling_does_not_create_an_implicit_import() {
    let api = package(file(vec![choice("Choice")]), "api");
    let mut user = file(vec![consume(
        &["Choice", "Item"],
        ty_generic("Visible", vec![ty_named("Int")]),
    )]);
    user.imports.push(exact("api", "Choice", Some("Visible")));
    let errors = lower_sources(vec![api, user]).expect_err("a pattern prefix must be in scope");
    assert!(
        errors.iter().any(|error| error.message
            == "pattern `Choice.Item` does not match a subject of type Choice<Int>"),
        "{errors:?}"
    );
}

#[test]
fn enum_exact_import_blocks_a_current_package_fallback() {
    let wrong = package(file(vec![choice("Wrong")]), "wrong");
    let producer = package(file(vec![choice("Choice")]), "app");
    let mut user = package(
        file(vec![consume(
            &["Choice", "Item"],
            qualified_type(&["app", "Choice"]),
        )]),
        "app",
    );
    user.imports.push(exact("wrong", "Wrong", Some("Choice")));
    let errors = lower_sources(vec![wrong, producer, user])
        .expect_err("the selected exact declaration cannot retry another namespace");
    assert!(
        errors.iter().any(|error| error.message
            == "pattern `Choice.Item` does not match a subject of type Choice<Int>"),
        "{errors:?}"
    );
}

#[test]
fn enum_star_ambiguity_retains_both_declaration_origins() {
    let left = package(file(vec![choice("Choice")]), "left");
    let right = package(file(vec![choice("Choice")]), "right");
    let mut user = file(vec![consume(
        &["Choice", "Item"],
        qualified_type(&["left", "Choice"]),
    )]);
    user.imports.extend([star("left"), star("right")]);
    let errors = lower_sources(vec![left, right, user])
        .expect_err("different stars cannot select a prefix by subject shape");
    assert!(
        errors.iter().any(|error| error.message
            == "type `Choice` is ambiguous in the star import layer"
            && error.notes.len() == 2),
        "{errors:?}"
    );
}
