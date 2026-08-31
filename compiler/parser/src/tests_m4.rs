//! Unit tests for the M4 syntax: enum declarations (four variant forms,
//! type parameters, constant default values), `@Intrinsic` annotations,
//! the statement-level `when` (patterns, guards, `..`, `else`), and
//! destructuring `val` / `var` declarations.

use scoop_ast::{
    Decl, Expr, FieldSelector, FunctionBody, Pattern, Span, StatementKind, TypeRefKind,
    VariantDeclKind, When,
};

use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::stmt_dump;

/// Parses `fun main() { when (s) { <arms> } }` and returns the `When`.
fn when_with_arms(arms: &str) -> When {
    let file = ok(&format!(
        "fun main() {{\n    when (s) {{\n{arms}    }}\n}}\n"
    ));
    let StatementKind::When(when) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a when statement");
    };
    when.clone()
}

/// Parses `fun main() { val <target> = init }`-style source and returns
/// the declared pattern.
fn val_target(target: &str) -> scoop_ast::ValDecl {
    let file = ok(&format!("fun main() {{\n    val {target} = init\n}}\n"));
    let StatementKind::ValDecl(decl) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a val declaration");
    };
    decl.clone()
}

// --- enum declarations ------------------------------------------------------

#[test]
fn enum_unit_variants() {
    let file = ok("enum Color { Red, Green, Blue }");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.name.text, "Color");
    assert_eq!(decl.span, Span::new(0, 31));
    assert!(decl.type_params.is_empty());
    assert_eq!(decl.variants.len(), 3);
    assert_eq!(decl.variants[0].name.text, "Red");
    assert_eq!(decl.variants[0].name.span, Span::new(13, 16));
    assert_eq!(decl.variants[0].span, Span::new(13, 16));
    assert!(
        decl.variants
            .iter()
            .all(|v| matches!(v.kind, VariantDeclKind::Unit))
    );
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  enum Color\n    Red\n    Green\n    Blue\n"
    );
}

#[test]
fn enum_variants_separated_by_newlines() {
    let file = ok("enum E {\n    A\n    B\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.variants.len(), 2);
}

#[test]
fn enum_trailing_comma_is_allowed() {
    let file = ok("enum E { A, B, }");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.variants.len(), 2);
}

#[test]
fn enum_positional_variants() {
    let file = ok("enum Shape {\n    Circle(Int),\n    Rect(Int, Int)\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    let VariantDeclKind::Positional(types) = &decl.variants[0].kind else {
        panic!("expected a positional variant");
    };
    assert_eq!(types.len(), 1);
    assert!(matches!(&types[0].kind, TypeRefKind::Named(name) if name.text == "Int"));
    let VariantDeclKind::Positional(types) = &decl.variants[1].kind else {
        panic!("expected a positional variant");
    };
    assert_eq!(types.len(), 2);
    assert_eq!(decl.variants[1].span, Span::new(34, 48));
}

#[test]
fn enum_positional_variant_type_forms() {
    let file = ok("enum E { V(Int?, (Int, String)) }");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    let VariantDeclKind::Positional(types) = &decl.variants[0].kind else {
        panic!("expected a positional variant");
    };
    assert!(matches!(&types[0].kind, TypeRefKind::Nullable(_)));
    assert!(matches!(&types[1].kind, TypeRefKind::Tuple(elements) if elements.len() == 2));
}

#[test]
fn enum_generic_type_params() {
    let file = ok("enum Option<T> {\n    Some(T),\n    None\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.type_params.len(), 1);
    assert_eq!(decl.type_params[0].text, "T");
    assert_eq!(decl.type_params[0].span, Span::new(12, 13));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  enum Option<T>\n    Some(T)\n    None\n"
    );
}

#[test]
fn enum_named_field_variant() {
    let file = ok("enum E {\n    Named { w: Int, h: Int }\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    let VariantDeclKind::Named(fields) = &decl.variants[0].kind else {
        panic!("expected a named-field variant");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name.text, "w");
    assert!(fields[0].default.is_none());
    assert!(matches!(&fields[1].ty.kind, TypeRefKind::Named(name) if name.text == "Int"));
}

