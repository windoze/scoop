use super::*;
use crate::tests::{
    block, call, class_decl, core_file, core_source_identity, file, fun, fun_expr, ident, int_lit,
    sp, test_source_identity, ty_named,
};
use scoop_ast as ast;

fn package(mut source: ast::SourceFile, names: &[&str]) -> ast::SourceFile {
    if let Some((first, rest)) = names.split_first() {
        source.package = ast::PackageSyntax::QualifiedPackage {
            package_keyword_span: sp(),
            path: ast::QualifiedNameSyntax {
                first: ident(first),
                rest: rest
                    .iter()
                    .map(|name| ast::QualifiedNameTailSyntax {
                        dot_span: sp(),
                        identifier: ident(name),
                    })
                    .collect(),
                span: sp(),
            },
            span: sp(),
        };
    }
    source
}

fn visibility(private: bool) -> ast::VisibilitySyntax {
    if private {
        ast::VisibilitySyntax::Explicit {
            visibility: ast::DeclaredVisibility::Private,
            span: sp(),
        }
    } else {
        ast::VisibilitySyntax::Omitted
    }
}

fn object(name: &str, private: bool) -> ast::ObjectDecl {
    ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: visibility(private),
        name: ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    }
}

fn class(name: &str, private: bool) -> ast::Decl {
    let ast::Decl::Class(mut declaration) = class_decl(
        ast::ClassModifier::Final,
        name,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        unreachable!("the class builder returns a class declaration")
    };
    declaration.visibility = visibility(private);
    ast::Decl::Class(declaration)
}

fn property(name: &str, private: bool, runtime: bool) -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: visibility(private),
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(if runtime {
                call("make", Vec::new())
            } else {
                int_lit(1)
            }),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

fn declarations(names: &[&str], private: bool) -> ast::SourceFile {
    let mut make = fun_expr(
        "make",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        int_lit(1),
    );
    let ast::Decl::Function(function) = &mut make else {
        unreachable!()
    };
    function.visibility = visibility(private);
    package(
        file(vec![
            property("value", private, true),
            property("image", private, false),
            make,
            ast::Decl::Object(object("Tools", private)),
        ]),
        names,
    )
}

fn callable(name: &str, private: bool, receiver: Option<&str>) -> ast::Decl {
    let mut declaration = fun(name, Vec::new());
    let ast::Decl::Function(function) = &mut declaration else {
        unreachable!("the function test builder constructs a function")
    };
    function.visibility = visibility(private);
    function.receiver_ty = receiver.map(ty_named);
    declaration
}

fn computed_property(name: &str, receiver: Option<&str>) -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: receiver.map(ty_named),
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
            setter: Some(ast::SetterDecl {
                annotations: Vec::new(),
                visibility: ast::SetterVisibilitySyntax::Inherited,
                parameter: ast::SetterParameterSyntax::Default { span: sp() },
                body: ast::AccessorBodySyntax::Block(block(Vec::new())),
                span: sp(),
            }),
        }),
        span: sp(),
    })
}

fn callable_stems(output: &hir::Output, name: &str) -> Vec<String> {
    let mut export = output
        .export
        .functions
        .iter()
        .filter(|(_, function)| function.name == name)
        .map(|(_, function)| function.link_stem.as_str().to_string())
        .collect::<Vec<_>>();
    let mut local = output
        .local
        .functions
        .iter()
        .filter(|(_, function)| function.name == name)
        .map(|(_, function)| function.link_stem.as_str().to_string())
        .collect::<Vec<_>>();
    export.sort();
    local.sort();
    assert_eq!(export, local, "concretization preserves callable stems");
    export
}

fn class_stems(output: &hir::Output, name: &str) -> Vec<String> {
    let mut export = output
        .export
        .classes
        .iter()
        .filter(|(_, declaration)| declaration.name == name)
        .map(|(_, declaration)| declaration.link_stem.as_str().to_string())
        .collect::<Vec<_>>();
    let mut local = output
        .local
        .classes
        .iter()
        .filter(|(_, definition)| definition.name == name)
        .map(|(_, definition)| definition.link_stem.as_str().to_string())
        .collect::<Vec<_>>();
    export.sort();
    local.sort();
    assert_eq!(export, local, "concretization preserves nominal stems");
    export
}

