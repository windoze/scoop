use scoop_hir as hir;
use scoop_identity::{ExecutableEntryDeclaration, GeneratedCallableKey, SourceRole};

use super::support::lower_tests;

mod contracts;
mod imported;

#[test]
fn generated_test_entry_calls_checked_lifecycle_with_its_own_identity() {
    let output = lower_tests(&[(
        "tests/cases.scoop",
        r#"
        package example
        import scoop.test.*
        fun main(): Int = 77
        @Test class Cases private constructor(val value: Int = 4) {
            @BeforeTest private fun prepare() {}
            @Test private fun check() {}
            @AfterTest private fun cleanup() {}
        }
    "#,
        SourceRole::Test,
    )])
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let module = &output.output().export;
    let hir::ConeOutputKind::Executable { local_entry } = output.output().output_kind() else {
        panic!("test lowering produces an executable")
    };
    let function = local_entry.local_function().function();
    assert_ne!(module.functions[function].name, "main");
    assert_eq!(module.functions[function].params.len(), 1);
    let hir::HirFunctionIdentity::TestEntry(record) = &module.function_identities[function] else {
        panic!("test entry has a generated identity")
    };
    assert_eq!(
        record.key(),
        &GeneratedCallableKey::TestEntry { cone: module.cone }
    );
    assert_eq!(
        local_entry.declaration(),
        ExecutableEntryDeclaration::Generated(record.id())
    );
    assert_eq!(module.test_containers.len(), 1);
    assert_eq!(module.lambdas.len(), 4);
    assert!(
        module
            .functions
            .iter()
            .any(|(_, function)| function.name == "main")
    );
    hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
}

#[test]
fn generated_callbacks_keep_each_private_container_in_its_source_file() {
    let text = "import scoop.test.Test\n@Test private class Cases(val seed: Int = 2) { @Test private fun check() {} }";
    let output = lower_tests(&[
        ("tests/a.scoop", text, SourceRole::Test),
        ("tests/z.scoop", text, SourceRole::Test),
    ])
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let module = &output.output().export;
    assert_eq!(module.test_containers.len(), 2);
    assert_eq!(module.lambdas.len(), 4);
    hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
}

#[test]
fn ignored_tests_have_callbacks_guarded_by_runtime_selection() {
    let output = lower_tests(&[(
        "tests/cases.scoop",
        r#"
        import scoop.test.*
        @Test class Empty {}
        @Test class Ignored {
            @Ignore("later") @Test fun check() {}
        }
    "#,
        SourceRole::Test,
    )])
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
    assert_eq!(output.output().export.test_containers.len(), 2);
    let module = &output.output().export;
    assert_eq!(module.lambdas.len(), 2);
    let entry = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "$test_entry")
        .unwrap()
        .1;
    let hir::FunctionKind::User(body) = &entry.kind else {
        panic!("generated entry has a body")
    };
    assert_eq!(
        body.statements
            .iter()
            .filter(|statement| matches!(statement.kind, hir::StatementKind::If { .. }))
            .count(),
        1
    );
}

#[test]
fn an_empty_generated_entry_survives_the_mir_entry_boundary() {
    use crate::tests::m23_ordinary_core_only::support::{parsed_ordinary_text, trusted_core};

    let core = trusted_core();
    let sources = parsed_ordinary_text("fun main(): Int = 77");
    let world = core.world(sources.cone());
    let input = crate::CurrentConeSources::try_new(
        &sources,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let output = crate::lower_current_tests(&input).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let local = &output.output().local;
    let hir::LocalConeOutputKind::Executable { local_entry } = local.output_kind() else {
        panic!("test output has an executable entry")
    };
    let entry = &local.functions[local_entry.local_function().function()];
    let hir::CoreProtocolCallableDefinitionV1::Function(arguments) =
        core.interface.program_arguments().definition()
    else {
        panic!("program arguments use the core source function")
    };
    let callable = scoop_mir::SelectedDependencyMirCallableV1::try_new(
        core.foundation.origin(),
        scoop_identity::DependencyCallableDeclarationId::Function(arguments),
        scoop_identity::StrongCallableDefinitionOwner::Function(arguments),
        scoop_identity::ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            None,
            Vec::new(),
            local.exact_type_identities[entry.params[0].ty].id(),
        ),
    )
    .unwrap();
    let dependencies =
        scoop_mir::SelectedExternalMirSet::try_from_callables(local.cone, vec![callable]).unwrap();
    let mir = scoop_mir_lower::lower_current_cone(&output, dependencies, Default::default(), None)
        .unwrap();
    assert_eq!(mir.module().cone, output.output().local.module().cone);
}