#[test]
fn enum_named_fields_separated_by_newlines() {
    let file = ok("enum E {\n    Named {\n        w: Int\n        h: Int\n    }\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    let VariantDeclKind::Named(fields) = &decl.variants[0].kind else {
        panic!("expected a named-field variant");
    };
    assert_eq!(fields.len(), 2);
}

#[test]
fn enum_constructor_variant_with_constant_defaults() {
    let file = ok("enum E {\n    WithDefault(val d: Int = 0, val s: String = \"x\")\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    let VariantDeclKind::Constructor(fields) = &decl.variants[0].kind else {
        panic!("expected a constructor-style variant");
    };
    assert_eq!(fields.len(), 2);
    assert!(matches!(
        fields[0].default,
        Some(Expr::IntLiteral { value: 0, .. })
    ));
    assert!(matches!(&fields[1].default, Some(Expr::StringLiteral { value, .. }) if value == "x"));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  enum E\n    WithDefault <ctor>\n      d: Int = <expr>\n      s: String = <expr>\n"
    );
}

#[test]
fn enum_all_four_variant_forms_dump() {
    let file = ok(
        "enum Shape {\n    Circle(Int),\n    Rect(Int, Int),\n    Named { w: Int, h: Int },\n    WithDefault(val d: Int = 0)\n}\n",
    );
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  enum Shape\n    Circle(Int)\n    Rect(Int, Int)\n    Named <named>\n      w: Int\n      h: Int\n    WithDefault <ctor>\n      d: Int = <expr>\n"
    );
}

// --- enum diagnostics ---------------------------------------------------------