fn concrete_class_identities(
    output: &hir::Output,
    name: &str,
) -> Vec<scoop_identity::PersistentTypeId> {
    let mut identities = output
        .export
        .classes
        .iter()
        .filter(|(_, declaration)| declaration.name == name)
        .map(|(id, _)| {
            output.export.nominal_identities[id]
                .concrete_type_id()
                .expect("a non-generic test class has a concrete persistent identity")
        })
        .collect::<Vec<_>>();
    identities.sort();
    identities
}

fn source_identity(source_key: u32) -> scoop_identity::SourceIdentity {
    let path = match source_key {
        1 => "src/001-one.scoop",
        2 => "src/002-two.scoop",
        3 => "src/003-three.scoop",
        10 => "src/010-ten.scoop",
        17 => "src/017-seventeen.scoop",
        20 => "src/020-twenty.scoop",
        23 => "src/023-twenty-three.scoop",
        30 => "src/030-thirty.scoop",
        91 => "src/091-ninety-one.scoop",
        _ => panic!("test source key {source_key} needs an explicit logical path"),
    };
    test_source_identity(path)
}

fn lower(sources: &[(u32, ast::SourceFile)], locator: &str) -> hir::Output {
    let core = core_file();
    let mut parsed = sources.iter().map(|(source_key, source)| {
        ast::IdentifiedParsedSource::new(source_identity(*source_key), source.clone())
    });
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        parsed.next().expect("nonempty test source set"),
        parsed.collect(),
    ))
    .expect("unique explicit test source identities");
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
            display_locator: locator,
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");
    crate::lower_legacy_combined_sources(&input, crate::IntrinsicDeclarationPolicy::CoreOnly)
        .expect("test current unit lowers")
}

fn keys(output: &hir::Output) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut globals: Vec<_> = output
        .export
        .globals
        .iter()
        .map(|(_, global)| global.name.clone())
        .collect();
    let mut units: Vec<_> = output
        .export
        .initialization_units
        .iter()
        .map(|(_, unit)| unit.stable_key.clone())
        .collect();
    let mut roots: Vec<_> = output
        .export
        .singleton_published_roots
        .iter()
        .map(|(_, root)| root.link_name.clone())
        .collect();
    globals.sort();
    units.sort();
    roots.sort();
    for collection in [&globals, &units, &roots] {
        assert!(
            collection.windows(2).all(|pair| pair[0] != pair[1]),
            "local identities are unique: {collection:?}"
        );
    }
    let mut concrete_globals: Vec<_> = output
        .local
        .globals
        .iter()
        .map(|(_, global)| global.name.clone())
        .collect();
    let mut concrete_units: Vec<_> = output
        .local
        .initialization_units
        .iter()
        .map(|(_, unit)| unit.stable_key.clone())
        .collect();
    let mut concrete_roots: Vec<_> = output
        .local
        .singleton_published_roots
        .iter()
        .map(|(_, root)| root.link_name.clone())
        .collect();
    concrete_globals.sort();
    concrete_units.sort();
    concrete_roots.sort();
    assert_eq!(
        (&globals, &units, &roots),
        (&concrete_globals, &concrete_units, &concrete_roots)
    );
    (globals, units, roots)
}

#[test]
fn length_framing_preserves_delimiters_and_unicode() {
    let mut key = String::new();
    field(&mut key, 'p', "a:b");
    field(&mut key, 'n', "中文");
    assert_eq!(key, "p3:a:bn6:中文");
}

#[test]
fn package_globals_and_singletons_are_distinct_and_locator_independent() {
    let mut sources = vec![
        (17, declarations(&["a"], false)),
        (23, declarations(&["b"], false)),
        (91, file(vec![fun("main", Vec::new())])),
    ];
    let first = keys(&lower(&sources, "/old/duplicate.scoop"));
    assert_eq!(first.0.len(), 4);
    assert_eq!(first.1.len(), 4);
    assert_eq!(first.2.len(), 2);
    sources.reverse();
    assert_eq!(first, keys(&lower(&sources, "/different/duplicate.scoop")));
}

#[test]
fn private_sources_with_identical_locators_keep_distinct_storage_and_roots() {
    let mut sources = vec![
        (17, declarations(&["same"], true)),
        (23, declarations(&["same"], true)),
        (91, file(vec![fun("main", Vec::new())])),
    ];
    let first = keys(&lower(&sources, "same.scoop"));
    assert_eq!(first.0.len(), 4);
    assert_eq!(first.2.len(), 2);
    sources.swap(0, 1);
    assert_eq!(first, keys(&lower(&sources, "other.scoop")));
}

