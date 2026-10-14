use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{ConeCoordinate, RequestedConeKind, SourceRole};

use super::m23_ordinary_core_only::support::trusted_core;
use super::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use crate::{CurrentConeSources, lower_current_cone};

fn lower(
    production_dependency: &str,
    test_dependency: &str,
    production: &str,
    tests: &str,
) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    let mut core = trusted_core();
    let prepared = [
        ("production-provider", production_dependency),
        ("test-provider", test_dependency),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (name, source))| {
        let coordinate = ConeCoordinate::new("test", name, "1.0.0").unwrap();
        let (foundation, interface) = project_dependency_text(&core, &coordinate, source, &[]);
        let aliases = alias_expansions(interface.type_aliases());
        let imported =
            core.import_dependency_foundation(&coordinate, &foundation, 171 + index as u8);
        (coordinate.identity().unwrap(), imported, interface, aliases)
    })
    .collect::<Vec<_>>();
    let production_identity = super::test_source_identity("src/api.scoop");
    let test_identity = super::test_source_identity("tests/check.scoop");
    let sources = scoop_parser::parse_current_cone(ast::NonEmptyVec::new(
        scoop_parser::CurrentConeSourceInput::new(
            &production_identity,
            production,
            "src/api.scoop".as_ref(),
        ),
        vec![
            scoop_parser::CurrentConeSourceInput::new(
                &test_identity,
                tests,
                "tests/check.scoop".as_ref(),
            )
            .with_role(SourceRole::Test),
        ],
    ))
    .unwrap();
    let providers = prepared
        .iter()
        .map(
            |(_, foundation, interface, alias_expansions)| hir::ImportedProviderInput {
                foundation,
                interface,
                alias_expansions,
            },
        );
    let world = hir::ImportedSemanticWorld::from_dependencies(
        sources.cone(),
        std::iter::once(core.provider()).chain(providers).collect(),
        vec![],
    )
    .unwrap();
    let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&sources, protocols, &world)
        .unwrap()
        .with_test_dependencies([prepared[1].0])
        .unwrap();
    lower_current_cone(RequestedConeKind::Library, &input)
}

#[test]
fn dependency_roles_filter_overloads_in_each_import_layer() {
    for (prefix, call) in [
        ("package api", "choose"),
        ("import api.choose", "choose"),
        ("import api.choose as selected", "selected"),
        ("import api.*", "choose"),
    ] {
        lower(
            "package api\npublic fun choose(value: Any): Int = 7",
            "package api\npublic fun choose(value: Int): String = \"test\"",
            &format!("{prefix}\nfun production(): Int = {call}(1)\nfun defaulted(value: Int = {call}(1)): Int = value\nfun <T> generic(value: T): Int = {call}(1)"),
            &format!("{prefix}\nfun check(): String = {call}(1)"),
        ).unwrap_or_else(|errors| panic!("{prefix}: {errors:#?}"));
    }
}

#[test]
fn test_dependency_types_and_values_do_not_complete_production_names() {
    for production in [
        "import helpers.Hidden\nfun production(): Hidden = Hidden()",
        "fun production(value: helpers.Hidden) {}",
        "package helpers\nfun production(): Hidden = Hidden()",
        "import helpers.hiddenValue\nval production: Int = hiddenValue",
        "import helpers.*\nfun production(): Int = hidden()",
    ] {
        let errors = match lower(
            "public fun available(): Int = 1",
            "package helpers\npublic class Hidden public constructor() {}\npublic val hiddenValue: Int = 2\npublic fun hidden(): Int = 3",
            production,
            "import helpers.Hidden\nfun check(): Hidden = Hidden()",
        ) {
            Ok(_) => panic!("production source used a test-only dependency"),
            Err(errors) => errors,
        };
        assert!(
            errors.iter().any(|error| error
                .source
                .as_deref()
                .is_some_and(|source| source.logical_path().as_str() == "src/api.scoop")
                && error.span.is_some()),
            "{errors:#?}"
        );
    }
}

#[test]
fn test_package_does_not_shadow_a_production_object() {
    lower(
        "package api\npublic object Tools { public val answer: Int = 42 }",
        "package api.Tools\npublic class Noise public constructor()",
        "import api.Tools.answer\nfun production(): Int = answer",
        "import api.Tools.Noise\nfun check(): api.Tools.Noise = Noise()",
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
}

#[test]
fn test_dependency_extension_calls_are_selected_only_in_test_sources() {
    for prefix in ["package api", "import api.choose", "import api.*"] {
        lower(
            "package api\npublic fun Any.choose(): Int = 7",
            "package api\npublic fun Int.choose(): String = \"test\"",
            &format!("{prefix}\nfun production(): Int = 1.choose()"),
            &format!("{prefix}\nfun check(): String = 1.choose()"),
        )
        .unwrap_or_else(|errors| panic!("{prefix}: {errors:#?}"));
    }
}

#[test]
fn test_dependency_type_conflicts_are_removed_before_production_type_selection() {
    lower(
        "package api\npublic class Value public constructor() { public fun answer(): Int = 42 }",
        "package api\npublic class Value",
        "import api.Value\nfun production(value: api.Value): Int = value.answer()\nfun create(): api.Value = Value()",
        "fun helper() {}",
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
}

#[test]
fn test_dependencies_must_be_non_core_direct_dependencies() {
    let core = trusted_core();
    let sources = super::m23_ordinary_core_only::support::parsed_ordinary_text("fun helper() {}");
    let world = core.world(sources.cone());
    for (dependency, expected) in [
        (
            scoop_identity::ConeIdentity::CORE,
            crate::CurrentConeSourceError::TestOnlyCore,
        ),
        (
            sources.cone(),
            crate::CurrentConeSourceError::TestDependencyNotDirect(sources.cone()),
        ),
    ] {
        let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
        let input = CurrentConeSources::try_new(&sources, protocols, &world).unwrap();
        assert!(
            matches!(input.with_test_dependencies([dependency]), Err(error) if error == expected)
        );
    }
}