#[test]
fn enum_member_function() {
    // M4 rejected member functions; M6 adds them (after the variants).
    let file = ok("enum E {\n    A,\n    fun f() {}\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.variants.len(), 1);
    assert_eq!(decl.methods.len(), 1);
    assert_eq!(decl.methods[0].name.text, "f");
}

#[test]
fn enum_init_block_not_supported() {
    let (span, message) = err("enum E {\n    init\n}\n");
    assert_eq!(span, Span::new(13, 17));
    assert_eq!(
        message,
        "`init` blocks are not supported yet (milestone M6)"
    );
}

#[test]
fn enum_var_field_not_supported() {
    let (span, message) = err("enum E { V(var x: Int) }");
    assert_eq!(span, Span::new(11, 14));
    assert_eq!(
        message,
        "`var` variant fields are not supported (value types are immutable)"
    );
}

#[test]
fn enum_non_constant_default_is_an_error() {
    let (span, message) = err("enum E { V(val x: Int = f()) }");
    assert_eq!(span, Span::new(24, 27));
    assert_eq!(
        message,
        "only constant expressions are allowed as variant field defaults (milestone M4)"
    );
}

#[test]
fn enum_block_style_default_is_an_error() {
    let (span, message) = err("enum E { V { x: Int = 0 } }");
    assert_eq!(span, Span::new(20, 21));
    assert_eq!(
        message,
        "variant field defaults require the constructor-style form (milestone M4)"
    );
}

#[test]
fn enum_variants_need_a_separator() {
    let (span, message) = err("enum E { A B }");
    assert_eq!(span, Span::new(11, 12));
    assert_eq!(message, "expected `,` or newline after variant, found `B`");
}

#[test]
fn enum_semicolons_do_not_separate_variants() {
    let (_, message) = err("enum E { A; B }");
    assert_eq!(message, "expected `,` or newline after variant, found `;`");
}

#[test]
fn enum_empty_type_parameter_list_is_an_error() {
    let (span, message) = err("enum E<> {}");
    assert_eq!(span, Span::new(7, 8));
    assert_eq!(message, "expected type parameter name, found `>`");
}

#[test]
fn enum_empty_named_field_block_is_an_error() {
    let (span, message) = err("enum E { V {} }");
    assert_eq!(span, Span::new(12, 13));
    assert_eq!(message, "expected field declaration, found `}`");
}

#[test]
fn enum_empty_positional_variant_is_an_error() {
    let (span, message) = err("enum E { V() }");
    assert_eq!(span, Span::new(11, 12));
    assert_eq!(message, "expected type, found `)`");
}

// --- annotations --------------------------------------------------------------

#[test]
fn intrinsic_annotation_on_bodiless_function() {
    let file = ok("@Intrinsic(\"rt_print\")\nfun print(message: String)\n");
    let function = only_function(&file);
    assert_eq!(function.annotations.len(), 1);
    let annotation = &function.annotations[0];
    assert_eq!(annotation.name.text, "Intrinsic");
    assert!(matches!(
        annotation.args.as_slice(),
        [scoop_ast::AnnotationArg {
            value: scoop_ast::AnnotationLiteral::String(value),
            ..
        }] if value == "rt_print"
    ));
    assert_eq!(annotation.span, Span::new(0, 22));
    assert_eq!(function.name.text, "print");
    assert_eq!(function.span, Span::new(0, 49));
    // A bodiless `@Intrinsic` function (spec 13.1): `FunctionBody::None`
    // since M6; `annotations` is what HIR keys on.
    assert!(matches!(function.body, FunctionBody::None));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n    @Intrinsic(\"rt_print\")\n  fun print(message: String)\n"
    );
}

#[test]
fn intrinsic_function_followed_by_another_declaration() {
    let file = ok("@Intrinsic(\"rt_print\")\nfun print(message: String)\n\nfun main() {}\n");
    assert_eq!(file.declarations.len(), 2);
    assert!(matches!(file.declarations[0], Decl::Function(_)));
    assert!(matches!(file.declarations[1], Decl::Function(_)));
}

#[test]
fn intrinsic_function_with_return_type() {
    let file = ok("@Intrinsic(\"int_add\")\nfun add(lhs: Int, rhs: Int): Int\n");
    let function = only_function(&file);
    assert!(function.return_ty.is_some());
    assert_eq!(function.params.len(), 2);
}

#[test]
fn unknown_annotation_is_preserved_for_hir() {
    let file = ok("@Unknown fun f() {}");
    assert_eq!(only_function(&file).annotations[0].name.text, "Unknown");
}

#[test]
fn marker_annotation_has_no_arguments() {
    let file = ok("@Intrinsic fun f() {}");
    assert!(only_function(&file).annotations[0].args.is_empty());
}

#[test]
fn empty_annotation_argument_list_is_preserved() {
    let file = ok("@Intrinsic() fun f() {}");
    assert!(only_function(&file).annotations[0].args.is_empty());
}

#[test]
fn annotation_integer_argument_is_preserved() {
    let file = ok("@Intrinsic(42) fun f() {}");
    assert!(matches!(
        only_function(&file).annotations[0].args[0].value,
        scoop_ast::AnnotationLiteral::Int(42)
    ));
}

#[test]
fn multiple_annotation_arguments_are_preserved() {
    let file = ok("@Intrinsic(\"a\", \"b\") fun f() {}");
    assert_eq!(only_function(&file).annotations[0].args.len(), 2);
}

#[test]
fn multiple_annotations_are_preserved() {
    let file = ok("@Intrinsic(\"a\")\n@Intrinsic(\"b\")\nfun f() {}\n");
    assert_eq!(only_function(&file).annotations.len(), 2);
}

#[test]
fn annotation_does_not_suppress_a_block_body() {
    let file = ok("@Intrinsic(\"x\") fun f() {}");
    assert!(matches!(only_function(&file).body, FunctionBody::Block(_)));
}

#[test]
fn annotation_does_not_suppress_an_expression_body() {
    let file = ok("@Intrinsic(\"x\") fun f() = 1");
    assert!(matches!(only_function(&file).body, FunctionBody::Expr(_)));
}

#[test]
fn annotation_on_a_struct_is_preserved_for_hir() {
    let file = ok("@Intrinsic(\"x\") struct S(val x: Int)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected struct");
    };
    assert_eq!(decl.annotations[0].name.text, "Intrinsic");
}

