use super::*;

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
    assert!(matches!(
        &*fields[1].subpattern,
        Pattern::Binding(name) if name.text == "yy"
    ));
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
