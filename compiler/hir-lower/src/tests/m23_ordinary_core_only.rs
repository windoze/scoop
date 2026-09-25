use scoop_identity::{CallableTemplateOrigin, ConeIdentity, NominalDeclarationOwner};
use scoop_wire::WirePath;

use super::{call, file, fun, fun_expr, int_lit, sp, stmt, ty_named, var};
use crate::{CurrentConeSources, lower_current_cone};

mod aliases;
mod constants;
mod equality;
mod implicit_arrays;
mod intrinsic_calls;
mod member_calls;
pub(crate) mod support;

use support::{parsed_ordinary, trusted_core, trusted_core_with_answer};

fn public_type_alias(name: &str, target: scoop_ast::TypeRef) -> scoop_ast::Decl {
    scoop_ast::Decl::TypeAlias(scoop_ast::TypeAliasDecl {
        visibility: scoop_ast::VisibilitySyntax::Explicit {
            visibility: scoop_ast::DeclaredVisibility::Public,
            span: sp(),
        },
        name: super::ident(name),
        target,
        span: sp(),
    })
}

#[test]
fn ordinary_library_lowers_against_imported_core_without_core_sources() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(Vec::new()));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .expect("ordinary library lowering uses imported core authority");

    assert_eq!(output.output().export.source_files.len(), 1);
    assert_eq!(
        output.output().export.source_files[0].identity.cone(),
        ordinary.cone()
    );
    assert!(matches!(
        output.output().export.core_protocols,
        scoop_hir::CoreProtocols::Imported(_)
    ));
    assert!(matches!(
        output.output().local.core_protocols,
        scoop_hir::concrete::ConcreteCoreProtocols::Imported(_)
    ));
    assert!(output.output().local.materialization().roots().is_empty());
    assert!(output.imported_dependencies().is_empty());
    let foundation = scoop_hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
    assert_eq!(foundation.counts().external_source_types, 10);
    assert_eq!(foundation.counts().external_generic_types, 0);
}

#[test]
fn public_alias_retains_the_exact_imported_core_type_binding() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(vec![public_type_alias("Number", ty_named("Int"))]));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let int_binding = core.type_binding("Int");
    let scoop_hir::ImportedTarget::Type(int_declaration) = int_binding.target() else {
        panic!("the Int prelude binding targets a concrete nominal")
    };
    let int_declaration = int_declaration.persistent();
    let expected_binding = int_binding
        .sources()
        .next()
        .unwrap()
        .witness()
        .dependency()
        .route()
        .hops()[0]
        .binding();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .expect("a public alias may expose a trusted-core nominal");

    let [witness] = output.binding_witness_uses() else {
        panic!("the public alias must retain exactly one source-name witness")
    };
    assert_eq!(
        witness.target(),
        scoop_hir::ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(int_declaration))
    );
    assert_eq!(
        witness.role(),
        scoop_hir::ExternalHirBindingWitnessRole::AliasTarget
    );
    assert_eq!(
        witness.witness().route().immediate_provider(),
        ConeIdentity::CORE
    );
    assert_eq!(witness.witness().route().hops().len(), 1);
    assert_eq!(
        witness.witness().route().hops()[0].exporter(),
        ConeIdentity::CORE
    );
    assert_eq!(
        witness.witness().route().hops()[0].binding(),
        expected_binding
    );
}

#[test]
fn ordinary_executable_selects_current_main_under_imported_core_authority() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(vec![fun("main", Vec::new())]));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_current_cone(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("ordinary executable lowering uses imported core authority");

    assert!(matches!(
        output.output().output_kind(),
        scoop_hir::ConeOutputKind::Executable { .. }
    ));
    assert!(
        output
            .output()
            .export
            .source_files
            .iter()
            .all(|source| { source.identity.cone() != ConeIdentity::CORE })
    );
}

