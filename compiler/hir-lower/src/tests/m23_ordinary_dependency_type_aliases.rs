use scoop_identity::ConeCoordinate;

use super::m23_ordinary_core_only::support::{parsed_ordinary, trusted_core};
use super::m23_ordinary_dependencies::support::{
    alias_expansions, exact_import, project_dependency,
};
use super::{
    Decl, file, fun_expr, generic_struct_decl, ident, make_core_public, sp, struct_decl,
    ty_generic, ty_named, var,
};
use crate::{CurrentConeSources, lower_current_cone};

#[derive(Debug)]
struct AliasLoweringSummary {
    provider_alias: scoop_identity::PersistentTypeAliasId,
    facade_source: Option<scoop_hir::TypeAliasSourceTarget>,
    facade_target_name: Option<String>,
    selected_aliases: usize,
    alias_target_witnesses: Vec<scoop_hir::ExternalHirBindingWitnessUse>,
    concrete_witnesses: Vec<scoop_hir::ExternalHirBindingWitnessUse>,
}

fn public_type_alias(name: &str, target: scoop_ast::TypeRef) -> Decl {
    Decl::TypeAlias(scoop_ast::TypeAliasDecl {
        visibility: scoop_ast::VisibilitySyntax::Explicit {
            visibility: scoop_ast::DeclaredVisibility::Public,
            span: sp(),
        },
        name: ident(name),
        target,
        span: sp(),
    })
}

fn dependency_source(declarations: Vec<Decl>) -> scoop_ast::SourceFile {
    let mut source = file(declarations);
    make_core_public(&mut source);
    source.package = scoop_ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: qualified(&["dependency", "api"]),
        span: sp(),
    };
    source
}

fn consumer_source(import_name: &str, local_name: &str) -> scoop_ast::SourceFile {
    let mut source = file(vec![
        public_type_alias("Facade", ty_named(local_name)),
        fun_expr(
            "identity",
            Vec::new(),
            vec![("value", ty_named(local_name))],
            Some(ty_named(local_name)),
            var("value"),
        ),
    ]);
    make_core_public(&mut source);
    let mut import = exact_import(&["dependency", "api", import_name]);
    let scoop_ast::ImportSyntax::Exact { alias, .. } = &mut import else {
        unreachable!("the test helper creates an exact import")
    };
    *alias = Some(scoop_ast::ImportAliasSyntax {
        as_keyword_span: sp(),
        name: ident(local_name),
        span: sp(),
    });
    source.imports.push(import);
    source
}

fn lower_alias_fixture(
    provider_name: &str,
    provider_source: scoop_ast::SourceFile,
    consumer: scoop_ast::SourceFile,
) -> Result<AliasLoweringSummary, Vec<scoop_ast::Diagnostic>> {
    let mut core = trusted_core();
    let provider = ConeCoordinate::new("test", provider_name, "1.0.0").unwrap();
    let (foundation, interface) = project_dependency(&core, &provider, provider_source, &[]);
    let provider_alias = interface
        .type_aliases()
        .records()
        .first()
        .expect("the provider fixture exports a typealias")
        .alias();
    let provider_foundation = core.import_dependency_foundation(&provider, &foundation, 61);
    let provider_aliases = alias_expansions(interface.type_aliases());
    let ordinary = parsed_ordinary(consumer);
    let world = scoop_hir::ImportedSemanticWorld::from_dependencies(
        ordinary.cone(),
        vec![
            core.provider(),
            scoop_hir::ImportedProviderInput {
                foundation: &provider_foundation,
                interface: &interface,
                alias_expansions: &provider_aliases,
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();
    lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).map(|output| {
        let facade = output
            .output()
            .export
            .module()
            .type_aliases
            .iter()
            .find_map(|(_, alias)| (alias.name == "Facade").then_some(alias));
        AliasLoweringSummary {
            provider_alias,
            facade_source: facade.map(|alias| alias.source_target),
            facade_target_name: facade
                .map(|alias| scoop_hir::type_name(output.output().export.module(), alias.target)),
            selected_aliases: output.imported_dependencies().type_alias_count(),
            alias_target_witnesses: output.binding_witness_uses().to_vec(),
            concrete_witnesses: output.concrete_dependency_witness_uses().to_vec(),
        }
    })
}

#[test]
fn renamed_dependency_alias_is_transparent_and_retains_both_witness_roles() {
    let provider = dependency_source(vec![public_type_alias("Number", ty_named("Int"))]);
    let consumer = consumer_source("Number", "Renamed");

    let output = lower_alias_fixture("alias-provider", provider, consumer)
        .expect("a core-closed dependency alias must resolve transparently");

    assert_eq!(output.selected_aliases, 1);
    assert_eq!(output.facade_target_name.as_deref(), Some("Int"));
    assert_eq!(
        output.facade_source,
        Some(scoop_hir::TypeAliasSourceTarget::ImportedAlias(
            output.provider_alias
        ))
    );
    let [alias_witness] = output.alias_target_witnesses.as_slice() else {
        panic!("the public facade alias must retain one exact source-name route")
    };
    assert_eq!(
        alias_witness.target(),
        scoop_hir::ExternalHirTargetV1::TypeAlias(output.provider_alias)
    );
    assert_eq!(
        alias_witness.role(),
        scoop_hir::ExternalHirBindingWitnessRole::AliasTarget
    );
    let [concrete_witness] = output.concrete_witnesses.as_slice() else {
        panic!("the selected dependency alias must retain one concrete-use route")
    };
    assert_eq!(concrete_witness.target(), alias_witness.target());
    assert_eq!(
        concrete_witness.role(),
        scoop_hir::ExternalHirBindingWitnessRole::ConcreteSelectedUse
    );
    assert_eq!(concrete_witness.witness(), alias_witness.witness());
}

#[test]
fn dependency_alias_resolves_the_actual_foreign_struct() {
    let provider = dependency_source(vec![
        struct_decl("Payload", Vec::new()),
        public_type_alias("PayloadAlias", ty_named("Payload")),
    ]);
    let consumer = consumer_source("PayloadAlias", "ImportedPayload");

    let output = lower_alias_fixture("nominal-alias-provider", provider, consumer)
        .expect("a foreign struct alias resolves from its complete declaration");
    assert_eq!(output.facade_target_name.as_deref(), Some("Payload"));
    assert_eq!(output.selected_aliases, 1);
}

#[test]
fn dependency_alias_to_generic_application_reports_the_generic_gate() {
    let provider = dependency_source(vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        public_type_alias("IntBox", ty_generic("Box", vec![ty_named("Int")])),
    ]);
    let consumer = consumer_source("IntBox", "ImportedBox");

    let diagnostics = lower_alias_fixture("generic-alias-provider", provider, consumer)
        .expect_err("a foreign generic alias needs the M23-7 generic capability");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("SCOOP_HIR_CROSS_CONE_GENERIC_REQUIRED")
    }));
}

fn qualified(parts: &[&str]) -> scoop_ast::QualifiedNameSyntax {
    let (first, rest) = parts.split_first().expect("qualified names are nonempty");
    scoop_ast::QualifiedNameSyntax {
        first: ident(first),
        rest: rest
            .iter()
            .map(|part| scoop_ast::QualifiedNameTailSyntax {
                dot_span: sp(),
                identifier: ident(part),
            })
            .collect(),
        span: sp(),
    }
}