// --- when statements ----------------------------------------------------------

#[test]
fn when_with_bare_and_prefixed_unit_variants_and_else() {
    let when = when_with_arms(
        "        Red -> { print(\"r\") }\n        Color.Green -> { }\n        else -> { println(\"o\") }\n",
    );
    assert_eq!(when.arms.len(), 2);
    // A bare identifier is always a binding ("binding first"); HIR
    // resolves it to the unit variant.
    assert!(matches!(&when.arms[0].pattern, Pattern::Binding(name) if name.text == "Red"));
    // `Color.Green` is a unit variant pattern with a type prefix.
    let Pattern::Positional {
        path,
        elements,
        rest,
        ..
    } = &when.arms[1].pattern
    else {
        panic!("expected a positional pattern");
    };
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].text, "Color");
    assert_eq!(path[1].text, "Green");
    assert!(elements.is_empty());
    assert!(rest.is_none());
    assert!(when.else_body.is_some());
}

#[test]
fn when_statement_dump() {
    assert_eq!(
        stmt_dump(
            "when (c) {\n        Red -> { print(\"r\") }\n        else -> { println(\"o\") }\n    }"
        ),
        "when\n  Var c\n  arm Red\n    Call print\n      StringLiteral \"r\"\n  else\n    Call println\n      StringLiteral \"o\"\n"
    );
}

#[test]
fn when_arm_with_guard() {
    let when = when_with_arms("        Some(x) if (x > 0) -> { println(x) }\n");
    assert_eq!(when.arms.len(), 1);
    let arm = &when.arms[0];
    let Pattern::Positional { path, elements, .. } = &arm.pattern else {
        panic!("expected a positional pattern");
    };
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].text, "Some");
    assert!(matches!(&elements[0], Pattern::Binding(name) if name.text == "x"));
    let guard = arm.guard.as_ref().expect("guard present");
    assert!(matches!(guard, Expr::Binary { .. }));
}

#[test]
fn when_guard_dump() {
    assert_eq!(
        stmt_dump(
            "when (s) {\n        Some(x) if (x > 0) -> { println(x) }\n        None -> { }\n    }"
        ),
        "when\n  Var s\n  arm Some(x) if <guard>\n    Call println\n      Var x\n  arm None\n"
    );
}

#[test]
fn when_positional_pattern_with_literal_and_wildcard() {
    let when = when_with_arms("        Rect(0, _) -> { }\n");
    let Pattern::Positional { elements, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a positional pattern");
    };
    assert_eq!(elements.len(), 2);
    assert!(matches!(&elements[0], Pattern::Literal { .. }));
    assert!(matches!(&elements[1], Pattern::Wildcard { .. }));
    assert!(rest.is_none());
}

#[test]
fn when_prefixed_variant_with_payload() {
    let when = when_with_arms("        Option.Some(x) -> { }\n");
    let Pattern::Positional { path, elements, .. } = &when.arms[0].pattern else {
        panic!("expected a positional pattern");
    };
    let names: Vec<&str> = path.iter().map(|ident| ident.text.as_str()).collect();
    assert_eq!(names, ["Option", "Some"]);
    assert_eq!(elements.len(), 1);
}

#[test]
fn when_named_field_pattern_with_rename_and_rest() {
    let when = when_with_arms("        Named { w, h: height, .. } -> { }\n");
    let Pattern::Named {
        path, fields, rest, ..
    } = &when.arms[0].pattern
    else {
        panic!("expected a named pattern");
    };
    assert_eq!(path.len(), 1);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name.text, "w");
    assert!(fields[0].rename.is_none());
    assert_eq!(fields[1].name.text, "h");
    assert_eq!(
        fields[1].rename.as_ref().map(|ident| ident.text.as_str()),
        Some("height")
    );
    assert!(rest.is_some());
}

