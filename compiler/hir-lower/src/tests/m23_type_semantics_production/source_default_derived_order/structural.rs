use super::*;
use source_dispatch::with_hir_sources;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/structural-defaults.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/structural-default-combinations.scoop"
));

#[test]
fn structural_defaults_follow_operand_visibility_through_hir_and_mir() {
    let mut snapshot = String::new();
    for (source, names) in [
        (SOURCE, &["unitDefault", "tupleDefault"][..]),
        (
            COMBINATIONS,
            &["nestedDefault", "Base.same", "Child.same"][..],
        ),
    ] {
        with_source(source, |output, mir| {
            assert!(!mir.functions.is_empty());
            snapshot.push_str(&functions(output, names));
            if source == COMBINATIONS {
                let export = output.output().export.module();
                let (_, holder) = export
                    .classes
                    .iter()
                    .find(|(_, c)| c.name == "Holder")
                    .unwrap();
                let owner = hir::ExportParameterOwner::ClassConstructor(holder.constructors[0]);
                check(output, owner, 0);
                snapshot.push_str("Holder: DerivedEquality Universal\n");
            }
        });
    }
    assert_eq!(
        snapshot,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/structural-defaults.hir.snap"
        ))
    );
}

#[test]
fn structural_default_access_is_independent_of_the_helpers_first_source_file() {
    let warmup = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/structural-default-warmup.scoop"
    ));
    for files in [
        [("src/first.scoop", warmup), ("src/second.scoop", SOURCE)],
        [("src/first.scoop", SOURCE), ("src/second.scoop", warmup)],
    ] {
        with_hir_sources(&files, |output, _| {
            assert_eq!(
                functions(output, &["unitDefault", "tupleDefault"]),
                "unitDefault: DerivedEquality Universal\ntupleDefault: DerivedEquality Universal\n"
            );
        });
    }
}

#[test]
fn structural_default_equality_keeps_hidden_tuple_element_domains() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/errors/structural-default-hidden.scoop"
    ));
    let diagnostics =
        lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap_err();
    let failures = diagnostics
        .iter()
        .filter(|d| {
            d.message == "default expression references a callable outside the callable's complete call domain"
        })
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1, "{diagnostics:?}");
    let expression = "(Hidden(), Hidden()) == (Hidden(), Hidden())";
    let start = source.find(expression).unwrap() as u32;
    assert_eq!(failures[0].file, 1);
    assert_eq!(
        failures[0].span,
        Some(ast::Span::new(start, start + expression.len() as u32))
    );
}

fn functions(output: &hir::DependencyHirOutput, names: &[&str]) -> String {
    let export = output.output().export.module();
    let mut result = String::new();
    for name in names {
        let id = export
            .functions
            .iter()
            .find(|(_, f)| f.name == *name)
            .unwrap()
            .0;
        check(output, hir::ExportParameterOwner::Function(id), 2);
        result.push_str(&format!("{name}: DerivedEquality Universal\n"));
    }
    result
}

fn check(output: &hir::DependencyHirOutput, owner: hir::ExportParameterOwner, position: usize) {
    let source = default_expression(output, owner, position);
    let references = source
        .references
        .callables
        .iter()
        .filter(|r| {
            matches!(
                r.target,
                hir::ExportDefaultCallableTarget::DerivedEquality(_)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 1);
    assert!(references[0].target_domain.is_universal());
}
