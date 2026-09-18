use scoop_identity::{ConeCoordinate, SignatureTypeKey};

use super::m23_ordinary_core_only::support::{parsed_ordinary, trusted_core};
use super::{
    Decl, assign_index, binary, bool_lit, call, extension_expr, file, fun, fun_expr, ident,
    int_lit, make_core_public, method_call, sp, stmt, subscript, this_expr, tuple_lit, ty_named,
    ty_tuple, unit_lit, val,
};
use crate::{OrdinarySources, lower_ordinary};

mod extensions;
pub(super) mod support;

use support::*;

#[test]
fn ordinary_dependency_calls_commit_one_reused_typed_hir_use() {
    let mut core = trusted_core();
    let provider = DependencyFunctionFixture::new(
        "callable-provider",
        "run",
        SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ),
    );
    let provider_foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 51);
    let aliases = empty_alias_expansions();
    let core_semantic_interface = empty_interface();
    let mut source = file(vec![fun(
        "consumer",
        vec![stmt(call("run", Vec::new())), stmt(call("run", Vec::new()))],
    )]);
    source
        .imports
        .push(exact_import(&["dependency", "api", "run"]));
    let ordinary = parsed_ordinary(source);
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ordinary.cone(),
        Some(scoop_hir::TrustedCoreImportedProviderInput::from_validated(
            certificate(&ConeCoordinate::reserved_core(), 41),
            &core.foundation,
            &core_semantic_interface,
            &aliases,
        )),
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            certificate(&provider.coordinate, 51),
            &provider_foundation,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input)
        .expect("a core-closed dependency function is executable in M23-5");

    assert_eq!(output.imported_dependencies().len(), 1);
    assert_eq!(output.concrete_dependency_witness_uses().len(), 1);
    let selected = output.imported_dependencies().callables().next().unwrap();
    let witness = &output.concrete_dependency_witness_uses()[0];
    assert_eq!(
        witness.target(),
        scoop_hir::ExternalHirTargetV1::Callable(selected.interface().declaration())
    );
    assert_eq!(
        witness.role(),
        scoop_hir::ExternalHirBindingWitnessRole::ConcreteSelectedUse
    );
    assert_eq!(
        witness.witness(),
        selected
            .binding()
            .sources()
            .next()
            .unwrap()
            .witness()
            .dependency()
    );
    assert_eq!(
        output.output().export.imported_dependency_callables.len(),
        1
    );
    assert_eq!(output.output().local.imported_dependency_callables.len(), 1);
    assert_eq!(
        scoop_hir::dump(&output.output().export)
            .matches("ImportedDependencyCall #0")
            .count(),
        2
    );
}

#[test]
fn direct_dependency_function_is_visible_in_the_split_current_package() {
    let mut core = trusted_core();
    let provider = DependencyFunctionFixture::new(
        "split-package-provider",
        "run",
        SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ),
    );
    let provider_foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 54);
    let aliases = empty_alias_expansions();
    let core_semantic_interface = empty_interface();
    let source = in_package(
        file(vec![fun("consumer", vec![stmt(call("run", Vec::new()))])]),
        &["dependency", "api"],
    );
    let ordinary = parsed_ordinary(source);
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ordinary.cone(),
        Some(scoop_hir::TrustedCoreImportedProviderInput::from_validated(
            certificate(&ConeCoordinate::reserved_core(), 41),
            &core.foundation,
            &core_semantic_interface,
            &aliases,
        )),
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            certificate(&provider.coordinate, 54),
            &provider_foundation,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input)
        .expect("a direct dependency contributes to the current split package");

    assert_eq!(output.imported_dependencies().callable_count(), 1);
    assert!(scoop_hir::dump(&output.output().export).contains("ImportedDependencyCall #0"));
}

#[test]
fn unsupported_dependency_candidate_falls_through_to_current_package() {
    let mut core = trusted_core();
    let provider = DependencyFunctionFixture::new(
        "layout-provider",
        "run",
        SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))),
    );
    let provider_foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 52);
    let aliases = empty_alias_expansions();
    let core_semantic_interface = empty_interface();
    let mut source = file(vec![
        fun("run", Vec::new()),
        fun("consumer", vec![stmt(call("run", Vec::new()))]),
    ]);
    source
        .imports
        .push(exact_import(&["dependency", "api", "run"]));
    let ordinary = parsed_ordinary(source);
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ordinary.cone(),
        Some(scoop_hir::TrustedCoreImportedProviderInput::from_validated(
            certificate(&ConeCoordinate::reserved_core(), 41),
            &core.foundation,
            &core_semantic_interface,
            &aliases,
        )),
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            certificate(&provider.coordinate, 52),
            &provider_foundation,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input)
        .expect("an unsupported exact candidate must not shadow a lower valid layer");

    assert!(output.imported_dependencies().is_empty());
    assert!(
        output
            .output()
            .export
            .imported_dependency_callables
            .is_empty()
    );
}

#[test]
fn unsupported_dependency_winner_reports_the_stable_layout_gate() {
    let mut core = trusted_core();
    let provider = DependencyFunctionFixture::new(
        "layout-only-provider",
        "raw",
        SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))),
    );
    let provider_foundation =
        core.import_dependency_foundation(&provider.coordinate, &provider.foundation, 53);
    let aliases = empty_alias_expansions();
    let core_semantic_interface = empty_interface();
    let mut source = file(vec![fun("consumer", vec![stmt(call("raw", Vec::new()))])]);
    source
        .imports
        .push(exact_import(&["dependency", "api", "raw"]));
    let ordinary = parsed_ordinary(source);
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ordinary.cone(),
        Some(scoop_hir::TrustedCoreImportedProviderInput::from_validated(
            certificate(&ConeCoordinate::reserved_core(), 41),
            &core.foundation,
            &core_semantic_interface,
            &aliases,
        )),
        vec![scoop_hir::DirectImportedProviderInput::from_validated(
            certificate(&provider.coordinate, 53),
            &provider_foundation,
            &provider.interface,
            &aliases,
        )],
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();

    let diagnostics = match lower_ordinary(scoop_identity::RequestedConeKind::Library, &input) {
        Ok(_) => panic!("a dependency pointer result requires the M23-6 ABI capability"),
        Err(diagnostics) => diagnostics,
    };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED")
    }));
}

fn in_package(mut source: scoop_ast::SourceFile, segments: &[&str]) -> scoop_ast::SourceFile {
    let (first, rest) = segments
        .split_first()
        .expect("test package paths are non-empty");
    source.package = scoop_ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: scoop_ast::QualifiedNameSyntax {
            first: ident(first),
            rest: rest
                .iter()
                .map(|segment| scoop_ast::QualifiedNameTailSyntax {
                    dot_span: sp(),
                    identifier: ident(segment),
                })
                .collect(),
            span: sp(),
        },
        span: sp(),
    };
    source
}
