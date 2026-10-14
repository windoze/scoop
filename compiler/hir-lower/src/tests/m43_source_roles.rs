use crate::{CurrentConeSources, lower_current_cone};
use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{RequestedConeKind, SourceRole};

use super::m23_ordinary_core_only::support::trusted_core;

fn lower(production: &str, tests: &str) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    lower_sources(&[
        ("cases/check.scoop", tests, SourceRole::Test),
        ("src/api.scoop", production, SourceRole::Production),
    ])
}

fn lower_sources(
    inputs: &[(&str, &str, SourceRole)],
) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    let core = trusted_core();
    let identities = inputs
        .iter()
        .map(|(path, _, _)| super::test_source_identity(path))
        .collect::<Vec<_>>();
    let mut parser_inputs = inputs
        .iter()
        .zip(&identities)
        .map(|((path, text, role), identity)| {
            scoop_parser::CurrentConeSourceInput::new(identity, text, path.as_ref())
                .with_role(*role)
        });
    let first = parser_inputs.next().unwrap();
    let sources =
        scoop_parser::parse_current_cone(ast::NonEmptyVec::new(first, parser_inputs.collect()))
            .unwrap();
    let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(sources.cone());
    let input = CurrentConeSources::try_new(&sources, protocols, &world).unwrap();
    lower_current_cone(RequestedConeKind::Library, &input)
}

#[test]
fn test_overloads_do_not_change_production_calls_or_default_arguments() {
    lower(
        "fun choose(value: Any): Int = 7\n\
         fun production(): Int = choose(1)\n\
         fun defaulted(value: Int = choose(1)): Int = value\n\
         fun <T> generic(value: T): Int = choose(1)",
        "fun choose(value: Int): String = \"test\"\n\
         fun selected(): String = choose(1)\n\
         fun checkDefault(): Int = defaulted()\n\
         fun checkGeneric(): Int = generic(1)",
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
}

#[test]
fn test_extensions_do_not_change_production_receiver_calls() {
    lower(
        "fun Any.choose(): Int = 7\nfun production(): Int = 1.choose()",
        "fun Int.choose(): String = \"test\"\nfun selected(): String = 1.choose()",
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
}

#[test]
fn production_lookup_cannot_use_test_functions_types_values_or_qualified_types() {
    for (production, tests, name) in [
        (
            "const val production: Int = hidden",
            "const val hidden: Int = 7",
            "hidden",
        ),
        (
            "fun production(): Hidden = 1",
            "typealias Hidden = Int",
            "Hidden",
        ),
        (
            "@Hidden fun production() {}",
            "annotation class Hidden",
            "Hidden",
        ),
        (
            "fun production(): Int = hidden()",
            "fun hidden(): Int = 7",
            "hidden",
        ),
        (
            "fun production(): Hidden = Hidden()",
            "class Hidden",
            "Hidden",
        ),
        (
            "val production: Int = hidden",
            "val hidden: Int = 7",
            "hidden",
        ),
        (
            "fun production(): only.tests.Hidden = only.tests.Hidden()",
            "package only.tests\nclass Hidden",
            "only",
        ),
        (
            "fun <T> production(value: T): Int = hidden()",
            "fun hidden(): Int = 7",
            "hidden",
        ),
        (
            "fun production(value: Int = hidden()): Int = value",
            "fun hidden(): Int = 7",
            "hidden",
        ),
    ] {
        let errors = match lower(production, tests) {
            Ok(_) => panic!("production source resolved test declaration {name}"),
            Err(errors) => errors,
        };
        assert!(
            errors.iter().any(|error| error
                .source
                .as_deref()
                .is_some_and(|source| source.logical_path().as_str() == "src/api.scoop")
                && error.message.contains(name)
                && error.span.is_some()),
            "{errors:#?}"
        );
    }
}

#[test]
fn imports_filter_test_declarations_before_ambiguity_and_overload_selection() {
    lower(
        "package api\nfun choose(value: Any): Int = 7\nfun production(): Int = choose(1)",
        "package api\nfun choose(value: Int): String = \"test\"\nfun selected(): String = choose(1)",
    ).unwrap_or_else(|errors| panic!("{errors:#?}"));
    for import in ["import helpers.hidden", "import helpers.*"] {
        let errors = match lower(
            &format!("{import}\nfun production(): Int = hidden()"),
            "package helpers\nfun hidden(): Int = 7",
        ) {
            Ok(_) => panic!("production import resolved a test-only package"),
            Err(errors) => errors,
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("import target is not available")),
            "{errors:#?}"
        );
    }
}

#[test]
fn exact_alias_and_star_imports_keep_production_overloads_only() {
    for (import, name) in [
        ("import api.choose", "choose"),
        ("import api.choose as selected", "selected"),
        ("import api.*", "choose"),
    ] {
        let caller = format!("{import}\nfun production(): Int = {name}(1)");
        lower_sources(&[
            ("cases/check.scoop", "package api\nfun choose(value: Int): String = \"test\"\nfun check(): String = choose(1)", SourceRole::Test),
            ("src/api.scoop", "package api\nfun choose(value: Any): Int = 7", SourceRole::Production),
            ("src/caller.scoop", &caller, SourceRole::Production),
        ]).unwrap_or_else(|errors| panic!("{errors:#?}"));
    }
}

#[test]
fn test_only_package_does_not_shadow_a_production_object_qualifier() {
    lower(
        "object Tools { val answer: Int = 42 }\nfun production(): Int = Tools.answer",
        "package Tools\nclass Noise",
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
}

#[test]
fn test_sources_use_production_internal_and_test_helpers_with_ordinary_privacy() {
    lower(
        "package api\ninternal class Value {}\ninternal fun answer(): Int = 42",
        "package cases\nimport api.Value\nimport api.answer\n\
         private fun helper(): Int = answer()\nfun check(): Int = helper()\n\
         fun value(): Value = Value()",
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
    for (production, tests) in [
        (
            "private fun hidden(): Int = 1",
            "fun check(): Int = hidden()",
        ),
        (
            "class Value { private fun hidden(): Int = 1 }",
            "fun check(): Int = Value().hidden()",
        ),
    ] {
        let errors = match lower(production, tests) {
            Ok(_) => panic!("test source bypassed ordinary private access"),
            Err(errors) => errors,
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("not accessible")),
            "{errors:#?}"
        );
    }
}
