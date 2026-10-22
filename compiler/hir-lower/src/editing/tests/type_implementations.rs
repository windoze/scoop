use super::imports::{analyze_files, at};
use super::*;

fn target(
    analysis: &mut DeclarationAnalysis,
    text: &str,
    needle: &str,
    length: usize,
) -> hir::SourceNominalId {
    let origin = at("src/relations.scoop", text, needle, length);
    let V::Known(targets) = analysis
        .type_implementation_targets(&origin.source, origin.span, || false)
        .unwrap()
    else {
        panic!("known target: {:?}", analysis.diagnostics());
    };
    assert_eq!(targets.len(), 1, "{targets:?}");
    targets[0]
}

#[test]
fn nominal_implementations_reuse_generic_diamonds_and_class_ancestry() {
    let text = "interface Root<T> {}\ninterface Left<T> : Root<T> {}\ninterface Right<T> : Root<T> {}\nabstract class Base<T> : Left<T> {}\nclass Leaf : Base<Boolean>(), Right<Boolean> {}\nstruct Value(val flag: Boolean) : Root<Boolean> {}\nobject Single : Root<Boolean> {}\nfun damaged(value: Root<Boolean>) { value. }";
    let mut analysis = analyze_files(&[("src/relations.scoop", text)]);
    let root = target(&mut analysis, text, "Root<T>", 4);
    let V::Known(implementations) = analysis.type_implementations(root, || false).unwrap() else {
        panic!("known relationships: {:?}", analysis.diagnostics());
    };
    let mut names = implementations
        .iter()
        .map(|implementation| {
            &text
                [implementation.origin.span.start as usize..implementation.origin.span.end as usize]
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["Base", "Leaf", "Left", "Right", "Single", "Value"]);
    let base = target(&mut analysis, text, "Base<T>", 4);
    let V::Known(implementations) = analysis.type_implementations(base, || false).unwrap() else {
        panic!("known base");
    };
    assert_eq!(implementations.len(), 1);
    assert_eq!(
        implementations[0].origin,
        at("src/relations.scoop", text, "Leaf :", 4)
    );
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.recovered_facts.is_empty());
}

#[test]
fn aliases_and_exact_imports_use_the_actual_nominal_target() {
    let text = "package api\ninterface Root<T> {}\ntypealias Same<T> = Root<T>\nclass Child : Root<Boolean> {}";
    let consumer = "import api.Root as Parent\nimport api.Same\nfun use(value: Same<Boolean>): Parent<Boolean> = value";
    let mut analysis = analyze_files(&[("src/relations.scoop", text), ("src/use.scoop", consumer)]);
    let root = target(&mut analysis, text, "Root<T>", 4);
    for (needle, length) in [
        ("Root as", 4),
        ("Parent\n", 6),
        ("Same<Boolean>", 4),
        ("Parent<Boolean>", 6),
    ] {
        let origin = at("src/use.scoop", consumer, needle, length);
        assert!(
            matches!(analysis.type_implementation_targets(&origin.source, origin.span, || false).unwrap(), V::Known(targets) if targets == [root])
        );
    }
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn cancellation_does_not_publish_a_partial_index_and_failed_relations_are_unavailable() {
    let text = "interface Root {}\nclass Child : Root {}";
    let mut analysis = analyze_files(&[("src/relations.scoop", text)]);
    let root = target(&mut analysis, text, "Root {}", 4);
    let checks = std::cell::Cell::new(0);
    assert!(matches!(
        analysis.type_implementations(root, || {
            checks.set(checks.get() + 1);
            checks.get() == 3
        }),
        Err(EditingCancelled)
    ));
    assert!(analysis.type_implementations.is_none());
    assert!(
        matches!(analysis.type_implementations(root, || false).unwrap(), V::Known(values) if values.len() == 1)
    );
    checks.set(0);
    assert!(matches!(analysis.type_implementations(root, || {
        checks.set(checks.get() + 1);
        false
    }).unwrap(), V::Known(values) if values.len() == 1));
    assert_eq!(checks.get(), 1);

    let mut failed = analyze_files(&[("src/relations.scoop", "interface Root : Root {}")]);
    assert!(matches!(
        failed.type_implementations(root, || false).unwrap(),
        V::Unavailable(U::DeclarationFailure)
    ));
    assert!(failed.type_implementations.is_none());
}