#[test]
fn package_edges_and_nominal_edges_cannot_share_a_singleton_key() {
    let ast::Decl::Class(mut host) = class_decl(
        ast::ClassModifier::Final,
        "b",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        unreachable!()
    };
    host.members.push(ast::ClassMember::Nested(Box::new(
        ast::NestedNominalDecl::Object(Box::new(object("Tools", false))),
    )));
    let sources = vec![
        (
            1,
            package(
                file(vec![ast::Decl::Object(object("Tools", false))]),
                &["a", "b"],
            ),
        ),
        (2, package(file(vec![ast::Decl::Class(host)]), &["a"])),
        (3, file(vec![fun("main", Vec::new())])),
    ];
    let (_, units, roots) = keys(&lower(&sources, "same.scoop"));
    assert_eq!(roots.len(), 2);
    assert_eq!(units.len(), 2);
    assert!(roots.iter().any(|key| key.contains("p8:s1:as1:b")));
    assert!(roots.iter().any(|key| key.contains("p4:s1:aC4:n1:b")));
}

#[test]
fn nested_singletons_in_private_hosts_inherit_the_host_source_key() {
    let ast::Decl::Class(mut host) = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        unreachable!()
    };
    host.visibility = visibility(true);
    host.members.push(ast::ClassMember::Nested(Box::new(
        ast::NestedNominalDecl::Object(Box::new(object("Tools", false))),
    )));
    let source = file(vec![ast::Decl::Class(host)]);
    let sources = vec![
        (10, source.clone()),
        (20, source),
        (30, file(vec![fun("main", Vec::new())])),
    ];
    let (_, _, roots) = keys(&lower(&sources, "same.scoop"));
    assert_eq!(roots.len(), 2);
    assert!(roots[0].contains("f17:src/010-ten.scoop"));
    assert!(roots[1].contains("f20:src/020-twenty.scoop"));
}

#[test]
fn callable_stems_separate_packages_and_file_private_sources() {
    let sources = vec![
        (
            1,
            package(file(vec![callable("clash", false, None)]), &["a"]),
        ),
        (
            2,
            package(file(vec![callable("clash", false, None)]), &["b"]),
        ),
        (
            17,
            package(file(vec![callable("secret", true, None)]), &["same"]),
        ),
        (
            23,
            package(file(vec![callable("secret", true, None)]), &["same"]),
        ),
        (91, file(vec![fun("main", Vec::new())])),
    ];
    let output = lower(&sources, "duplicate.scoop");
    for name in ["clash", "secret"] {
        let stems = callable_stems(&output, name);
        assert_eq!(stems.len(), 2);
        assert_ne!(stems[0], stems[1]);
    }
}

#[test]
fn nominal_stems_separate_packages_and_file_private_sources() {
    let sources = vec![
        (1, package(file(vec![class("Same", false)]), &["a"])),
        (2, package(file(vec![class("Same", false)]), &["b"])),
        (17, package(file(vec![class("Secret", true)]), &["same"])),
        (23, package(file(vec![class("Secret", true)]), &["same"])),
        (91, file(vec![fun("main", Vec::new())])),
    ];
    let output = lower(&sources, "duplicate.scoop");
    for name in ["Same", "Secret"] {
        let stems = class_stems(&output, name);
        assert_eq!(stems.len(), 2);
        assert_ne!(stems[0], stems[1]);
        let identities = concrete_class_identities(&output, name);
        assert_eq!(identities.len(), 2);
        assert_ne!(identities[0], identities[1]);
    }
}

#[test]
fn persistent_nominal_identities_ignore_source_order_and_display_locator() {
    let mut sources = vec![
        (1, package(file(vec![class("Same", false)]), &["a"])),
        (2, package(file(vec![class("Same", false)]), &["b"])),
        (3, file(vec![fun("main", Vec::new())])),
    ];
    let first = concrete_class_identities(&lower(&sources, "/old/tree/input.scoop"), "Same");
    sources.reverse();
    let second = concrete_class_identities(&lower(&sources, "/new/tree/renamed.scoop"), "Same");
    assert_eq!(first, second);
}

