use scoop_identity::{CallableTemplateOrigin, ConeIdentity};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::{call, file, fun, fun_expr, int_lit, sp, stmt, ty_named, var};
use crate::{OrdinaryCoreOnlySources, lower_ordinary_core_only};

mod support;

use support::{parsed_ordinary, trusted_core, trusted_core_with_answer};

#[test]
fn ordinary_library_lowers_against_imported_core_without_core_sources() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(Vec::new()));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Library, &input)
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
    assert!(matches!(
        output.output().local.materialization(),
        scoop_hir::LocalConcreteMaterializationContract::Ordinary
    ));
    assert_eq!(output.imported_core().callable_count(), 0);
    assert_eq!(output.imported_core().type_count(), 0);
    assert_eq!(output.imported_core().value_count(), 0);
    assert!(output.imported_dependencies().is_empty());
    let foundation = scoop_hir::CanonicalHirFoundation::from_ordinary_output(&output).unwrap();
    assert_eq!(foundation.counts().core_external_source_types, 10);
    assert_eq!(foundation.counts().core_external_generic_types, 0);
}

#[test]
fn ordinary_executable_selects_current_main_under_imported_core_authority() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(vec![fun("main", Vec::new())]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input)
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
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &core.strong_callables)
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("a param-free strong core callable is available to ordinary HIR");

    assert_eq!(output.imported_core().callable_count(), 1);
    assert!(output.imported_dependencies().is_empty());
    assert_eq!(output.output().export.imported_core_callables.len(), 1);
    assert_eq!(output.output().local.imported_core_callables.len(), 1);
    assert_eq!(
        scoop_hir::dump(&output.output().export)
            .matches("ImportedCoreCall #0")
            .count(),
        2
    );
}

#[test]
fn imported_core_default_requires_the_selected_set_to_project() {
    let core = trusted_core_with_answer();
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
    let ordinary = parsed_ordinary(file(vec![declaration]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &core.strong_callables)
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();
    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Library, &input)
        .expect("the ordinary default selects the strong core callable");

    assert!(matches!(
        scoop_hir::CanonicalExportDefaultTemplatesV1::from_export_hir(
            output.output().export.module()
        ),
        Err(scoop_hir::DefaultTemplateProductionError::Template { source, .. })
            if matches!(
                source.as_ref(),
                scoop_hir::DefaultTemplateEnvelopeProjectionError::Body(
                    scoop_hir::DefaultBodyProjectionError::Entity(
                        scoop_hir::DefaultEntityProjectionError::ImportedCoreUnavailable(_)
                    )
                )
            )
    ));

    let templates = scoop_hir::CanonicalExportDefaultTemplatesV1::from_ordinary_hir(&output)
        .expect("the ordinary product supplies the exact imported-core selection sidecar");
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
        .validate_reference_closure_semantics(
            callables.get(owner).unwrap(),
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
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
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &core.strong_callables)
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();
    let hir = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("ordinary HIR selects the trusted-core callable");
    let selected_mir = core.project_selected_callables_to_mir(hir.imported_core());

    let mir = scoop_mir_lower::lower_ordinary(&hir, selected_mir)
        .expect("ordinary MIR retains the exact trusted-core selection");

    assert_eq!(mir.imported_core().len(), 1);
    assert_eq!(mir.module().meta.imported_core_callables.len(), 1);
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
                matches!(call.target.callee, scoop_mir::Callee::CoreExternal(_)).then_some(call)
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
fn ordinary_call_rejects_a_core_candidate_without_strong_implementation() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![stmt(call("coreAnswer", Vec::new()))],
    )]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let diagnostics =
        match lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input) {
            Ok(_) => {
                panic!("a raw HIR candidate cannot stand in for a strong implementation proof")
            }
            Err(diagnostics) => diagnostics,
        };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("SCOOPC_CAPABILITY_CORE_IMPLEMENTATION_UNAVAILABLE")
    }));
}

#[test]
fn ordinary_call_rejects_a_generic_core_candidate_with_the_stable_capability_code() {
    let core = trusted_core();
    let ordinary = parsed_ordinary(file(vec![fun(
        "main",
        vec![stmt(call("print", vec![int_lit(1)]))],
    )]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let diagnostics =
        match lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input) {
            Ok(_) => panic!("generic core prelude candidates are unavailable in M23-3"),
            Err(diagnostics) => diagnostics,
        };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("SCOOPC_CAPABILITY_CORE_GENERIC_UNAVAILABLE")
    }));
}

#[test]
fn current_function_shadows_an_imported_core_prelude_callable() {
    let core = trusted_core_with_answer();
    let ordinary = parsed_ordinary(file(vec![
        fun("coreAnswer", Vec::new()),
        fun("main", vec![stmt(call("coreAnswer", Vec::new()))]),
    ]));
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &core.strong_callables)
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();

    let output = lower_ordinary_core_only(scoop_identity::RequestedConeKind::Executable, &input)
        .expect("the current package layer wins before core prelude lookup");

    assert_eq!(output.imported_core().callable_count(), 0);
    assert!(output.output().export.imported_core_callables.is_empty());
}
