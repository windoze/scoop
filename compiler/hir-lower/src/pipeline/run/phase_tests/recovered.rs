use super::*;

mod unavailable;

fn collect<'a>(
    lowerer: &mut Lowerer,
    core: &'a ast::SourceFile,
    source: &str,
    parsed: &'a scoop_parser::ParsedFile,
) -> SourceDeclarations<'a> {
    collect_files(lowerer, core, &[(source, parsed)])
}

fn collect_files<'a>(
    lowerer: &mut Lowerer,
    core: &'a ast::SourceFile,
    source_files: &[(&str, &'a scoop_parser::ParsedFile)],
) -> SourceDeclarations<'a> {
    let mut headers = vec![core.header()];
    let mut inputs = core
        .declarations
        .iter()
        .map(|declaration| (0, ast::DeclarationInput::Complete(declaration)))
        .collect::<Vec<_>>();
    let identities = source_files
        .iter()
        .enumerate()
        .map(|(index, _)| crate::tests::test_source_identity(&format!("src/user{index}.scoop")))
        .collect::<Vec<_>>();
    for (index, &(text, parsed)) in source_files.iter().enumerate() {
        let source = scoop_parser::declaration_source(
            parsed,
            &identities[index],
            lowerer.intrinsic_sources[index + 1].role,
            text,
        );
        headers.push(source.header.unwrap());
        inputs.extend(
            source
                .declarations
                .into_iter()
                .map(|declaration| (index + 1, declaration)),
        );
    }
    lowerer
        .collect_declaration_inputs(headers, inputs, None)
        .unwrap()
}

#[test]
fn recovered_callable_participates_in_imports_defaults_and_ordinary_calls() {
    let source = "package api\nimport api.editing as invoke\n\
        fun editing(value: Boolean = true): Boolean { value. }\n\
        fun healthy(): Boolean = invoke()";
    let core = crate::tests::core_file();
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    let mut lowerer = source_lowerer(source);
    let declarations = collect(&mut lowerer, &core, source, &parsed);
    lowerer.resolve_source_signatures(&declarations).unwrap();
    let editing = declarations
        .function_headers
        .iter()
        .find(|(_, header, _)| header.name.text == "editing")
        .unwrap()
        .0;
    assert!(
        !declarations
            .pending_functions
            .iter()
            .any(|(id, _, _)| *id == editing)
    );
    assert_eq!(lowerer.signatures[&editing].params[0].ty, lowerer.boolean);
    assert_eq!(lowerer.signatures[&editing].return_ty, lowerer.boolean);
    let protocols = lowerer.complete_declaration_bodies(&declarations).unwrap();
    lowerer.lower_source_bodies(&declarations, protocols.type_reference);
    assert!(lowerer.diagnostics.is_empty(), "{:?}", lowerer.diagnostics);
}

#[test]
fn recovered_and_complete_headers_share_duplicate_signature_rejection() {
    let source = "fun same(value: Boolean): Boolean { value. }\n\
        fun same(value: Boolean): Boolean = false\nfun healthy(): Boolean = true";
    let core = crate::tests::core_file();
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    let mut lowerer = source_lowerer(source);
    let declarations = collect(&mut lowerer, &core, source, &parsed);
    lowerer.resolve_source_signatures(&declarations).unwrap();
    let mut rejected = 0;
    for &(id, header, file) in &declarations.function_headers {
        if file != 1 {
            continue;
        }
        assert_eq!(
            lowerer.declaration_surface.rejects_function(id),
            header.name.text == "same"
        );
        rejected += usize::from(lowerer.declaration_surface.rejects_function(id));
    }
    assert_eq!(rejected, 2);
    assert!(
        lowerer
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message
                == "function `same` is already declared with the same signature")
    );
}

#[test]
fn missing_expression_body_keeps_its_result_unavailable() {
    let source = "fun unfinished() = value.\nfun healthy(): Boolean = true";
    let core = crate::tests::core_file();
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    let mut lowerer = source_lowerer(source);
    let declarations = collect(&mut lowerer, &core, source, &parsed);
    lowerer.resolve_source_signatures(&declarations).unwrap();
    let unfinished = declarations
        .function_headers
        .iter()
        .find(|(_, header, _)| header.name.text == "unfinished")
        .unwrap()
        .0;
    assert!(lowerer.signatures[&unfinished].result_pending);
    assert!(!lowerer.has_inferred_function(unfinished));
    assert_eq!(lowerer.infer_function_result(unfinished), None);
}