#[test]
fn object_and_backing_class_have_distinct_final_nominal_stems() {
    let sources = vec![(
        1,
        file(vec![
            ast::Decl::Object(object("Registry", false)),
            fun("main", Vec::new()),
        ]),
    )];
    let output = lower(&sources, "objects.scoop");
    let (object_id, object) = output
        .export
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Registry")
        .expect("Registry object");
    let backing = &output.export.classes[object.backing_class];
    assert_ne!(object.link_stem, backing.link_stem);
    let object_identity = &output.export.nominal_identities[object_id];
    let backing_identity = &output.export.nominal_identities[object.backing_class];
    assert!(object_identity.source().is_some());
    assert!(backing_identity.source().is_none());
    assert_ne!(
        object_identity.concrete_type_id(),
        backing_identity.concrete_type_id()
    );

    let (_, concrete_object) = output
        .local
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Registry")
        .expect("concrete Registry object");
    assert_eq!(concrete_object.link_stem, object.link_stem);
    assert_eq!(
        output.local.classes[concrete_object.backing_class].link_stem,
        backing.link_stem
    );
}

#[test]
fn nested_nominal_stems_inherit_a_private_owners_source_identity() {
    let host = || {
        let ast::Decl::Class(mut host) = class("Host", true) else {
            unreachable!("the class helper returns a class")
        };
        let ast::Decl::Class(inner) = class("Inner", false) else {
            unreachable!("the class helper returns a class")
        };
        host.members.push(ast::ClassMember::Nested(Box::new(
            ast::NestedNominalDecl::Class(Box::new(inner)),
        )));
        ast::Decl::Class(host)
    };
    let sources = vec![
        (10, package(file(vec![host()]), &["same"])),
        (20, package(file(vec![host()]), &["same"])),
        (30, file(vec![fun("main", Vec::new())])),
    ];
    let output = lower(&sources, "same.scoop");
    let nested = output
        .export
        .classes
        .iter()
        .filter(|(_, declaration)| declaration.name == "Inner")
        .collect::<Vec<_>>();
    assert_eq!(nested.len(), 2);
    assert_ne!(nested[0].1.link_stem, nested[1].1.link_stem);
    assert_ne!(
        output.export.nominal_identities[nested[0].0].concrete_type_id(),
        output.export.nominal_identities[nested[1].0].concrete_type_id()
    );
    for (source_id, declaration) in nested {
        let source_identity = &output.export.nominal_identities[source_id];
        let concrete = output
            .local
            .classes
            .iter()
            .find(|(_, definition)| &definition.origin == source_identity)
            .expect("each nested declaration is concretized")
            .1;
        assert_eq!(concrete.link_stem, declaration.link_stem);
    }
}

#[test]
fn callable_roles_separate_ordinary_extensions_and_receiver_types() {
    let sources = vec![(
        1,
        file(vec![
            callable("mix", false, None),
            callable("mix", false, Some("Int")),
            callable("mix", false, Some("String")),
            fun("main", Vec::new()),
        ]),
    )];
    let output = lower(&sources, "roles.scoop");
    let stems = callable_stems(&output, "mix");
    assert_eq!(stems.len(), 3);
    let ordinary = stems
        .iter()
        .find(|stem| stem.contains("q8:ordinary"))
        .expect("ordinary callable stem");
    let extensions = stems
        .iter()
        .filter(|stem| stem.contains("q9:extension"))
        .collect::<Vec<_>>();
    assert_eq!(extensions.len(), 2);
    assert_eq!(extensions[0], extensions[1]);
    assert_ne!(ordinary.as_str(), extensions[0].as_str());
}

#[test]
fn accessor_stems_encode_getter_setter_and_extension_roles() {
    let sources = vec![(
        1,
        file(vec![
            computed_property("value", None),
            computed_property("value", Some("Int")),
            fun("main", Vec::new()),
        ]),
    )];
    let output = lower(&sources, "accessors.scoop");
    let getters = callable_stems(&output, "$get$value");
    let setters = callable_stems(&output, "$set$value");
    assert_eq!(getters.len(), 2);
    assert_eq!(setters.len(), 2);
    assert!(getters.windows(2).all(|pair| pair[0] != pair[1]));
    assert!(setters.windows(2).all(|pair| pair[0] != pair[1]));
    assert!(
        getters.iter().all(|stem| stem.contains("r6:getter")),
        "{getters:?}"
    );
    assert!(
        setters.iter().all(|stem| stem.contains("r6:setter")),
        "{setters:?}"
    );
    assert!(
        getters
            .iter()
            .chain(&setters)
            .any(|stem| stem.contains("q9:extension"))
    );
}
