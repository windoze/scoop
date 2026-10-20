use super::*;

#[test]
fn m60_documentation_projects_real_core_sources_and_canonical_annotations() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sysroot/src/scoop.core/src");
    let mut paths = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "scoop"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut parsed = Vec::new();
    let mut texts = Vec::new();
    let mut contexts = Vec::new();
    let mut documentation = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        let identity = core_source_identity(&format!(
            "src/{}",
            path.file_name().unwrap().to_str().unwrap()
        ));
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
}
