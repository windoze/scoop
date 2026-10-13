use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{ConeCoordinate, RequestedConeKind, SourceRole};

use super::support::{MARKERS, marker_core};
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use crate::{CurrentConeSources, lower_current_cone};

fn lower(text: &str, role: SourceRole) -> Result<hir::DependencyHirOutput, Vec<ast::Diagnostic>> {
    let mut core = marker_core();
    let coordinate = ConeCoordinate::new("scoop", "scoop.test", "0.1.0").unwrap();
    let (foundation, interface) = project_dependency_text(&core, &coordinate, MARKERS, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 182);
    let identity = crate::tests::test_source_identity("tests/check.scoop");
    let sources = scoop_parser::parse_current_cone(ast::NonEmptyVec::new(
        scoop_parser::CurrentConeSourceInput::new(&identity, text, "tests/check.scoop".as_ref())
            .with_role(role),
        vec![],
    ))
    .unwrap();
    let world = hir::ImportedSemanticWorld::from_dependencies(
        sources.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &imported,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        vec![],
    )
    .unwrap();
    let input = CurrentConeSources::try_new(
        &sources,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    lower_current_cone(RequestedConeKind::Library, &input)
}

#[test]
fn imported_marker_aliases_keep_their_test_semantics() {
    for prefix in ["import scoop.test.Test as Check", "import scoop.test.*"] {
        let name = if prefix.ends_with("Check") {
            "Check"
        } else {
            "Test"
        };
        let source = format!("{prefix}\n@{name} class Cases {{ @{name} fun check() {{}} }}");
        lower(&source, SourceRole::Test).unwrap_or_else(|errors| panic!("{errors:#?}"));
        let errors = match lower(&source, SourceRole::Production) {
            Ok(_) => panic!("imported markers must reject production sources"),
            Err(errors) => errors,
        };
        assert_eq!(errors.len(), 2, "{errors:#?}");
        assert!(
            errors
                .iter()
                .all(|error| error.message.contains("only allowed in test sources"))
        );
    }
}

#[test]
fn same_package_annotation_conflicts_use_normal_name_binding() {
    let errors = match lower(
        "package scoop.test\nannotation class Test\n@Test struct Ordinary(val value: Int)",
        SourceRole::Production,
    ) {
        Ok(_) => panic!("different declarations in the same package must be ambiguous"),
        Err(errors) => errors,
    };
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_eq!(
        errors[0].message,
        "type `Test` is ambiguous in the current package layer"
    );
    assert_eq!(errors[0].notes.len(), 2);
}