#[test]
fn when_named_field_pattern_without_rest() {
    let when = when_with_arms("        Point { x, y } -> { }\n");
    let Pattern::Named { fields, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a named pattern");
    };
    assert_eq!(fields.len(), 2);
    assert!(rest.is_none());
}

#[test]
fn when_tuple_pattern() {
    let when = when_with_arms("        (x, 0, _) -> { }\n");
    let Pattern::Tuple { elements, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a tuple pattern");
    };
    assert_eq!(elements.len(), 3);
    assert!(matches!(&elements[0], Pattern::Binding(name) if name.text == "x"));
    assert!(matches!(&elements[1], Pattern::Literal { .. }));
    assert!(matches!(&elements[2], Pattern::Wildcard { .. }));
    assert!(rest.is_none());
}

#[test]
fn when_tuple_pattern_with_rest_in_the_middle() {
    let when = when_with_arms("        (x, .., y) -> { }\n");
    let Pattern::Tuple { elements, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a tuple pattern");
    };
    assert_eq!(elements.len(), 2);
    assert!(rest.is_some());
}

#[test]
fn when_tuple_pattern_with_only_rest() {
    let when = when_with_arms("        (..) -> { }\n");
    let Pattern::Tuple { elements, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a tuple pattern");
    };
    assert!(elements.is_empty());
    assert!(rest.is_some());
}

#[test]
fn when_parenthesized_pattern_is_not_a_tuple() {
    // `(x)` mirrors the expression disambiguation: just `x`.
    let when = when_with_arms("        (x) -> { }\n");
    assert!(matches!(&when.arms[0].pattern, Pattern::Binding(name) if name.text == "x"));
}

#[test]
fn when_unit_literal_patterns() {
    let when = when_with_arms("        () -> { }\n");
    let Pattern::Literal { expr, .. } = &when.arms[0].pattern else {
        panic!("expected a literal pattern");
    };
    assert!(matches!(*expr.clone(), Expr::UnitLiteral { .. }));
    let when = when_with_arms("        Unit -> { }\n");
    let Pattern::Literal { expr, .. } = &when.arms[0].pattern else {
        panic!("expected a literal pattern");
    };
    assert!(matches!(*expr.clone(), Expr::UnitLiteral { .. }));
}

#[test]
fn when_literal_patterns() {
    for (source, expect) in [
        ("0", "int"),
        ("\"x\"", "string"),
        ("true", "bool"),
        ("false", "bool"),
    ] {
        let when = when_with_arms(&format!("        {source} -> {{ }}\n"));
        let Pattern::Literal { expr, .. } = &when.arms[0].pattern else {
            panic!("expected a literal pattern");
        };
        match expect {
            "int" => assert!(matches!(*expr.clone(), Expr::IntLiteral { .. })),
            "string" => assert!(matches!(*expr.clone(), Expr::StringLiteral { .. })),
            _ => assert!(matches!(*expr.clone(), Expr::BoolLiteral { .. })),
        }
    }
}

#[test]
fn when_nested_patterns() {
    let when = when_with_arms("        Some((a, b)) -> { }\n");
    let Pattern::Positional { elements, .. } = &when.arms[0].pattern else {
        panic!("expected a positional pattern");
    };
    let Pattern::Tuple {
        elements: inner, ..
    } = &elements[0]
    else {
        panic!("expected a nested tuple pattern");
    };
    assert_eq!(inner.len(), 2);

    let when = when_with_arms("        Some(Some(x)) -> { }\n");
    let Pattern::Positional { elements, .. } = &when.arms[0].pattern else {
        panic!("expected a positional pattern");
    };
    let Pattern::Positional { path, .. } = &elements[0] else {
        panic!("expected a nested positional pattern");
    };
    assert_eq!(path[0].text, "Some");
}

