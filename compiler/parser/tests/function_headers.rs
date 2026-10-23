use scoop_ast::{DeclarationFragment, FunctionBodyForm, FunctionHeader, MethodModifier};
use scoop_parser::{ParsedFile, parse, parse_recovering};
use scoop_syntax::{DeclarationData, SyntaxKind};

fn headers(parsed: &ParsedFile) -> Vec<DeclarationData<&FunctionHeader>> {
    parsed
        .declaration_data()
        .declarations
        .iter()
        .filter_map(|data| {
            let DeclarationFragment::FunctionHeader(header) = &data.value else {
                return None;
            };
            Some(DeclarationData {
                declaration: data.declaration,
                value: header.as_ref(),
            })
        })
        .collect()
}

fn names(parsed: &ParsedFile) -> Vec<&str> {
    headers(parsed)
        .into_iter()
        .map(|header| header.value.signature.name.text.as_str())
        .collect()
}

#[test]
fn broken_body_retains_the_actual_complete_signature() {
    let source = "fun other(): Int = 42\nfun editing(user: User) { user. }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parse(source).is_err());
    assert!(parsed.ast().is_none());
    assert_eq!(names(&parsed), ["other", "editing"]);
    let editing = &headers(&parsed)[1];
    let signature = &editing.value.signature;
    assert_eq!(editing.value.body_form, FunctionBodyForm::Block);
    assert_eq!(signature.params.len(), 1);
    assert_eq!(signature.params[0].name.text, "user");
    assert_eq!(
        &source[signature.span.start as usize..signature.span.end as usize],
        "fun editing(user: User)"
    );
    let declaration = &parsed.declarations()[editing.declaration.0];
    assert_eq!(declaration.kind, SyntaxKind::FunctionDecl);
    assert_eq!(declaration.name.as_ref().unwrap().span, signature.name.span);
    assert!(declaration.parent.is_none());
}

#[test]
fn unfinished_expression_and_unclosed_block_keep_distinct_body_forms() {
    for (source, expected) in [
        ("fun editing(): Int =", FunctionBodyForm::Expression),
        ("fun editing(): Int {", FunctionBodyForm::Block),
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none());
        assert_eq!(names(&parsed), ["editing"]);
        assert_eq!(headers(&parsed)[0].value.body_form, expected);
        assert!(headers(&parsed)[0].value.signature.return_ty.is_some());
    }
}

#[test]
fn recovered_parameter_items_do_not_create_a_complete_signature() {
    for source in [
        "fun bad(value: Int, : String) {}\nfun good() {}",
        "fun bad(value: Int other: String) {}\nfun good() {}",
        "fun bad(value: Int { }\nfun good() {}",
        "fun bad(): {}\nfun good() {}",
        "@Tag(,) fun bad() {}\nfun good() {}",
        "context(value: , good: Context) fun bad() {}\nfun good() {}",
        "fun bad(value: String = \"\\q\") {}\nfun good() {}",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
        assert_eq!(names(&parsed), ["good"], "{source}");
        assert!(parsed.declarations().iter().any(|declaration| {
            declaration
                .name
                .as_ref()
                .is_some_and(|name| name.text == "bad")
        }));
    }
}

#[test]
fn a_previous_missing_item_diagnostic_does_not_invalidate_the_next_header() {
    let parsed = parse_recovering("package\nfun good() {}").unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(names(&parsed), ["good"]);
}

#[test]
fn nested_headers_keep_their_grammar_owners_beside_errors() {
    let source = "class C { fun broken(value: ) { value. }\nfun good() { fun local() =\n } }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(names(&parsed), ["good", "local"]);
    let owners = headers(&parsed)
        .iter()
        .map(|header| {
            let declaration = &parsed.declarations()[header.declaration.0];
            let parent = &parsed.declarations()[declaration.parent.unwrap().0];
            parent.name.as_ref().unwrap().text.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(owners, ["C", "good"]);
}

#[test]
fn extension_header_preserves_binders_names_and_where_clause() {
    let source = "fun <T> List<T>.`首 个`(value: T): T where T: Any { value. }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(names(&parsed), ["首 个"]);
    let signature = &headers(&parsed)[0].value.signature;
    assert!(signature.receiver_ty.is_some());
    assert_eq!(signature.type_params[0].name.text, "T");
    assert!(signature.where_clause.is_some());
    assert_eq!(
        &source[signature.name.span.start as usize..signature.name.span.end as usize],
        "`首 个`"
    );
}

#[test]
fn valid_headers_are_the_same_data_used_by_the_complete_ast() {
    let source = "fun block() {}\nfun expression() = 1\nfun omitted()";
    let parsed = parse_recovering(source).unwrap();
    let ast = parsed.ast().unwrap();
    assert_eq!(names(&parsed), ["block", "expression", "omitted"]);
    for (declaration, header) in ast.declarations.iter().zip(headers(&parsed)) {
        let scoop_ast::Decl::Function(function) = declaration else {
            panic!("the input only contains functions");
        };
        assert_eq!(function.signature, header.value.signature);
        assert_eq!(function.body.form(), header.value.body_form);
    }
}

#[test]
fn interface_default_and_abstract_modifiers_use_the_real_body_form() {
    let source = "interface I { fun abstractMember(): Int\nfun defaultMember(): Int { missing. } }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    let headers = headers(&parsed);
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[0].value.body_form, FunctionBodyForm::Omitted);
    assert_eq!(
        headers[0].value.signature.modifier,
        MethodModifier::Abstract
    );
    assert_eq!(headers[1].value.body_form, FunctionBodyForm::Block);
    assert_eq!(headers[1].value.signature.modifier, MethodModifier::Open);
}
