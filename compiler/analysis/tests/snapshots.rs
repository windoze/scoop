use std::sync::Arc;

use scoop_analysis::{AnalysisHost, Cancelled};
use scoop_source::SourceText;

fn text(source: &str) -> SourceText {
    SourceText::from(source.to_owned())
}

#[test]
fn changed_files_replace_their_parse_and_preserve_other_inputs() {
    let mut host = AnalysisHost::default();
    let first = host.add_source(text("fun first() = 1"));
    let second = host.add_source(text("fun second() = 2"));
    let previous = host.snapshot();
    let previous_first = previous.source(first).unwrap().unwrap().clone();
    let previous_second = previous.source(second).unwrap().unwrap().clone();
    let second_parse = previous_second.parse().unwrap();
    assert!(host.set_text(first, text("fun changed() { value. }")));
    let current = host.snapshot();
    assert_eq!(previous.check_cancelled(), Err(Cancelled));
    assert!(current.check_cancelled().is_ok());
    let current_first = current.source(first).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&previous_first, current_first));
    assert_eq!(previous_first.text.as_str(), "fun first() = 1");
    assert!(current_first.parse().unwrap().ast().is_none());
    let current_second = current.source(second).unwrap().unwrap();
    assert!(Arc::ptr_eq(&previous_second, current_second));
    assert!(std::ptr::eq(second_parse, current_second.parse().unwrap()));
}

#[test]
fn identical_text_reuses_the_revision_and_released_sources_do_not_reappear() {
    let mut host = AnalysisHost::default();
    let source = host.add_source(text("fun f() = 1"));
    let previous = host.snapshot();
    assert!(host.set_text(source, text("fun f() = 1")));
    assert_eq!(host.revision(), previous.revision);
    assert!(previous.check_cancelled().is_ok());
    host.remove_source(source);
    assert_eq!(previous.check_cancelled(), Err(Cancelled));
    assert!(!host.set_text(source, text("fun f() = 2")));
    assert!(host.snapshot().source(source).unwrap().is_none());
    let replacement = host.add_source(text("fun f() = 3"));
    assert_ne!(replacement, source);
}

#[test]
fn declaration_inputs_borrow_the_current_grammar_projection() {
    let mut host = AnalysisHost::default();
    let source = host.add_source(text(
        "fun editing(value: Boolean): Boolean { value. }\nfun good(): Boolean = true",
    ));
    let snapshot = host.snapshot();
    let source = snapshot.source(source).unwrap().unwrap();
    let parsed = source.parse().unwrap();
    let identity = scoop_identity::SourceIdentity::new(
        scoop_identity::ConeIdentity::CORE,
        scoop_identity::NormalizedSourcePath::new("src/editing.scoop").unwrap(),
    );
    let input = source
        .declaration_source(&identity, scoop_identity::SourceRole::Test)
        .unwrap();
    assert_eq!(input.role, scoop_identity::SourceRole::Test);
    assert_eq!(input.text, source.text.as_str());
    assert_eq!(input.declarations.len(), 2);
    let scoop_ast::DeclarationInput::RecoveredFunction { header, body } = input.declarations[0]
    else {
        panic!("the first source declaration has a complete header");
    };
    assert_eq!(body.selectors.len(), 1);
    let original = parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| match &item.value {
            scoop_ast::DeclarationFragment::FunctionHeader(header)
                if header.signature.name.text == "editing" =>
            {
                Some(&header.signature)
            }
            _ => None,
        })
        .unwrap();
    assert!(std::ptr::eq(header.signature, original));
    assert!(matches!(
        input.declarations[1],
        scoop_ast::DeclarationInput::Complete(_)
    ));
}

#[test]
fn definition_names_require_exact_grammar_declaration_ranges() {
    use scoop_source::Span;
    let mut host = AnalysisHost::default();
    let input = "/// Documentation\n@NoGC fun 函数(value: Boolean): Boolean = value\nfun 函数(value: String): String = value";
    let id = host.add_source(text(input));
    let source = host.source(id).unwrap();
    let boundary = input.find("\nfun").unwrap() as u32;
    let first = Span::new(input.find("@NoGC").unwrap() as u32, boundary);
    let second = Span::new(boundary + 1, input.len() as u32);
    for (declaration, name) in [
        (first, input.find("函数").unwrap()),
        (second, input.rfind("函数").unwrap()),
    ] {
        assert_eq!(
            source.declaration_name_span(declaration).unwrap(),
            Some(Span::new(name as u32, (name + "函数".len()) as u32))
        );
    }
    let parameter = input.find("value: Boolean").unwrap() as u32;
    assert_eq!(
        source
            .declaration_name_span(Span::new(parameter, parameter + 14))
            .unwrap(),
        Some(Span::new(parameter, parameter + 5))
    );
    assert_eq!(
        source
            .declaration_name_span(Span::new(first.start + 1, first.end))
            .unwrap(),
        None
    );
    let value = input.rfind("value").unwrap() as u32;
    assert_eq!(
        source
            .declaration_name_span(Span::new(value, value + 5))
            .unwrap(),
        None
    );
}

#[test]
fn prepared_documentation_belongs_to_the_original_source_version() {
    let mut host = AnalysisHost::default();
    let input = "/// Original **text**.\nfun healthy(): Boolean = true\nfun broken(value: ) {}";
    let id = host.add_source(text(input));
    let before = host.source(id).unwrap().clone();
    let documentation = before.documentation().unwrap();
    assert_eq!(
        documentation.documents[0].body.text.text,
        "Original **text**."
    );
    assert!(std::ptr::eq(documentation, before.documentation().unwrap()));
    host.set_text(id, text(input));
    assert!(std::ptr::eq(
        documentation,
        host.source(id).unwrap().documentation().unwrap()
    ));
    host.set_text(id, text(&input.replace("Original", "Updated")));
    assert_eq!(
        host.source(id).unwrap().documentation().unwrap().documents[0]
            .body
            .text
            .text,
        "Updated **text**."
    );
    assert_eq!(
        documentation.documents[0].body.text.text,
        "Original **text**."
    );
}

#[test]
fn declaration_symbol_indexes_keep_error_names_and_exclude_callable_locals() {
    let mut host = AnalysisHost::default();
    let id = host.add_source(text(
        "class Outer { fun method() { fun local() {} }; class Inner { fun run() {} } }\nfun broken(value: ) {}",
    ));
    let source = host.source(id).unwrap().clone();
    let symbols = source.symbols().unwrap();
    assert!(std::ptr::eq(symbols, source.symbols().unwrap()));
    assert_eq!(
        symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>(),
        ["Outer", "method", "Inner", "run", "broken"]
    );
    assert_eq!(symbols[3].container.as_deref(), Some("Outer.Inner"));
    for symbol in symbols {
        assert_eq!(symbol.declaration.source, source.version);
        assert_eq!(
            &source.text.as_str()[symbol.name_span.start as usize..symbol.name_span.end as usize],
            symbol.name
        );
    }
    host.set_text(id, text("fun replacement() {}"));
    let updated = host.source(id).unwrap().symbols().unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].name, "replacement");
    assert_ne!(updated[0].declaration.source, symbols[0].declaration.source);
}
