use super::*;

fn collect<'a>(
    lowerer: &mut Lowerer,
    core: &'a ast::SourceFile,
    source: &str,
    parsed: &'a scoop_parser::ParsedFile,
) -> SourceDeclarations<'a> {
    assert!(parsed.ast().is_none());
    let projections = parsed.declaration_data();
    let header = projections
        .source
        .as_ref()
        .unwrap()
        .header(Span::new(0, source.len() as u32))
        .unwrap();
    let complete = projections
        .declarations
        .iter()
        .filter_map(|item| {
            matches!(item.value, ast::DeclarationFragment::Complete(_)).then_some(item.declaration)
        })
        .collect::<std::collections::HashSet<_>>();
    let recovered = projections.declarations.iter().filter_map(|item| {
        let syntax = &parsed.declarations()[item.declaration.0];
        if syntax.parent.is_some() {
            return None;
        }
        match &item.value {
            ast::DeclarationFragment::Complete(declaration) => {
                Some((1, ast::DeclarationInput::Complete(declaration)))
            }
            ast::DeclarationFragment::FunctionHeader(header)
                if !complete.contains(&item.declaration) =>
            {
                Some((
                    1,
                    ast::DeclarationInput::FunctionHeader(ast::FunctionHeaderRef {
                        signature: &header.signature,
                        body_form: header.body_form,
                        span: Span::new(header.signature.span.start, syntax.span.end),
                    }),
                ))
            }
            ast::DeclarationFragment::FunctionHeader(_) => None,
        }
    });
    let core_inputs = core
        .declarations
        .iter()
        .map(|declaration| (0, ast::DeclarationInput::Complete(declaration)));
    lowerer
        .collect_declaration_inputs(
            vec![core.header(), header],
            core_inputs.chain(recovered),
            None,
        )
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
