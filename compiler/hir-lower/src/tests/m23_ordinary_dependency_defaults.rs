use scoop_identity::ConeCoordinate;

use super::m23_ordinary_core_only::support::{TrustedCoreFixture, parsed_ordinary, trusted_core};
use super::m23_ordinary_dependencies::support::{
    empty_alias_expansions, exact_import, project_dependency,
};
use super::{
    call, file, fun, fun_expr, int_lit, make_core_public, scoop_extern_fun, sp, stmt, ty_named, var,
};
use crate::{CurrentConeSources, lower_current_cone};

#[test]
fn dependency_default_calls_public_provider_helper_with_split_origins() {
    let mut core = trusted_core();
    let provider = ConeCoordinate::new("test", "default-provider", "1.0.0").unwrap();
    let (foundation, interface) = dependency_with_callable_default(&core, &provider);
    let provider_foundation = core.import_dependency_foundation(&provider, &foundation, 54);
    let aliases = empty_alias_expansions();

    let mut consumer = file(vec![fun(
        "consumer",
        vec![stmt(call("withDefault", Vec::new()))],
    )]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "withDefault"]));
    let ordinary = parsed_ordinary(consumer);
    let world = scoop_hir::ImportedSemanticWorld::from_dependencies(
        ordinary.cone(),
        vec![
            core.provider(),
            scoop_hir::ImportedProviderInput {
                foundation: &provider_foundation,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .expect("the provider-bound default helper must materialize in the consumer");

    assert_eq!(output.imported_dependencies().len(), 2);
    assert_eq!(
        output.output().export.imported_dependency_callables.len(),
        2
    );
    let helper = imported_helper_expression(output.output().export.module());
    let origin = helper.origin.concrete();
    let source_files = &output.output().export.source_files;
    assert_eq!(
        source_files[usize::try_from(origin.definition.file).unwrap()]
            .identity
            .cone(),
        provider.identity().unwrap()
    );
    assert_eq!(
        source_files[usize::try_from(origin.evaluation.file).unwrap()]
            .identity
            .cone(),
        ordinary.cone()
    );
    let constant = imported_default_constant_expression(output.output().export.module());
    let origin = constant.origin.concrete();
    assert_eq!(
        source_files[usize::try_from(origin.definition.file).unwrap()]
            .identity
            .cone(),
        provider.identity().unwrap()
    );
    assert_eq!(
        source_files[usize::try_from(origin.evaluation.file).unwrap()]
            .identity
            .cone(),
        ordinary.cone()
    );
}

#[test]
fn rejected_dependency_default_falls_through_without_committing_provider_state() {
    let mut core = trusted_core();
    let provider = ConeCoordinate::new("test", "rejected-default-provider", "1.0.0").unwrap();
    let mut with_default = fun_expr(
        "withDefault",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    let scoop_ast::Decl::Function(function) = &mut with_default else {
        unreachable!("fun_expr always builds a function declaration")
    };
    function.params[0].syntax = scoop_ast::ParameterSyntax::Default {
        expression: call("externalValue", Vec::new()),
        equals_span: sp(),
    };
    let mut provider_source = file(vec![
        scoop_extern_fun(
            "externalValue",
            "test_external_value",
            Vec::new(),
            Some(ty_named("Int")),
        ),
        with_default,
    ]);
    make_core_public(&mut provider_source);
    provider_source.package = scoop_ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: qualified(&["dependency", "api"]),
        span: sp(),
    };
    let (foundation, interface) = project_dependency(&core, &provider, provider_source, &["Int"]);
    let provider_foundation = core.import_dependency_foundation(&provider, &foundation, 55);
    let aliases = empty_alias_expansions();

    let mut consumer = file(vec![
        fun("withDefault", Vec::new()),
        fun("consumer", vec![stmt(call("withDefault", Vec::new()))]),
    ]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "withDefault"]));
    let ordinary = parsed_ordinary(consumer);
    let world = scoop_hir::ImportedSemanticWorld::from_dependencies(
        ordinary.cone(),
        vec![
            core.provider(),
            scoop_hir::ImportedProviderInput {
                foundation: &provider_foundation,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .expect("a rejected dependency default must not shadow the current-package callable");

    assert!(output.imported_dependencies().is_empty());
    assert!(
        output
            .output()
            .export
            .imported_dependency_callables
            .is_empty()
    );
    assert!(
        output
            .output()
            .export
            .source_files
            .iter()
            .all(|source| source.identity.cone() != provider.identity().unwrap())
    );

    let mut unsupported = file(vec![fun(
        "consumer",
        vec![stmt(call("withDefault", Vec::new()))],
    )]);
    unsupported
        .imports
        .push(exact_import(&["dependency", "api", "withDefault"]));
    let unsupported = parsed_ordinary(unsupported);
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&unsupported, core_inputs, &world).unwrap();
    let diagnostics = match lower_current_cone(scoop_identity::RequestedConeKind::Library, &input) {
        Ok(_) => panic!("a native dependency in the only default candidate must be rejected"),
        Err(diagnostics) => diagnostics,
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("SCOOP_HIR_CROSS_CONE_NATIVE_REQUIRED")
    }));
}

