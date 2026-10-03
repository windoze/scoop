use super::*;
use scoop_hir as hir;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-imported-core-members")
            .join(name),
    )
    .unwrap()
}

#[test]
fn imported_integer_equality_needs_no_external_callables() {
    constants::with_input(&fixture("equality.scoop"), |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
        assert!(output.imported_dependencies().is_empty());
    });
}

#[test]
fn imported_integer_literal_patterns_keep_complete_equality_plans() {
    constants::with_input(&fixture("equality-patterns.scoop"), |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
        let export = output.output().export.module();
        let mut kinds = std::collections::BTreeSet::new();
        for (_, function) in export
            .functions
            .iter()
            .filter(|(_, function)| function.name.starts_with("pattern"))
        {
            let hir::FunctionKind::User(body) = &function.kind else {
                panic!("ordinary function body")
            };
            let when = body
                .statements
                .iter()
                .find_map(|statement| match &statement.kind {
                    hir::StatementKind::When(when) => Some(when),
                    _ => None,
                })
                .expect("pattern function contains a when");
            let hir::Pattern::Literal {
                value,
                equality: hir::LiteralPatternEquality::Integer { kind },
                ..
            } = &when.arms[0].pattern
            else {
                panic!("integer literal pattern carries its normalized equality");
            };
            assert_eq!(export.types[value.ty], hir::Type::Integer(*kind));
            kinds.insert(*kind);
        }
        assert_eq!(kinds, hir::IntegerKind::ALL.into_iter().collect());
        assert!(output.imported_dependencies().is_empty());
    });
}
