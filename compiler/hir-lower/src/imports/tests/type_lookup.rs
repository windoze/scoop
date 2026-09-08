use super::*;
use crate::tests::{fun, fun_expr, fun_sig, generic_struct_decl, struct_decl, ty_named, var};

fn tagged(name: &str, marker: &str) -> ast::Decl {
    struct_decl(name, vec![(marker, ty_named("Unit"))])
}

fn consumer(name: &str) -> ast::SourceFile {
    file(vec![
        fun_sig(
            "consume",
            Vec::new(),
            vec![("value", ty_named(name))],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ])
}

fn selected_type(output: &hir::Output) -> hir::TypeId {
    output
        .export
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "consume").then(|| function.params[0].ty))
        .expect("consumer is present")
}

fn marker(output: &hir::Output) -> &str {
    let hir::Type::Struct(application) = output.export.types[selected_type(output)] else {
        panic!("selected type is a structure")
    };
    let template = output.export.struct_applications[application].template;
    let hir::StructRepresentation::Declared(fields) =
        &output.export.structs[template].representation
    else {
        panic!("selected type is an ordinary structure")
    };
    &fields[0].name
}

fn alias(name: &str, target: ast::TypeRef) -> ast::Decl {
    ast::Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        target,
        span: sp(),
    })
}

fn qualified(segments: &[&str]) -> ast::TypeRef {
    ast::TypeRef {
        kind: ast::TypeRefKind::Qualified {
            path: segments.iter().map(|segment| ident(segment)).collect(),
            arguments: Vec::new(),
        },
        span: sp(),
    }
}

#[test]
fn type_lookup_uses_exact_then_package_then_star_and_deduplicates_origins() {
    let exact_source = package(file(vec![tagged("FromExact", "exact")]), &["exactlib"]);
    let star_source = package(file(vec![tagged("Chosen", "star")]), &["starlib"]);
    for (include_exact, include_current, expected) in [
        (true, true, "exact"),
        (false, true, "current"),
        (false, false, "star"),
    ] {
        let mut user = consumer("Chosen");
        if include_current {
            user.declarations.push(tagged("Chosen", "current"));
        }
        user.imports = vec![star(&["starlib"], false), star(&["starlib"], false)];
        if include_exact {
            user.imports.extend([
                exact(&["exactlib", "FromExact"], Some("Chosen"), false),
                exact(&["exactlib", "FromExact"], Some("Chosen"), false),
            ]);
        }
        let output = lower_sources(vec![
            exact_source.clone(),
            star_source.clone(),
            user.clone(),
        ])
        .expect("the highest visible type layer wins");
        assert_eq!(marker(&output), expected);
        user.imports.reverse();
        let permuted = lower_sources(vec![user, star_source.clone(), exact_source.clone()])
            .expect("source/import order does not select a different type");
        assert_eq!(marker(&permuted), expected);
    }
}

#[test]
fn imported_primitive_spelling_can_shadow_core_without_affecting_core_bodies() {
    let library = package(file(vec![tagged("Int", "customInt")]), &["api"]);
    let mut user = consumer("Int");
    user.imports.push(star(&["api"], false));
    let custom = lower_sources(vec![library.clone(), user])
        .expect("star type is above core prelude even for primitive spelling");
    assert_eq!(marker(&custom), "customInt");
    let core = lower_sources(vec![library, consumer("Int")])
        .expect("without import the core prelude remains available");
    assert_eq!(
        core.export.types[selected_type(&core)],
        hir::Type::Integer(hir::IntegerKind::SIGNED_32)
    );
}

#[test]
fn core_cannot_access_current_unit_types_through_qualified_package_paths() {
    let mut public_type = struct_decl("PublicType", Vec::new());
    let ast::Decl::Struct(declaration) = &mut public_type else {
        unreachable!()
    };
    declaration.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    };
    let library = package(file(vec![public_type]), &["api"]);
    let mut core = crate::tests::core_file();
    core.declarations.push(fun_sig(
        "coreCannotSeeUserType",
        Vec::new(),
        vec![("value", qualified(&["api", "PublicType"]))],
        None,
        Vec::new(),
    ));
    let errors = lower_sources_with_core(vec![library, file(vec![fun("main", Vec::new())])], core)
        .expect_err("core qualification must not expose the current unit namespace");
    assert!(
        errors.iter().any(|error| {
            error.file == 0 && error.message == "package `api` has no accessible type `PublicType`"
        }),
        "{errors:#?}"
    );
}

