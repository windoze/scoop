use super::*;

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
    assert_eq!(decl.type_params[0].name.text, "T");
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
    assert!(matches!(fields[0].syntax, ParameterSyntax::Required));
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
        fields[0].syntax,
        ParameterSyntax::Default {
            expression: Expr::IntLiteral { value: 0, .. },
            ..
        }
    ));
    assert!(
        matches!(&fields[1].syntax, ParameterSyntax::Default { expression: Expr::StringLiteral { value, .. }, .. } if value == "x")
    );
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
