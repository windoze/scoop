use scoop_parser::ParsedFile;
use scoop_syntax::SyntaxKind;

pub fn parse(source: &str) -> ParsedFile {
    let parsed = scoop_parser::parse_recovering(source).unwrap();
    assert!(
        parsed.ast().is_some(),
        "parse failed: {:?}\n{source}",
        parsed.diagnostics()
    );
    parsed
}

pub fn checked(source: &str) -> String {
    let original = parse(source);
    let formatted = scoop_fmt::format(&original.syntax());
    let after = parse(&formatted);
    assert_eq!(
        tokens(&original),
        tokens(&after),
        "changed tokens:\n{formatted}"
    );
    assert_eq!(
        normalized_ast(&original),
        normalized_ast(&after),
        "changed AST:\n{formatted}"
    );
    assert_eq!(
        documentation(&original),
        documentation(&after),
        "changed documentation attachment"
    );
    assert_eq!(
        formatted,
        scoop_fmt::format(&after.syntax()),
        "not idempotent"
    );
    formatted
}

fn tokens(parsed: &ParsedFile) -> Vec<(SyntaxKind, String)> {
    parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .filter(|token| token.kind() != SyntaxKind::Whitespace)
        .map(|token| (token.kind(), token.text().to_owned()))
        .collect()
}

fn documentation(parsed: &ParsedFile) -> Vec<(String, Option<usize>)> {
    let tokens: Vec<_> = parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect();
    parsed
        .ast()
        .unwrap()
        .documentation
        .iter()
        .map(|doc| {
            let target = match doc.target {
                scoop_ast::DocCommentTarget::Unattached => None,
                scoop_ast::DocCommentTarget::Declaration(span) => {
                    Some(tokens.partition_point(|token| {
                        u32::from(token.text_range().start()) < span.start
                    }))
                }
            };
            (doc.text.clone(), target)
        })
        .collect()
}

// Compare the complete AST, including syntax distinctions omitted by human dumps.
// Ignore positions only outside quoted strings, so literal contents remain exact.
fn normalized_ast(parsed: &ParsedFile) -> String {
    let debug = format!("{:?}", parsed.ast().unwrap());
    let mut output = String::new();
    let mut rest = debug.as_str();
    let mut quote = None;
    let mut escaped = false;
    while !rest.is_empty() {
        if quote.is_none() && rest.starts_with("Span { ") {
            let end = rest.find('}').unwrap();
            output.push_str("Span");
            rest = &rest[end + 1..];
            continue;
        }
        let character = rest.chars().next().unwrap();
        rest = &rest[character.len_utf8()..];
        output.push(character);
        if escaped {
            escaped = false;
        } else if quote.is_some() && character == '\\' {
            escaped = true;
        } else if Some(character) == quote {
            quote = None;
        } else if quote.is_none() && matches!(character, '\'' | '"') {
            quote = Some(character);
        }
    }
    output
}