#[test]
fn when_statement_spans() {
    let file = ok("fun main() {\n    when (s) {\n        Red -> { }\n    }\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    assert_eq!(stmt.span, Span::new(17, 52));
    let StatementKind::When(when) = &stmt.kind else {
        panic!("expected a when statement");
    };
    assert_eq!(when.arms.len(), 1);
    assert_eq!(when.arms[0].span, Span::new(36, 46));
    assert!(when.else_body.is_none());
}

// --- when diagnostics -----------------------------------------------------------

#[test]
fn when_rest_twice_in_positional_pattern() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        V(a, .., ..) -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(45, 47));
    assert_eq!(message, "`..` may appear at most once in a pattern");
}

#[test]
fn when_rest_twice_in_tuple_pattern() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        (a, .., b, ..) -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(47, 49));
    assert_eq!(message, "`..` may appear at most once in a pattern");
}

#[test]
fn when_rest_must_be_last_in_field_pattern() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        S { .., x } -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(44, 45));
    assert_eq!(message, "`..` must be the last element in a field pattern");
}

#[test]
fn when_rest_twice_in_field_pattern() {
    let (_, message) = err("fun main() {\n    when (s) {\n        S { .., .. } -> { }\n    }\n}\n");
    assert_eq!(message, "`..` must be the last element in a field pattern");
}

#[test]
fn when_wildcard_is_not_a_field_name() {
    let (span, message) = err("fun main() {\n    when (s) {\n        S { _ } -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(40, 41));
    assert_eq!(message, "`_` is not allowed in a field pattern");
}

#[test]
fn when_wildcard_is_not_a_field_rename() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        S { x: _ } -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(43, 44));
    assert_eq!(message, "`_` is not allowed in a field pattern");
}

#[test]
fn when_arm_needs_an_arrow() {
    let (span, message) = err("fun main() {\n    when (s) {\n        Red\n    }\n}\n");
    assert_eq!(span, Span::new(44, 45));
    assert_eq!(message, "expected `->`, found `}`");
}

#[test]
fn when_else_must_be_the_last_arm() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        else -> { }\n        Red -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(56, 59));
    assert_eq!(message, "expected `}`, found `Red`");
}

#[test]
fn when_guard_must_be_parenthesized() {
    let (span, message) = err("fun main() {\n    when (s) {\n        Red if x -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(43, 44));
    assert_eq!(message, "expected `(`, found `x`");
}

#[test]
fn when_bare_rest_is_not_a_pattern() {
    let (span, message) = err("fun main() {\n    when (s) {\n        .. -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(36, 38));
    assert_eq!(message, "expected pattern, found `..`");
}

#[test]
fn when_range_is_not_a_pattern() {
    // The `..` in the pattern position is the rest marker; a range
    // expression therefore cannot appear there (spec 4.6 disambiguation).
    let (span, message) = err("fun main() {\n    when (s) {\n        1..4 -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(37, 39));
    assert_eq!(message, "expected `->`, found `..`");
}

// --- destructuring declarations ----------------------------------------------

#[test]
fn val_tuple_destructuring() {
    let decl = val_target("(a, b)");
    assert!(!decl.mutable);
    let Pattern::Tuple { elements, rest, .. } = &decl.target else {
        panic!("expected a tuple pattern");
    };
    assert_eq!(elements.len(), 2);
    assert!(matches!(&elements[0], Pattern::Binding(name) if name.text == "a"));
    assert!(rest.is_none());
}

#[test]
fn val_tuple_destructuring_with_wildcard_and_rest() {
    let decl = val_target("(a, _, ..)");
    let Pattern::Tuple { elements, rest, .. } = &decl.target else {
        panic!("expected a tuple pattern");
    };
    assert_eq!(elements.len(), 2);
    assert!(matches!(&elements[1], Pattern::Wildcard { .. }));
    assert!(rest.is_some());
}

#[test]
fn val_single_element_tuple_needs_a_trailing_comma() {
    let decl = val_target("(a,)");
    assert!(matches!(&decl.target, Pattern::Tuple { elements, .. } if elements.len() == 1));
    // `(a)` is just `a` in parentheses.
    let decl = val_target("(a)");
    assert!(matches!(&decl.target, Pattern::Binding(name) if name.text == "a"));
}

#[test]
fn val_struct_positional_destructuring() {
    let decl = val_target("Point(x, _)");
    let Pattern::Positional { path, elements, .. } = &decl.target else {
        panic!("expected a positional pattern");
    };
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].text, "Point");
    assert_eq!(elements.len(), 2);
}

#[test]
fn val_struct_field_destructuring() {
    let decl = val_target("Point { x, y: yy, .. }");
    let Pattern::Named {
        fields, rest, path, ..
    } = &decl.target
    else {
        panic!("expected a named pattern");
    };
    assert_eq!(path.len(), 1);
    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields[1].rename.as_ref().map(|ident| ident.text.as_str()),
        Some("yy")
    );
    assert!(rest.is_some());
}