#[test]
fn same_layer_type_ambiguity_lists_every_origin_at_the_use_site() {
    let left = package(file(vec![tagged("Chosen", "left")]), &["left"]);
    let right = package(file(vec![tagged("Chosen", "right")]), &["right"]);
    for exact_layer in [false, true] {
        let mut user = consumer("Chosen");
        let ast::Decl::Function(consume) = &mut user.declarations[0] else {
            unreachable!()
        };
        consume.params[0].ty = ast::TypeRef {
            kind: ast::TypeRefKind::Named(ast::Ident {
                text: "Chosen".to_string(),
                span: ast::Span::new(111, 117),
            }),
            span: ast::Span::new(111, 117),
        };
        user.imports = if exact_layer {
            vec![
                exact(&["left", "Chosen"], None, false),
                exact(&["right", "Chosen"], None, false),
            ]
        } else {
            vec![star(&["left"], false), star(&["right"], false)]
        };
        let errors = lower_sources(vec![left.clone(), right.clone(), user.clone()])
            .expect_err("different typed origins in one layer are ambiguous");
        let error = errors
            .iter()
            .find(|error| error.message.contains("type `Chosen` is ambiguous"))
            .expect("ambiguity diagnostic");
        assert_eq!(error.span, Some(ast::Span::new(111, 117)));
        assert_eq!(error.notes.len(), 2);
        assert_eq!(
            error.notes.iter().map(|note| note.file).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(error.message.contains(if exact_layer {
            "exact import"
        } else {
            "star import"
        }));
        user.imports.reverse();
        let errors = lower_sources(vec![right.clone(), user, left.clone()])
            .expect_err("permutation remains ambiguous");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("type `Chosen` is ambiguous")
                    && error.notes.len() == 2)
        );
    }
}

#[test]
fn aliases_with_the_same_expansion_remain_distinct_lookup_origins() {
    let library = package(
        file(vec![
            tagged("Value", "value"),
            alias("First", ty_named("Value")),
            alias("Second", ty_named("Value")),
        ]),
        &["api"],
    );
    let mut user = consumer("Chosen");
    user.imports = vec![
        exact(&["api", "First"], Some("Chosen"), false),
        exact(&["api", "Second"], Some("Chosen"), false),
    ];
    let errors = lower_sources(vec![library, user])
        .expect_err("alias identity is not its expanded type identity");
    assert!(errors.iter().any(
        |error| error.message.contains("type `Chosen` is ambiguous") && error.notes.len() == 2
    ));
}

#[test]
fn private_type_in_current_package_does_not_hide_visible_star_type() {
    let mut private = tagged("Chosen", "private");
    let ast::Decl::Struct(declaration) = &mut private else {
        unreachable!()
    };
    declaration.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let private_source = file(vec![private]);
    let public_source = package(file(vec![tagged("Chosen", "visible")]), &["api"]);
    let mut user = consumer("Chosen");
    user.imports.push(star(&["api"], false));
    let output = lower_sources(vec![private_source, public_source, user])
        .expect("inaccessible candidates do not stop layer lookup");
    assert_eq!(marker(&output), "visible");
}

#[test]
fn imported_aliases_resolve_signatures_alias_rhs_and_normalized_duplicates() {
    let library = package(file(vec![tagged("Value", "value")]), &["api"]);
    let mut first = file(vec![
        alias("Local", ty_named("Imported")),
        fun_expr(
            "identity",
            Vec::new(),
            vec![("value", ty_named("Local"))],
            Some(ty_named("Imported")),
            var("value"),
        ),
        fun("main", Vec::new()),
    ]);
    first
        .imports
        .push(exact(&["api", "Value"], Some("Imported"), false));
    lower_sources(vec![library.clone(), first.clone()])
        .expect("alias RHS and parameter/result types use the frozen file scope");
    let mut second = file(vec![fun_expr(
        "identity",
        Vec::new(),
        vec![("value", ty_named("Value"))],
        Some(ty_named("Value")),
        var("value"),
    )]);
    second.imports.push(exact(&["api", "Value"], None, false));
    let errors = lower_sources(vec![second, library, first])
        .expect_err("duplicate signatures compare expanded types rather than import spelling");
    assert!(
        errors.iter().any(|error| error.message
            == "function `identity` is already declared with the same signature")
    );
    assert!(
        !errors
            .iter()
            .any(|error| error.message.contains("unknown type"))
    );
}

#[test]
fn imported_alias_cycle_is_diagnosed_across_file_scopes() {
    let mut left = package(file(vec![alias("Left", ty_named("R"))]), &["left"]);
    left.imports
        .push(exact(&["right", "Right"], Some("R"), false));
    let mut right = package(file(vec![alias("Right", ty_named("L"))]), &["right"]);
    right
        .imports
        .push(exact(&["left", "Left"], Some("L"), false));
    let errors = lower_sources(vec![left, right, file(vec![fun("main", Vec::new())])])
        .expect_err("import aliases do not bypass the typed alias cycle detector");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("typealias cycle:"))
    );
    assert!(
        !errors
            .iter()
            .any(|error| error.message.contains("unknown type"))
    );
}

