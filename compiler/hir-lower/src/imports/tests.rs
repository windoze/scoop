use super::*;
mod dependency;
mod dependency_fixture;
mod materialization;
mod pipeline;
mod reexports;
mod resolution;
mod type_lookup;
mod value_lookup;
use crate::tests::{
    call, core_source_identity, enum_decl, field, file, fun, ident, identified_test_sources, sp,
    stmt, str_lit, test_source_identity, ty_named, val, var, variant_positional, variant_unit,
};
use crate::{IntrinsicDeclarationPolicy, Lowerer, SourceKind, SourceProvider};

fn path(segments: &[&str]) -> ast::QualifiedNameSyntax {
    let mut offset = 20;
    let mut identifier = |text: &str| {
        let start = offset;
        offset += u32::try_from(text.len()).expect("test identifier length") + 1;
        ast::Ident {
            text: text.to_string(),
            span: ast::Span::new(start, offset - 1),
        }
    };
    let first = identifier(segments[0]);
    let rest = segments[1..]
        .iter()
        .map(|name| {
            let identifier = identifier(name);
            ast::QualifiedNameTailSyntax {
                dot_span: ast::Span::new(identifier.span.start - 1, identifier.span.start),
                identifier,
            }
        })
        .collect();
    ast::QualifiedNameSyntax {
        first,
        rest,
        span: ast::Span::new(20, offset - 1),
    }
}

fn exact(segments: &[&str], alias: Option<&str>, public: bool) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: exposure(public),
        selector: path(segments),
        alias: alias.map(|name| ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(name),
            span: sp(),
        }),
        import_keyword_span: ast::Span::new(7, 13),
        span: ast::Span::new(0, 90),
    }
}

fn exposure(public: bool) -> ast::ImportExposureSyntax {
    if public {
        ast::ImportExposureSyntax::PublicReexport {
            public_keyword_span: ast::Span::new(0, 6),
        }
    } else {
        ast::ImportExposureSyntax::Local
    }
}

fn star(segments: &[&str], public: bool) -> ast::ImportSyntax {
    let namespace = path(segments);
    let terminal_dot_span = ast::Span::new(namespace.span.end, namespace.span.end + 1);
    let star_span = ast::Span::new(terminal_dot_span.end, terminal_dot_span.end + 1);
    ast::ImportSyntax::Star {
        exposure: exposure(public),
        namespace,
        import_keyword_span: sp(),
        terminal_dot_span,
        star_span,
        span: ast::Span::new(0, 90),
    }
}

fn selector_span(import: &ast::ImportSyntax) -> ast::Span {
    match import {
        ast::ImportSyntax::Exact { selector, .. } => selector.span,
        ast::ImportSyntax::Star {
            namespace,
            star_span,
            ..
        } => ast::Span::new(namespace.span.start, star_span.end),
    }
}

fn package(mut source: ast::SourceFile, segments: &[&str]) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: path(segments),
        span: sp(),
    };
    source
}

fn lowerer() -> (Lowerer, PackageId, PackageId) {
    let files = [
        file(Vec::new()),
        package(file(Vec::new()), &["api"]),
        package(file(Vec::new()), &["api", "child"]),
        package(file(Vec::new()), &["api", "empty"]),
    ];
    let mut lowerer = Lowerer::new().with_intrinsic_sources(
        [
            "src/00-root.scoop",
            "src/01-api.scoop",
            "src/02-api-child.scoop",
            "src/03-api-empty.scoop",
        ]
        .into_iter()
        .map(|path| SourceProvider {
            provider: hir::IntrinsicProviderId::from_raw(1),
            kind: SourceKind::CurrentUnit,
            identity: test_source_identity(path),
            name: "same.scoop".to_string(),
            source: String::new(),
        })
        .collect(),
        IntrinsicDeclarationPolicy::CoreOnly,
    );
    let cone = lowerer.current_cone();
    lowerer
        .top_level_namespaces
        .initialize_sources([cone; 4], cone, &files);
    let (api, _) = lowerer
        .top_level_namespaces
        .longest_package_prefix(&[ident("api")]);
    let (child, _) = lowerer
        .top_level_namespaces
        .longest_package_prefix(&[ident("api"), ident("child")]);
    (lowerer, api, child)
}

fn function(
    surface: &mut CurrentUnitImports,
    lowerer: &Lowerer,
    namespace: ResolvedNamespace,
    name: &str,
    file: usize,
    index: u32,
    private: bool,
) -> CurrentUnitBindingId {
    function_with_visibility(
        surface,
        lowerer,
        namespace,
        name,
        file,
        index,
        if private {
            hir::DeclaredVisibility::Private
        } else {
            hir::DeclaredVisibility::Internal
        },
    )
}

fn function_with_visibility(
    surface: &mut CurrentUnitImports,
    lowerer: &Lowerer,
    namespace: ResolvedNamespace,
    name: &str,
    file: usize,
    index: u32,
    visibility: hir::DeclaredVisibility,
) -> CurrentUnitBindingId {
    let source = lowerer.visibility_file(file);
    let domain = lowerer.top_level_domain(visibility, file);
    surface.insert(
        namespace,
        CurrentUnitBinding {
            target: CurrentUnitTarget::Function(hir::FunctionId::from_raw(index.into())),
            source,
            file,
            span: ast::Span::new(index * 10, index * 10 + 3),
            access: hir::EffectiveLookupDomain(domain),
            name: name.to_string(),
        },
    )
}

fn class(
    surface: &mut CurrentUnitImports,
    lowerer: &Lowerer,
    namespace: ResolvedNamespace,
    name: &str,
    file: usize,
    index: u32,
) -> CurrentUnitBindingId {
    surface.insert(
        namespace,
        CurrentUnitBinding {
            target: CurrentUnitTarget::Class(hir::ClassId::from_raw(index.into())),
            source: lowerer.visibility_file(file),
            file,
            span: ast::Span::new(index * 10, index * 10 + 3),
            access: hir::EffectiveLookupDomain(
                lowerer.top_level_domain(hir::DeclaredVisibility::Internal, file),
            ),
            name: name.to_string(),
        },
    )
}

fn lower_sources(sources: Vec<ast::SourceFile>) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower_sources_with_core(sources, crate::tests::core_file())
}

fn lower_sources_with_core(
    sources: Vec<ast::SourceFile>,
    core: ast::SourceFile,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let parsed = identified_test_sources(sources);
    let input = crate::DefinedTestSources::try_new(
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
            display_locator: "same.scoop",
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");
    crate::lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        IntrinsicDeclarationPolicy::CoreOnly,
    )
}

fn constant(name: &str) -> ast::PropertyDecl {
    ast::PropertyDecl {
        context_parameters: Vec::new(),
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: crate::tests::ty_named("Int"),
        body: ast::PropertyBodySyntax::Const(Box::new(crate::tests::int_lit(7))),
        span: sp(),
    }
}

fn method(name: &str) -> ast::FunctionDecl {
    let ast::Decl::Function(function) = crate::tests::fun(name, Vec::new()) else {
        unreachable!()
    };
    function
}

fn declared_function(name: &str, visibility: ast::DeclaredVisibility) -> ast::Decl {
    let mut declaration = crate::tests::fun(name, Vec::new());
    let ast::Decl::Function(function) = &mut declaration else {
        unreachable!("the function builder returns a function")
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility,
        span: sp(),
    };
    declaration
}

fn with_import(import: ast::ImportSyntax) -> ast::SourceFile {
    let mut source = file(Vec::new());
    source.imports.push(import);
    source
}