#[test]
fn val_nested_destructuring() {
    let decl = val_target("(a, (b, c))");
    let Pattern::Tuple { elements, .. } = &decl.target else {
        panic!("expected a tuple pattern");
    };
    assert!(matches!(&elements[1], Pattern::Tuple { elements, .. } if elements.len() == 2));
}

#[test]
fn val_destructuring_with_type_annotation() {
    let file = ok("fun main() {\n    val (a, b): (Int, Int) = t\n}\n");
    let StatementKind::ValDecl(decl) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a val declaration");
    };
    assert!(matches!(&decl.target, Pattern::Tuple { .. }));
    let ty = decl.ty.as_ref().expect("type annotation present");
    assert!(matches!(&ty.kind, TypeRefKind::Tuple(elements) if elements.len() == 2));
}

#[test]
fn var_destructuring_behaves_like_val() {
    let file = ok("fun main() {\n    var (a, b) = t\n}\n");
    let StatementKind::ValDecl(decl) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a var declaration");
    };
    assert!(decl.mutable);
    assert!(matches!(&decl.target, Pattern::Tuple { .. }));
}

#[test]
fn val_wildcard_target() {
    let decl = val_target("_");
    assert!(matches!(&decl.target, Pattern::Wildcard { .. }));
}

#[test]
fn val_destructuring_dump() {
    assert_eq!(
        stmt_dump("val Point(x, _) = p"),
        "val Point(x, _)\n  Var p\n"
    );
    assert_eq!(stmt_dump("val (a, b) = t"), "val (a, b)\n  Var t\n");
}

#[test]
fn val_destructuring_spans() {
    let file = ok("fun main() {\n    val (a, b) = t\n}\n");
    let StatementKind::ValDecl(decl) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a val declaration");
    };
    assert_eq!(decl.span, Span::new(17, 31));
    let Pattern::Tuple { span, .. } = &decl.target else {
        panic!("expected a tuple pattern");
    };
    assert_eq!(*span, Span::new(21, 27));
}

#[test]
fn val_destructuring_rest_twice_is_an_error() {
    let (span, message) = err("fun main() {\n    val (a, .., ..) = t\n}\n");
    assert_eq!(span, Span::new(29, 31));
    assert_eq!(message, "`..` may appear at most once in a pattern");
}

// --- single-expression arm bodies (spec 5.1) -----------------------------------

#[test]
fn when_arm_with_single_expression_body() {
    let when = when_with_arms("        Red -> println(\"red\")\n");
    assert_eq!(when.arms.len(), 1);
    let body = &when.arms[0].body;
    assert_eq!(body.statements.len(), 1);
    assert!(matches!(
        &body.statements[0].kind,
        StatementKind::Expr(Expr::Call(call)) if call.callee.text == "println"
    ));
    assert_eq!(body.span, body.statements[0].span);
}

#[test]
fn when_else_with_single_expression_body() {
    let when =
        when_with_arms("        Red -> println(\"r\")\n        else -> println(\"other\")\n");
    let else_body = when.else_body.as_ref().expect("else present");
    assert_eq!(else_body.statements.len(), 1);
    assert!(matches!(
        &else_body.statements[0].kind,
        StatementKind::Expr(Expr::Call(_))
    ));
}

