use crate::parse;
use scoop_ast::{DocCommentTarget, Span};

#[test]
fn shebang_keeps_source_positions_and_documentation() {
    for newline in ["\n", "\r\n"] {
        let source = format!(
            "#!/path/解释器 scoop run{newline}//! ordinary{newline}/// entry{newline}fun main() {{}}"
        );
        let file = parse(&source).unwrap();
        assert_eq!(file.declarations.len(), 1);
        assert_eq!(file.documentation.len(), 1);
        let declaration = file.declarations[0].span();
        assert_eq!(declaration.start as usize, source.find("fun main").unwrap());
        assert_eq!(
            file.documentation[0].target,
            DocCommentTarget::Declaration(declaration)
        );
        let span = file.documentation[0].span;
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            format!("/// entry{}", if newline == "\r\n" { "\r" } else { "" })
        );
    }
}

#[test]
fn shebang_only_file_is_empty_even_without_final_newline() {
    for source in ["#!", "#!/path/to/scoop run", "#! /// not documentation\r\n"] {
        let file = parse(source).unwrap();
        assert!(file.declarations.is_empty());
        assert!(file.documentation.is_empty());
    }
}

#[test]
fn shebang_requires_the_first_byte_of_the_file() {
    for prefix in [" ", "\n", "\u{feff}", "// comment\n"] {
        let source = format!("{prefix}#!scoop run\nfun main() {{}}");
        let diagnostics = parse(&source).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.span == Some(Span::new(prefix.len() as u32, prefix.len() as u32 + 1))
        }));
    }
}

#[test]
fn diagnostic_after_shebang_uses_original_byte_span() {
    let source = "#!scoop run\r\nfun main() { val = 1 }";
    let diagnostics = parse(source).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    let offset = source.find('=').unwrap() as u32;
    assert_eq!(diagnostics[0].span, Some(Span::new(offset, offset + 1)));
}
