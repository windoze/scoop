use scoop_ast::{Span, StatementKind, TypeRefKind};
use scoop_parser::{parse_function_body, parse_type};

#[test]
fn body_keeps_fragment_offsets_and_ordinary_statements() {
    let source = "// 中文\r\n{\n    val 文本 = \"hello\"\n    return 文本\n}\n/* end */";
    let body = parse_function_body(source).unwrap();
    let start = source.find('{').unwrap();
    let end = source.rfind('}').unwrap() + 1;
    assert_eq!(body.span, Span::new(start as u32, end as u32));
    assert_eq!(body.statements.len(), 2);
    let StatementKind::ValDecl(declaration) = &body.statements[0].kind else {
        panic!("expected a local value");
    };
    let scoop_ast::Pattern::Binding(name) = &declaration.target else {
        panic!("expected a named binding");
    };
    assert_eq!(name.text, "文本");
    let span = name.span;
    assert_eq!(&source[span.start as usize..span.end as usize], "文本");
}

#[test]
fn body_requires_braces_and_consumes_all_input() {
    for source in ["", "return 1", "= 1", "{} {}", "{} fun extra() {}", "{};"] {
        assert!(parse_function_body(source).is_err(), "{source:?}");
    }
    let errors = parse_function_body("{}\nfun extra() {}").unwrap_err();
    assert_eq!(errors[0].span, Some(Span::new(3, 6)));
    assert!(errors[0].message.contains("end of function body"));
    assert!(parse_function_body("/* start */ {} // end").is_ok());
}

#[test]
fn body_retains_independent_errors_after_recovery() {
    let source = "{\n    val = 1\n    val = 2\n}";
    let errors = parse_function_body(source).unwrap_err();
    assert_eq!(errors.len(), 2);
    for (diagnostic, (offset, _)) in errors.iter().zip(source.match_indices('=')) {
        assert_eq!(
            diagnostic.span,
            Some(Span::new(offset as u32, offset as u32 + 1))
        );
        assert!(diagnostic.message.contains("pattern"));
    }
}

#[test]
fn fragment_lexical_errors_are_never_accepted() {
    for source in ["{ val x = \"unfinished", "{} /* unfinished", "{}\0"] {
        let errors = parse_function_body(source).unwrap_err();
        assert!(!errors.is_empty());
    }
    for source in ["Int /* unfinished", "Int\0", "`unfinished"] {
        assert!(!parse_type(source).unwrap_err().is_empty());
    }
}

#[test]
fn type_supports_ordinary_combinations_and_original_spelling() {
    for source in [
        "Unit",
        "()",
        "(Int,)",
        "(Int, String?)",
        "List<Map<Int, String>>?",
        "Outer.Inner<Int>",
        "Box<Int>.Companion",
        "(Int) -> String",
        "suspend User.(Int, String?) -> Result<Bool>",
        "@Tag List<@Tag Int>",
    ] {
        let ty = parse_type(source).unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
        assert_eq!(ty.span, Span::new(0, source.len() as u32));
    }
    let source = "/* 文档 */ `中文类型`? // end";
    let ty = parse_type(source).unwrap();
    let TypeRefKind::Nullable(inner) = ty.kind else {
        panic!("expected a nullable type");
    };
    let TypeRefKind::Named(name) = inner.kind else {
        panic!("expected a named type");
    };
    assert_eq!(name.text, "中文类型");
    assert_eq!(
        &source[name.span.start as usize..name.span.end as usize],
        "`中文类型`"
    );
}

#[test]
fn type_requires_a_single_complete_type() {
    for source in [
        "",
        "Int String",
        "Int;",
        "List<>",
        "List<Int",
        "(Int) ->",
        "_",
        "suspend Int",
    ] {
        assert!(parse_type(source).is_err(), "{source:?}");
    }
    let errors = parse_type("Int\nclass Extra {}").unwrap_err();
    assert_eq!(errors[0].span, Some(Span::new(4, 9)));
    assert!(errors[0].message.contains("end of type"));
}

#[test]
fn body_uses_normal_lambda_interpolation_and_control_grammar() {
    let source = "{\n    val items = values.map { x -> f\"value ${x}\" }\n    if (ready) return items\n    return empty()\n}";
    let body = parse_function_body(source).unwrap();
    assert_eq!(body.statements.len(), 3);
    let errors = parse_function_body("{ return 1").unwrap_err();
    let eof = "{ return 1".len() as u32;
    assert_eq!(errors[0].span, Some(Span::new(eof, eof)));
}
