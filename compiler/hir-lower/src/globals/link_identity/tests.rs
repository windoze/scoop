use super::*;
use crate::tests::{
    call, class_decl, core_file, file, fun, fun_expr, ident, int_lit, sp, ty_named,
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