fn dependency_with_callable_default(
    core: &TrustedCoreFixture,
    coordinate: &ConeCoordinate,
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    let mut with_default = fun_expr(
        "withDefault",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    let scoop_ast::Decl::Function(function) = &mut with_default else {
        unreachable!("fun_expr always builds a function declaration")
    };
    function.params[0].syntax = scoop_ast::ParameterSyntax::Default {
        expression: call("defaultValue", vec![var("DEFAULT_VALUE")]),
        equals_span: sp(),
    };
    let mut source = file(vec![
        const_property("DEFAULT_VALUE", int_lit(7)),
        fun_expr(
            "defaultValue",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        with_default,
    ]);
    make_core_public(&mut source);
    source.package = scoop_ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: qualified(&["dependency", "api"]),
        span: sp(),
    };
    project_dependency(core, coordinate, source, &["Int"])
}

fn imported_helper_expression(module: &scoop_hir::Module) -> &scoop_hir::Expr {
    let consumer = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "consumer").then_some(function))
        .expect("the consumer function must be present");
    let scoop_hir::FunctionKind::User(body) = &consumer.kind else {
        panic!("the consumer function must have a Scoop body")
    };
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            scoop_hir::StatementKind::ValDecl { init, .. }
                if matches!(
                    &init.kind,
                    scoop_hir::ExprKind::ImportedDependencyCall { args, .. } if args.len() == 1
                ) =>
            {
                Some(init)
            }
            _ => None,
        })
        .expect("the dependency default helper call must be materialized once")
}

fn imported_default_constant_expression(module: &scoop_hir::Module) -> &scoop_hir::Expr {
    let consumer = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "consumer").then_some(function))
        .expect("the consumer function must be present");
    let scoop_hir::FunctionKind::User(body) = &consumer.kind else {
        panic!("the consumer function must have a Scoop body")
    };
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            scoop_hir::StatementKind::ValDecl { init, .. }
                if matches!(
                    init.kind,
                    scoop_hir::ExprKind::IntegerLiteral(scoop_hir::HirIntegerConstant::Signed32(7))
                ) =>
            {
                Some(init)
            }
            _ => None,
        })
        .expect("the dependency default's const temporary must be materialized")
}

fn const_property(name: &str, expression: scoop_ast::Expr) -> scoop_ast::Decl {
    scoop_ast::Decl::Global(scoop_ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: scoop_ast::VisibilitySyntax::Omitted,
        modifier: scoop_ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: super::ident(name),
        ty: ty_named("Int"),
        body: scoop_ast::PropertyBodySyntax::Const(Box::new(expression)),
        span: sp(),
    })
}

fn qualified(parts: &[&str]) -> scoop_ast::QualifiedNameSyntax {
    let (first, rest) = parts.split_first().expect("qualified names are nonempty");
    scoop_ast::QualifiedNameSyntax {
        first: super::ident(first),
        rest: rest
            .iter()
            .map(|part| scoop_ast::QualifiedNameTailSyntax {
                dot_span: sp(),
                identifier: super::ident(part),
            })
            .collect(),
        span: sp(),
    }
}
