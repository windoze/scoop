use scoop_ast::{Decl, Expr, FunctionBody, Span};

use crate::{
    lexer::{TokenKind, lex},
    tests::{ok, only_function},
};

#[test]
fn unicode_identifiers_keep_utf8_spans_and_codepoints() {
    let source = "变量 e\u{301} é";
    let (tokens, errors) = lex(source);
    assert!(errors.is_empty());
    assert_eq!(tokens[0].kind, TokenKind::Ident("变量".into()));
    assert_eq!(tokens[0].span, Span::new(0, 6));
    assert_ne!(tokens[1].kind, tokens[2].kind);
    ok("fun <类型> 身份(输入: 类型): 类型 = 输入");
    ok("fun main() { val 名称 = 1; println(f\"${名称}\") }");
    let (_, errors) = lex("f\"$名称\"");
    assert_eq!(
        errors[0].message,
        "`$名称` interpolation requires braces; use `${名称}`"
    );
}

#[test]
fn escaped_keywords_are_names_in_each_syntactic_position() {
    let function = only_function(&ok("fun `if`(`return`: Int): Int = `return`")).clone();
    assert_eq!(function.name.text, "if");
    let FunctionBody::Expr(body) = function.body else {
        panic!("expected an expression body");
    };
    let Expr::Var(name) = *body else {
        panic!("expected a parameter reference");
    };
    assert_eq!(name.text, "return");
    ok("fun main() { `inline`(); value `in` other; `super`(); obj.`class`(`val` = 1) }");
    ok("fun <reified `class`> copy(noinline `crossinline`: `class`): `class` = `crossinline`");
    ok("class `_`\nfun copy(value: `_`): `_` = value");
}

#[test]
fn quoted_dots_are_retained_as_one_annotation_path_segment() {
    let file = ok(
        "package 中文.`包.名`\nimport 中文.`包.名`.`类型.名` as `别.名`\n\
                   @`标.记` fun first() {}\n@标.记 fun second() {}",
    );
    let names = file
        .declarations
        .iter()
        .map(|declaration| {
            let Decl::Function(function) = declaration else {
                panic!("expected function")
            };
            function.annotations[0]
                .name
                .segments()
                .map(|part| part.text.as_str())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, [vec!["标.记"], vec!["标", "记"]]);
}

#[test]
fn malformed_names_produce_lexical_diagnostics() {
    for (source, message) in [
        ("``", "escaped identifier must not be empty"),
        ("`未完成", "unterminated escaped identifier"),
        (
            "`a\nb`",
            "escaped identifier cannot contain a newline or control character",
        ),
        (
            "`a\0b`",
            "escaped identifier cannot contain a newline or control character",
        ),
        ("\u{301}bad", "unexpected character"),
    ] {
        let (_, errors) = lex(source);
        assert!(
            errors.iter().any(|error| error.message.contains(message)),
            "{source:?}: {errors:?}"
        );
    }
}
