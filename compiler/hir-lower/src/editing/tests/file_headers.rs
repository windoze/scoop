use super::imports::{analyze_files, at};
use super::*;

pub(super) fn available(analysis: &mut DeclarationAnalysis, name: &str) -> bool {
    let function = function(analysis, name);
    matches!(
        analysis.function_facts(function, || false).unwrap(),
        V::Known(_)
    )
}

#[test]
fn unavailable_headers_preserve_file_indexes_and_independent_bodies() {
    let provider = "package api\nfun other(value: Boolean): Boolean = value";
    let consumer = "import api.other\nfun editing(value: Boolean): Boolean = other(value)";
    for broken in [
        "package api.\nfun ghost(): Boolean = true",
        "package ignored\nimport api.\nfun ghost(): Boolean = true",
    ] {
        for bad_index in 0..3 {
            let mut inputs = vec![("src/api.scoop", provider), ("src/use.scoop", consumer)];
            inputs.insert(bad_index, ("src/broken.scoop", broken));
            let mut analysis = analyze_files(&inputs);
            assert!(
                available(&mut analysis, "editing"),
                "{:?}",
                analysis.diagnostics()
            );
            assert!(available(&mut analysis, "other"));
            assert!(
                !analysis
                    .functions()
                    .iter()
                    .any(|function| function.name == "ghost")
            );
            let diagnostic = analysis
                .diagnostics()
                .into_iter()
                .find(|error| error.message.contains("package or imports"))
                .unwrap();
            assert_eq!(diagnostic.file, bad_index);
            assert_eq!(
                diagnostic.source.as_deref(),
                Some(&crate::tests::test_source_identity("src/broken.scoop"))
            );
            let index = analysis.reference_index(|| false).unwrap();
            assert!(!index.declarations_available);
            assert_eq!(
                index.unavailable,
                [at("src/broken.scoop", broken, broken, broken.len())]
            );
            let target = index.targets_at(&at("src/api.scoop", provider, "other", 5))[0];
            assert!(
                index.references_to(target).any(|reference| reference.origin
                    == at("src/use.scoop", consumer, "other(value)", 5))
            );
        }
    }
}

#[test]
fn damaged_header_does_not_erase_a_recovered_receiver_context() {
    let mut analysis = analyze_files(&[
        ("src/broken.scoop", "package\nfun hidden() {}"),
        ("src/use.scoop", "fun editing(参数: String) { 参数. }"),
    ]);
    let function = function(&analysis, "editing");
    let receiver = analysis.receivers(function, || false).unwrap()[0].clone();
    let V::Known(value) = receiver.value_type else {
        panic!("independent parameter type")
    };
    let V::Known(description) = analysis
        .describe_body_type(function, value.context, value.ty)
        .unwrap()
    else {
        panic!("actual source context")
    };
    assert_eq!(description.name, "String");
    assert_eq!(
        receiver
            .local
            .unwrap()
            .definition
            .source
            .logical_path()
            .as_str(),
        "src/use.scoop"
    );
}

#[test]
fn fixing_a_header_replaces_unavailability_with_the_current_declaration() {
    let consumer = "import api.target\nfun editing(): Boolean = target()";
    let broken = "package api\nimport api.\nfun target(): Boolean = true";
    let fixed = "package api\nfun target(): Boolean = true";
    let mut first = analyze_files(&[("src/broken.scoop", broken), ("src/use.scoop", consumer)]);
    assert!(!available(&mut first, "editing"));
    assert!(
        !first
            .reference_index(|| false)
            .unwrap()
            .declarations_available
    );
    let mut next = analyze_files(&[("src/broken.scoop", fixed), ("src/use.scoop", consumer)]);
    assert!(available(&mut next, "editing"), "{:?}", next.diagnostics());
    let index = next.reference_index(|| false).unwrap();
    assert!(index.declarations_available);
    assert!(index.unavailable.is_empty());
    let target = index.targets_at(&at("src/broken.scoop", fixed, "target", 6))[0];
    assert_eq!(index.references_to(target).count(), 3);
    assert!(
        !first
            .reference_index(|| false)
            .unwrap()
            .declarations_available
    );
}

#[test]
fn only_unavailable_files_form_an_incomplete_index_without_a_fake_root_package() {
    let mut analysis = analyze_files(&[("src/broken.scoop", "package\nfun ghost() {}")]);
    assert!(analysis.functions().is_empty());
    assert!(
        analysis
            .lowerer
            .top_level_namespaces
            .known_source_namespace(0)
            .is_none()
    );
    assert!(analysis.lowerer.imports.files.is_empty());
    let index = analysis.reference_index(|| false).unwrap();
    assert!(!index.declarations_available);
    assert_eq!(index.unavailable.len(), 1);
    assert!(index.occurrences().is_empty());
}

#[test]
fn an_incomplete_core_header_uses_its_real_diagnostic_range() {
    let text = "package\nfun ghost() {}";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let identity = crate::tests::core_source_identity("src/broken.scoop");
    let input = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let world =
        hir::ImportedSemanticWorld::from_dependencies(ConeIdentity::CORE, Vec::new(), Vec::new())
            .unwrap();
    let mut analysis = analyze_declarations(
        &[input],
        &CoreProtocolInput::CurrentDeclarations,
        &world,
        &[],
        || false,
    )
    .unwrap();
    assert!(analysis.functions().is_empty());
    assert!(
        analysis
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.source.as_deref() == Some(&identity))
    );
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Option")
                && diagnostic.span == Some(ast::Span::new(0, text.len() as u32)))
    );
    assert!(
        !analysis
            .reference_index(|| false)
            .unwrap()
            .declarations_available
    );
}
