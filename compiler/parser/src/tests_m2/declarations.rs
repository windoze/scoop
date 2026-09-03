use super::*;

// --- declarations ---------------------------------------------------------

#[test]
fn struct_decl() {
    let file = ok("struct Point(val x: Int, val y: Int)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.name.text, "Point");
    assert_eq!(decl.name.span, Span::new(7, 12));
    assert_eq!(decl.span, Span::new(0, 36));
    assert_eq!(decl.fields.len(), 2);
    assert_eq!(decl.fields[0].name.text, "x");
    assert_eq!(decl.fields[0].span, Span::new(13, 23));
    assert_eq!(decl.fields[1].name.text, "y");
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  struct Point\n    field x: Int\n    field y: Int\n"
    );
}

#[test]
fn struct_field_type_forms() {
    let file =
        ok("struct S(val a: (Int, String), val b: (Int,), val c: (), val d: Unit, val e: (Int))");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    let kinds: Vec<&TypeRefKind> = decl.fields.iter().map(|f| &f.ty.kind).collect();
    assert!(
        matches!(kinds[0], TypeRefKind::Tuple(elements) if elements.len() == 2),
        "tuple type"
    );
    assert!(
        matches!(kinds[1], TypeRefKind::Tuple(elements) if elements.len() == 1),
        "1-tuple type needs a trailing comma"
    );
    assert!(
        matches!(kinds[2], TypeRefKind::Unit),
        "`()` is the Unit type"
    );
    assert!(
        matches!(kinds[3], TypeRefKind::Unit),
        "`Unit` is the Unit type"
    );
    assert!(
        matches!(kinds[4], TypeRefKind::Named(name) if name.text == "Int"),
        "`(T)` is just `T` in parentheses"
    );
}

#[test]
fn mixed_declarations() {
    let file = ok("struct P(val x: Int)\n\nfun main() {\n}\n");
    assert_eq!(file.declarations.len(), 2);
    assert!(matches!(file.declarations[0], Decl::Struct(_)));
    assert!(matches!(file.declarations[1], Decl::Function(_)));
}
