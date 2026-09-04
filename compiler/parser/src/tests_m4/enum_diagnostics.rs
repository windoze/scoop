use super::*;

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
    assert_eq!(message, "`init` blocks are not allowed in enums");
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
fn enum_expression_default_is_preserved_for_hir() {
    let file = ok("enum E { V(val x: Int = f()) }");
    let Decl::Enum(enumeration) = &file.declarations[0] else {
        panic!("expected enum")
    };
    let VariantDeclKind::Constructor(fields) = &enumeration.variants[0].kind else {
        panic!("expected constructor variant")
    };
    assert!(matches!(
        fields[0].syntax,
        scoop_ast::ParameterSyntax::Default {
            expression: Expr::Call(_),
            ..
        }
    ));
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
