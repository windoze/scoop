use scoop_parser::parse_recovering;
use scoop_syntax::SyntaxKind;

#[test]
fn declaration_names_follow_the_grammar_including_extension_receivers() {
    let source = "fun <T> List<T>.`first value`(value: T): T = value\nval String.count: Int get() = 0\ntypealias Text = String";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_some(), "{:?}", parsed.diagnostics());
    let declarations = parsed.declarations();
    let names: Vec<_> = declarations
        .iter()
        .filter_map(|declaration| declaration.name.as_ref())
        .map(|name| name.text.as_str())
        .collect();
    assert_eq!(names, ["first value", "T", "value", "count", "Text"]);
    let function = &declarations[0];
    let name = function.name.as_ref().unwrap();
    assert_eq!(
        &source[name.span.start as usize..name.span.end as usize],
        "`first value`"
    );
    assert!(function.parent.is_none());
    assert_eq!(declarations[1].parent.unwrap().0, 0);
    assert_eq!(declarations[2].parent.unwrap().0, 0);
}

#[test]
fn incomplete_declarations_keep_names_without_borrowing_another_name() {
    let source = "class C { fun broken(value: ) { value. }\nfun good() = 1\nfun () {} }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    let declarations = parsed.declarations();
    let names: Vec<_> = declarations
        .iter()
        .map(|declaration| declaration.name.as_ref().map(|name| name.text.as_str()))
        .collect();
    assert_eq!(
        names,
        [Some("C"), Some("broken"), Some("value"), Some("good"), None]
    );
    for (index, declaration) in declarations.iter().enumerate() {
        if let Some(parent) = declaration.parent {
            assert!(parent.0 < index);
            let outer = &declarations[parent.0];
            assert!(outer.span.start <= declaration.span.start);
            assert!(declaration.span.end <= outer.span.end);
        }
        if let Some(name) = &declaration.name {
            assert!(declaration.span.start <= name.span.start);
            assert!(name.span.end <= declaration.span.end);
        }
    }
}

#[test]
fn nominal_fields_variants_and_companions_have_actual_owners() {
    let source = "struct S(val field: Int) { companion object {} }\nenum E { C(val item: Int), N {field: String} }\nclass K { constructor(value: Int) {} }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_some(), "{:?}", parsed.diagnostics());
    let declarations = parsed.declarations();
    let owners: Vec<_> = declarations
        .iter()
        .filter_map(|declaration| {
            let parent = declaration.parent?;
            Some((
                declaration.name.as_ref()?.text.as_str(),
                declarations[parent.0].name.as_ref()?.text.as_str(),
            ))
        })
        .collect();
    assert_eq!(
        owners,
        [
            ("field", "S"),
            ("Companion", "S"),
            ("C", "E"),
            ("item", "C"),
            ("N", "E"),
            ("field", "N"),
            ("constructor", "K"),
            ("value", "constructor"),
        ]
    );
    assert_eq!(
        declarations
            .iter()
            .filter(|declaration| declaration.kind == SyntaxKind::Variant)
            .count(),
        2
    );
}
