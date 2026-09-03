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
