use super::*;
use crate::tests::m43_test_annotations::support::{marker_core, parsed_sources};

#[test]
fn a_selection_function_must_return_boolean_before_building_hir_control_flow() {
    let core = marker_core();
    let sources = parsed_sources(
        &[
            (
                "src/selection.scoop",
                "package scoop.test.execution\npublic fun isTestSelected(arguments: Array<String>, caseIndex: String, ignored: Boolean, caseName: String): Int = 0",
                SourceRole::Production,
            ),
            (
                "tests/check.scoop",
                "import scoop.test.Test\n@Test class Cases { @Test fun check() {} }",
                SourceRole::Test,
            ),
        ],
        false,
    );
    let world = core.world(sources.cone());
    let input = crate::CurrentConeSources::try_new(
        &sources,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let errors = crate::lower_current_tests(&input)
        .err()
        .expect("selection has the wrong result type");
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_eq!(
        errors[0].message,
        "test execution requires isTestSelected to return Boolean"
    );
    assert_eq!(
        errors[0].source.as_ref().unwrap().logical_path().as_str(),
        "tests/check.scoop"
    );
    assert_eq!(errors[0].span, Some(scoop_ast::Span::new(43, 48)));
}

#[test]
fn an_incompatible_execution_function_is_diagnosed_instead_of_skipping_the_case() {
    let core = marker_core();
    let sources = parsed_sources(
        &[
            (
                "src/execution/selection.scoop",
                crate::tests::m43_test_annotations::support::SELECTION_DECL,
                SourceRole::Production,
            ),
            (
                "src/execution/data.scoop",
                include_str!("../../../../../../sysroot/src/scoop.test/src/execution/data.scoop"),
                SourceRole::Production,
            ),
            (
                "src/execution/run.scoop",
                "package scoop.test.execution\npublic fun runReportedTestCase(): TestResult = TestResult.Passed",
                SourceRole::Production,
            ),
            (
                "tests/check.scoop",
                "import scoop.test.Test\n@Test class Cases { @Test fun check() {} }",
                SourceRole::Test,
            ),
        ],
        false,
    );
    let world = core.world(sources.cone());
    let input = crate::CurrentConeSources::try_new(
        &sources,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let errors = crate::lower_current_tests(&input)
        .err()
        .expect("an incompatible execution function must fail compilation");
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `runReportedTestCase` in implicit-import candidate layer:\n  - fun runReportedTestCase(): TestResult — expects 0 argument(s), but 3 were supplied"
    );
    assert_eq!(
        errors[0].source.as_ref().unwrap().logical_path().as_str(),
        "tests/check.scoop"
    );
    assert_eq!(errors[0].span, Some(scoop_ast::Span::new(43, 48)));
}
