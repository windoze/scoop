use super::*;
use crate::tests::{
    block, call, class_decl, core_file, file, fun, fun_expr, ident, int_lit, sp, ty_named,
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

fn lower(sources: &[(u32, ast::SourceFile)], locator: &str, request: u64) -> hir::Output {
    let core = core_file();
    let request = ast::Stage1RequestId::from_raw(request);
    let mut parsed = sources.iter().map(|(handle, source)| {
        ast::ParsedSource::new(
            ast::Stage1SourceHandle::new(request, *handle),
            source.clone(),
        )
    });
    let parsed = ast::AllParsedSources::try_new(
        request,
        ast::NonEmptyVec::new(
            parsed.next().expect("nonempty test request"),
            parsed.collect(),
        ),
    )
    .expect("unique test handles");
    let input = crate::Stage1CompilationInput::new(
        vec![crate::ProviderSource {
            source: &core,
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::Stage1SourceDetails {
            display_locator: locator,
            source_text: "",
        },
    );
    crate::lower_stage1_compilation_input(&input, crate::IntrinsicDeclarationPolicy::CoreOnly)
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
    let first = keys(&lower(&sources, "/old/duplicate.scoop", 1));
    assert_eq!(first.0.len(), 4);
    assert_eq!(first.1.len(), 4);
    assert_eq!(first.2.len(), 2);
    sources.reverse();
    assert_eq!(
        first,
        keys(&lower(&sources, "/different/duplicate.scoop", 99))
    );
}

#[test]
fn private_sources_with_identical_locators_keep_distinct_storage_and_roots() {
    let mut sources = vec![
        (17, declarations(&["same"], true)),
        (23, declarations(&["same"], true)),
        (91, file(vec![fun("main", Vec::new())])),
    ];
    let first = keys(&lower(&sources, "same.scoop", 1));
    assert_eq!(first.0.len(), 4);
    assert_eq!(first.2.len(), 2);
    sources.swap(0, 1);
    assert_eq!(first, keys(&lower(&sources, "other.scoop", 2)));
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
    let (_, units, roots) = keys(&lower(&sources, "same.scoop", 1));
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
    let (_, _, roots) = keys(&lower(&sources, "same.scoop", 1));
    assert_eq!(roots.len(), 2);
    assert!(roots[0].contains("f2:10"));
    assert!(roots[1].contains("f2:20"));
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
    let output = lower(&sources, "duplicate.scoop", 1);
    for name in ["clash", "secret"] {
        let stems = callable_stems(&output, name);
        assert_eq!(stems.len(), 2);
        assert_ne!(stems[0], stems[1]);
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
    let output = lower(&sources, "roles.scoop", 1);
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
    let output = lower(&sources, "accessors.scoop", 1);
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
