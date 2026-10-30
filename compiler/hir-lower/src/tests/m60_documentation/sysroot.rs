use super::*;

#[test]
fn m60_documentation_projects_real_core_sources_and_canonical_annotations() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/src/scoop.core");
    let paths = core::sysroot_sources();
    let mut parsed = Vec::new();
    let mut texts = Vec::new();
    let mut contexts = Vec::new();
    let mut documentation = Vec::new();
    for (index, path) in paths.into_iter().enumerate() {
        let mut text = std::fs::read_to_string(&path).unwrap();
        if index == 0 {
            text.push_str("\npublic fun documentationInferredInteger() = 42\n");
            text.push_str("public fun documentationInferredBoolean() = true\n");
            text.push_str("public fun documentationInferredString() = \"text\"\n");
        }
        let identity = core_source_identity(
            &path
                .strip_prefix(&root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/"),
        );
        let syntax = scoop_parser::parse(&text).unwrap();
        documentation.push(ast::IdentifiedSourceDocumentation {
            source: identity.clone(),
            documentation: scoop_doc::prepare_source(&syntax, &text),
        });
        parsed.push(ast::IdentifiedParsedSource::new(identity.clone(), syntax));
        texts.push(ast::CurrentSourceText::new(identity.clone(), text));
        contexts.push(ast::CurrentSourceDiagnosticContext::new(identity, path));
    }
    let sources = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(parsed.remove(0), parsed)).unwrap(),
        ast::NonEmptyVec::new(texts.remove(0), texts),
        ast::NonEmptyVec::new(contexts.remove(0), contexts),
    )
    .unwrap();
    let world =
        hir::ImportedSemanticWorld::from_dependencies(sources.cone(), Vec::new(), Vec::new())
            .unwrap();
    let input = crate::CurrentConeSources::try_new(
        &sources,
        crate::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap()
    .with_documentation(documentation)
    .unwrap();
    let output =
        crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let docs = output.output().documentation.as_ref().unwrap();
    assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
    assert!(docs.hidden_annotation.is_some());
    assert!(docs.alias_annotation.is_some());
    let integer = named(&output, Kind::Struct, "Int");
    assert!(
        integer
            .comment
            .as_ref()
            .unwrap()
            .body
            .text
            .text
            .contains("integer")
    );
    let location = named(&output, Kind::Function, "currentSourceLocation");
    assert!(location.comment.is_some());
    let Target::Declaration(Source::Annotation(alias)) =
        named(&output, Kind::Annotation, "DocAlias").target
    else {
        panic!("DocAlias has an annotation identity");
    };
    assert_eq!(docs.alias_annotation, Some(alias));
    let input = scoop_doc::project_cone(
        &output.output().export,
        docs,
        &scoop_identity::ConeCoordinate::reserved_core(),
        scoop_identity::RequestedConeKind::Library,
    )
    .unwrap();
    assert!(input.warnings.is_empty(), "{:?}", input.warnings);
    let integer = input
        .items
        .iter()
        .find(|item| item.kind == scoop_doc::ApiKind::Struct && item.name == "Int")
        .unwrap();
    assert!(integer.aliases.contains(&"integer".into()));
    assert!(
        input
            .visible_items(false)
            .iter()
            .any(|item| item.key == integer.key)
    );
    assert!(input.sources.iter().all(|source| !source.text.is_empty()));
    for (function, nominal) in [
        ("documentationInferredInteger", "Int"),
        ("documentationInferredBoolean", "Boolean"),
        ("documentationInferredString", "String"),
    ] {
        let function = input
            .items
            .iter()
            .find(|item| item.name == function)
            .unwrap();
        let nominal = input
            .items
            .iter()
            .find(|item| item.name == nominal && item.parent.is_none())
            .unwrap();
        assert!(
            function
                .signature
                .iter()
                .any(|token| token.link.as_ref() == Some(&nominal.key))
        );
    }
}