#[test]
fn qualified_alias_preserves_its_exact_generic_application() {
    let library = package(
        file(vec![
            generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
            alias(
                "IntBox",
                ast::TypeRef {
                    kind: ast::TypeRefKind::Generic(ident("Box"), vec![ty_named("Int")]),
                    span: sp(),
                },
            ),
        ]),
        &["api"],
    );
    let user = file(vec![
        fun_sig(
            "consume",
            Vec::new(),
            vec![("value", qualified(&["api", "IntBox"]))],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]);
    let output = lower_sources(vec![library, user])
        .expect("qualified alias is already a concrete type application");
    let hir::Type::Struct(application) = output.export.types[selected_type(&output)] else {
        panic!("IntBox is a struct application")
    };
    assert_eq!(
        output.export.types[output.export.struct_applications[application].arguments[0]],
        hir::Type::Integer(hir::IntegerKind::SIGNED_32)
    );
}

#[test]
fn imported_type_qualifier_walks_nested_owners_without_package_fallback() {
    let ast::Decl::Struct(nested) = tagged("Nested", "nested") else {
        unreachable!()
    };
    let ast::Decl::Struct(mut host) = struct_decl("Host", Vec::new()) else {
        unreachable!()
    };
    host.members.push(ast::StructMember::Nested(Box::new(
        ast::NestedNominalDecl::Struct(Box::new(nested)),
    )));
    let library = package(file(vec![ast::Decl::Struct(host)]), &["api"]);
    let mut user = file(vec![
        fun_sig(
            "consume",
            Vec::new(),
            vec![("value", qualified(&["Renamed", "Nested"]))],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]);
    user.imports
        .push(exact(&["api", "Host"], Some("Renamed"), false));
    let output = lower_sources(vec![library.clone(), user.clone()])
        .expect("the imported type alias is a typed qualifier");
    assert_eq!(marker(&output), "nested");
    let longer_package = package(file(Vec::new()), &["Renamed", "Nested"]);
    let errors = lower_sources(vec![library, user, longer_package])
        .expect_err("the longest package prefix cannot fall back to the imported owner");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "`Renamed.Nested` names a package, not a type")
    );
}

#[test]
fn imports_resolve_nominal_base_bounds_and_extension_receiver_signatures() {
    use crate::tests::{class_decl, interface_decl};
    let library = package(
        file(vec![
            interface_decl("Contract", Vec::new()),
            class_decl(
                ast::ClassModifier::Open,
                "Base",
                Vec::new(),
                None,
                Vec::new(),
                Vec::new(),
            ),
            struct_decl("Value", Vec::new()),
        ]),
        &["api"],
    );
    let ast::Decl::Function(mut bounded) = fun_expr(
        "bounded",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    ) else {
        unreachable!()
    };
    bounded.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_named("ImportedContract")));
    let mut extension = method("extension");
    extension.receiver_ty = Some(ty_named("ImportedValue"));
    let mut user = file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Derived",
            Vec::new(),
            Some(("ImportedBase", Vec::new())),
            Vec::new(),
            Vec::new(),
        ),
        ast::Decl::Function(bounded),
        ast::Decl::Function(extension),
        fun("main", Vec::new()),
    ]);
    user.imports = vec![
        exact(&["api", "Contract"], Some("ImportedContract"), false),
        exact(&["api", "Base"], Some("ImportedBase"), false),
        exact(&["api", "Value"], Some("ImportedValue"), false),
    ];
    lower_sources(vec![user, library])
        .expect("all declaration signature type positions use the frozen scope");
}

#[test]
fn a_selected_type_with_wrong_arity_does_not_fall_back_to_a_lower_layer() {
    let library = package(file(vec![struct_decl("Plain", Vec::new())]), &["api"]);
    let mut user = file(vec![
        generic_struct_decl("Box", vec!["T"], Vec::new()),
        fun_sig(
            "consume",
            Vec::new(),
            vec![(
                "value",
                ast::TypeRef {
                    kind: ast::TypeRefKind::Generic(ident("Box"), vec![ty_named("Int")]),
                    span: sp(),
                },
            )],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]);
    user.imports
        .push(exact(&["api", "Plain"], Some("Box"), false));
    let errors = lower_sources(vec![library, user])
        .expect_err("type lookup is not callable applicability fallback");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "struct `Box` is not generic")
    );
}