#[test]
fn ordinary_calls_select_one_strong_core_binding_and_reuse_its_typed_use() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![
            stmt(call("coreAnswer", Vec::new())),
            stmt(call("coreAnswer", Vec::new())),
        ],
    )]));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_current_cone(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("a param-free strong core callable is available to ordinary HIR");

    assert_eq!(output.imported_dependencies().callable_count(), 1);
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
fn imported_core_default_projects_after_the_input_world_is_dropped() {
    let mut declaration = fun_expr(
        "ordinaryDefault",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    let scoop_ast::Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.visibility = scoop_ast::VisibilitySyntax::Explicit {
        visibility: scoop_ast::DeclaredVisibility::Public,
        span: sp(),
    };
    function.params[0].syntax = scoop_ast::ParameterSyntax::Default {
        expression: call("coreAnswer", Vec::new()),
        equals_span: sp(),
    };
    let output = {
        let core = trusted_core_with_answer();
        let ordinary = parsed_ordinary(file(vec![declaration]));
        let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let world = core.world(ordinary.cone());
        let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();
        lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
            .expect("the ordinary default selects the strong core callable")
    };

    assert!(matches!(
        scoop_hir::CanonicalExportDefaultTemplatesV1::from_export_hir(
            output.output().export.module()
        ),
        Err(scoop_hir::DefaultTemplateProductionError::Template { source, .. })
            if matches!(
                source.as_ref(),
                scoop_hir::DefaultTemplateEnvelopeProjectionError::Body(
                    scoop_hir::DefaultBodyProjectionError::Entity(
                        scoop_hir::DefaultEntityProjectionError::ImportedDependencyUnavailable(_)
                    )
                )
            )
    ));

    let templates = scoop_hir::CanonicalExportDefaultTemplatesV1::from_dependency_hir(&output)
        .expect("the ordinary product owns the dependency selections needed by its defaults");
    let export = output.output().export.module();
    let function = export
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "ordinaryDefault").then_some(id))
        .unwrap();
    let scoop_hir::HirFunctionIdentity::Source(scoop_hir::HirSourceFunctionIdentity::Plain(
        identity,
    )) = &export.function_identities[function]
    else {
        panic!("the test declaration must have a plain source identity")
    };
    let owner = CallableTemplateOrigin::Function(identity.id());
    let template = templates
        .get(scoop_hir::ExportDefaultTemplateKeyV1::new(owner, 0))
        .unwrap();
    assert_eq!(template.references().callables().len(), 1);
    let callables = scoop_hir::CanonicalCallableInterfacesV1::from_export_hir(export).unwrap();
    template
        .validate_reference_closure_semantics(callables.get(owner).unwrap(), &WirePath::root())
        .expect("the imported callable reference must close exactly");
}

#[test]
fn ordinary_selected_core_call_lowers_to_one_branded_direct_mir_target() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![
            stmt(call("coreAnswer", Vec::new())),
            stmt(call("coreAnswer", Vec::new())),
        ],
    )]));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();
    let hir = lower_current_cone(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("ordinary HIR selects the trusted-core callable");
    let selected_dependencies = scoop_mir::SelectedExternalMirSet::try_from_callables(
        ordinary.cone(),
        hir.imported_dependencies()
            .callables()
            .map(|selected| {
                let declaration = selected.capability().declaration();
                let scoop_identity::DependencyCallableDeclarationId::Function(function) =
                    declaration
                else {
                    panic!("the fixture exports a source function")
                };
                scoop_mir::SelectedDependencyMirCallableV1::try_new(
                    ConeIdentity::CORE,
                    declaration,
                    scoop_identity::StrongCallableDefinitionOwner::Function(function),
                    selected.capability().signature().clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let mir = scoop_mir_lower::lower_current_cone(&hir, selected_dependencies)
        .expect("ordinary MIR retains the shared dependency selection");

    assert!(mir.selected_callables().initialization_cycle().is_none());
    assert_eq!(mir.module().meta.external_callables.len(), 1);
    let calls =
        mir.module()
            .functions
            .iter()
            .flat_map(|(_, function)| function.body.blocks.iter())
            .flat_map(|(_, block)| &block.statements)
            .filter_map(|statement| {
                let scoop_mir::StatementKind::Call(effect) = &statement.kind else {
                    return None;
                };
                let call = match effect {
                    scoop_mir::CallEffect::Unit(call)
                    | scoop_mir::CallEffect::Value { call, .. } => call,
                };
                matches!(call.target.callee, scoop_mir::Callee::External(_)).then_some(call)
            })
            .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|call| matches!(call.target.kind, scoop_mir::CallKind::Direct))
    );
}

#[test]
fn ordinary_core_call_uses_general_interface_without_legacy_strong_allowlist() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![stmt(call("coreAnswer", Vec::new()))],
    )]));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_current_cone(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("ordinary HIR selection uses the shared callable interface");
    assert_eq!(output.imported_dependencies().callable_count(), 1);
}

#[test]
fn ordinary_core_call_uses_shared_generic_argument_diagnostics() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![stmt(call("print", vec![int_lit(1)]))],
    )]));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let diagnostics =
        match lower_current_cone(scoop_identity::RequestedConeKind::Executable, &input) {
            Ok(_) => panic!("the shared call resolver requires explicit generic arguments"),
            Err(diagnostics) => diagnostics,
        };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("dependency function `print` expects 1 type argument(s), found 0")
    }));
}

#[test]
fn current_function_shadows_an_imported_core_prelude_callable() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![
        fun("coreAnswer", Vec::new()),
        fun("main", vec![stmt(call("coreAnswer", Vec::new()))]),
    ]));
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();

    let output = lower_current_cone(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("the current package layer wins before core prelude lookup");
    assert!(output.imported_dependencies().is_empty());
}
