use super::*;
use crate::tests::m43_test_annotations::support::{marker_core, parsed_sources};

#[test]
fn an_incompatible_execution_function_is_diagnosed_instead_of_skipping_the_case() {
    let core = marker_core();
    let sources = parsed_sources(
        &[
            (
                "src/execution/data.scoop",
                include_str!("../../../../../../sysroot/lib/scoop.test/src/execution/data.scoop"),
                SourceRole::Production,
            ),
            (
                "src/execution/run.scoop",
                "package scoop.test.execution\npublic fun runTestCase(): TestResult = TestResult.Passed",
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
        "no applicable candidate for `runTestCase` in implicit-import candidate layer:\n  - fun runTestCase(): TestResult — expects 0 argument(s), but 2 were supplied"
    );
    assert_eq!(
        errors[0].source.as_ref().unwrap().logical_path().as_str(),
        "tests/check.scoop"
    );
    assert_eq!(errors[0].span, Some(scoop_ast::Span::new(43, 48)));
}