#[test]
fn when_single_expression_arms_separate_like_statements() {
    let when = when_with_arms(
        "        Red -> println(\"r\")\n        Green -> println(\"g\")\n        Blue -> { println(\"b\") }\n",
    );
    assert_eq!(when.arms.len(), 3);
}

#[test]
fn when_guard_with_single_expression_body() {
    // The fixture shape from when-guards.scoop: guard, then a call body.
    let when = when_with_arms("        Num(n) if (n > 100) -> println(\"big\")\n");
    let arm = &when.arms[0];
    assert!(arm.guard.is_some());
    assert_eq!(arm.body.statements.len(), 1);
}

#[test]
fn when_single_expression_body_on_one_line_with_closing_brace() {
    let file = ok("fun main() { when (s) { Red -> println(\"r\") } }\n");
    let StatementKind::When(when) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a when statement");
    };
    assert_eq!(when.arms.len(), 1);
}

#[test]
fn when_single_expression_arm_body_needs_a_separator() {
    let (_, message) = err(
        "fun main() {\n    when (s) {\n        Red -> println(\"r\") Green -> println(\"g\")\n    }\n}\n",
    );
    assert_eq!(
        message,
        "expected `;` or newline after statement, found `Green`"
    );
}

#[test]
fn when_arm_body_is_not_a_declaration() {
    // Only expression statements are allowed without braces.
    let (_, message) = err("fun main() {\n    when (s) {\n        Red -> val x = 1\n    }\n}\n");
    assert_eq!(message, "expected expression, found `val`");
}

// --- qualified variant construction `E.V(args)` ------------------------------

#[test]
fn qualified_variant_construction_is_a_method_call() {
    // M4 collapsed `Shape.Circle(5)` into a dotted `Call`; since M6 the
    // same syntax parses as a method call (`expr.name(args)`), and
    // hir-lower resolves enum variant construction from that shape.
    let Expr::MethodCall {
        receiver,
        name,
        args,
        span,
    } = crate::tests_m2::init_expr("Shape.Circle(5)")
    else {
        panic!("expected a method call");
    };
    assert!(matches!(&*receiver, Expr::Var(head) if head.text == "Shape"));
    assert_eq!(name.text, "Circle");
    assert_eq!(name.span, Span::new(31, 37));
    assert_eq!(args.len(), 1);
    assert_eq!(span, Span::new(25, 40));
}

#[test]
fn qualified_variant_construction_without_arguments() {
    let Expr::MethodCall {
        receiver,
        name,
        args,
        ..
    } = crate::tests_m2::init_expr("Shape.WithDefault()")
    else {
        panic!("expected a method call");
    };
    assert!(matches!(&*receiver, Expr::Var(head) if head.text == "Shape"));
    assert_eq!(name.text, "WithDefault");
    assert!(args.is_empty());
}

#[test]
fn qualified_unit_variant_stays_a_field_access() {
    // Without a call, `Color.Red` is a FieldAccess; hir-lower resolves
    // the enum receiver to a unit variant construction.
    let Expr::FieldAccess(access) = crate::tests_m2::init_expr("Color.Red") else {
        panic!("expected a field access");
    };
    assert!(matches!(&*access.receiver, Expr::Var(name) if name.text == "Color"));
    let FieldSelector::Name(selector) = &access.selector else {
        panic!("expected a named selector");
    };
    assert_eq!(selector.text, "Red");
}

#[test]
fn field_access_without_a_call_is_unchanged() {
    // `p.x + 1` — the dot is not followed by `(`, so nothing collapses.
    let Expr::Binary { lhs, .. } = crate::tests_m2::init_expr("p.x + 1") else {
        panic!("expected a binary expression");
    };
    assert!(matches!(&*lhs, Expr::FieldAccess(_)));
}
