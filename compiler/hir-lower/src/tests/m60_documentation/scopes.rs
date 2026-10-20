use super::*;

#[test]
fn m60_documentation_keeps_file_import_scopes_and_reports_ambiguous_names() {
    let core = support::trusted_core();
    let mut inputs = [
        ("a.scoop", "package a\npublic class Item"),
        ("b.scoop", "package b\npublic class Item"),
        (
            "left.scoop",
            "import a.Item as Choice\n/** [Choice] */\npublic fun left() {}",
        ),
        (
            "right.scoop",
            "import b.Item as Choice\n/** [Choice] */\npublic fun right() {}",
        ),
        (
            "both.scoop",
            "import a.*\nimport b.*\n/** [Item] */\npublic fun both() {}",
        ),
    ];
    inputs.sort_by_key(|(path, _)| *path);
    let mut parsed = Vec::new();
    let mut texts = Vec::new();
    let mut contexts = Vec::new();
    let mut documentation = Vec::new();
    for (path, text) in inputs {
        let identity = test_source_identity(path);
        let syntax = scoop_parser::parse(text).unwrap();
        documentation.push(ast::IdentifiedSourceDocumentation {
            source: identity.clone(),
            documentation: scoop_doc::prepare_source(&syntax, text),
        });
        parsed.push(ast::IdentifiedParsedSource::new(identity.clone(), syntax));
        texts.push(ast::CurrentSourceText::new(identity.clone(), text.into()));
        contexts.push(ast::CurrentSourceDiagnosticContext::new(
            identity,
            path.into(),
        ));
    }
    let sources = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(parsed.remove(0), parsed)).unwrap(),
        ast::NonEmptyVec::new(texts.remove(0), texts),
        ast::NonEmptyVec::new(contexts.remove(0), contexts),
    )
    .unwrap();
    let world = core.world(sources.cone());
    let request = crate::CurrentConeSources::try_new(
        &sources,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap()
    .with_documentation(documentation)
    .unwrap();
    let output =
        crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &request).unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert_eq!(docs.warnings.len(), 1, "{:?}", docs.warnings);
    assert!(docs.warnings[0].message.contains("is ambiguous"));
    assert_eq!(
        docs.warnings[0].source.as_deref(),
        Some(&test_source_identity("both.scoop"))
    );
    let left = current(&comment(&output, Kind::Function, "left").body.references[0].resolution);
    let right = current(&comment(&output, Kind::Function, "right").body.references[0].resolution);
    assert_ne!(left, right);
    assert_eq!(
        docs.declarations
            .iter()
            .find(|entry| entry.target == left)
            .unwrap()
            .origin
            .file,
        0
    );
    assert_eq!(
        docs.declarations
            .iter()
            .find(|entry| entry.target == right)
            .unwrap()
            .origin
            .file,
        1
    );
}
