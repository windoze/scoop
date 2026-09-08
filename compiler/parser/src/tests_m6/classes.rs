use scoop_ast::{
    ClassModifier, Decl, Expr, FunctionBody, MethodModifier, PrimaryParameterProperty, Span,
    TypeRefKind,
};

use crate::tests::{err, ok};

// --- class declarations ------------------------------------------------------

#[test]
fn class_decl() {
    let file = ok("class Point(val x: Int, var y: Int)");
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    assert_eq!(decl.modifier, ClassModifier::Final);
    assert_eq!(decl.name.text, "Point");
    assert_eq!(decl.name.span, Span::new(6, 11));
    assert_eq!(decl.span, Span::new(0, 35));
    assert_eq!(decl.constructor.len(), 2);
    let x = &decl.constructor[0];
    assert_eq!(x.property, PrimaryParameterProperty::Val);
    assert_eq!(x.name.text, "x");
    assert_eq!(x.span, Span::new(12, 22));
    let y = &decl.constructor[1];
    assert_eq!(y.property, PrimaryParameterProperty::Var);
    assert_eq!(y.name.text, "y");
    assert_eq!(y.span, Span::new(24, 34));
    assert!(decl.supertypes.is_empty());
    assert_eq!(decl.functions().count(), 0);
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n  class Point(val x: Int, var y: Int)\n"
    );
}

#[test]
fn class_with_base_and_interfaces() {
    let file = ok("class Point(val x: Int) : Shape(\"point\"), Describable {}");
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    let base = &decl.supertypes[0].ty;
    let args = decl.supertypes[0]
        .constructor_arguments
        .as_ref()
        .expect("base arguments");
    assert!(matches!(
        &base.kind,
        TypeRefKind::Named(name) if name.text == "Shape"
    ));
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0].expression, Expr::StringLiteral { value, .. } if value == "point"));
    assert_eq!(decl.supertypes.len(), 2);
    assert!(
        matches!(&decl.supertypes[1].ty.kind, TypeRefKind::Named(name) if name.text == "Describable")
    );
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n  class Point(val x: Int) : Shape(<1 args>), Describable\n"
    );
}

#[test]
fn class_with_multiple_interfaces() {
    let file = ok("class C : I1, I2 {}");
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    assert!(decl.constructor.is_empty());
    let names: Vec<&str> = decl
        .supertypes
        .iter()
        .map(|supertype| match &supertype.ty.kind {
            TypeRefKind::Named(name) => name.text.as_str(),
            _ => panic!("expected a named interface"),
        })
        .collect();
    assert_eq!(names, ["I1", "I2"]);
}

#[test]
fn open_and_abstract_class_modifiers() {
    let file = ok("open class A {}\nabstract class B {}");
    let Decl::Class(a) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    assert_eq!(a.modifier, ClassModifier::Open);
    assert_eq!(a.span, Span::new(0, 15));
    let Decl::Class(b) = &file.declarations[1] else {
        panic!("expected a class declaration");
    };
    assert_eq!(b.modifier, ClassModifier::Abstract);
    assert_eq!(b.span, Span::new(16, 35));
}

#[test]
fn class_without_constructor_parens() {
    let file = ok("abstract class Base {\n    abstract fun kind(): Int\n}\n");
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    assert_eq!(decl.modifier, ClassModifier::Abstract);
    assert!(decl.constructor.is_empty());
    assert_eq!(decl.span, Span::new(0, 52));
    assert_eq!(decl.functions().count(), 1);
    let method = decl.functions().next().expect("class method");
    assert_eq!(method.modifier, MethodModifier::Abstract);
    assert!(!method.is_override);
    assert_eq!(method.name.text, "kind");
    assert_eq!(method.span, Span::new(26, 50));
    assert!(matches!(method.body, FunctionBody::None));
}

#[test]
fn class_member_functions() {
    let file = ok("class C {\n    fun a() {}\n    override fun b(): Int = 1\n}\n");
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    let methods = decl.functions().collect::<Vec<_>>();
    assert_eq!(methods.len(), 2);
    assert!(!methods[0].is_override);
    assert!(matches!(methods[0].body, FunctionBody::Block(_)));
    let b = methods[1];
    assert!(b.is_override);
    assert_eq!(b.modifier, MethodModifier::Open);
    assert_eq!(b.span, Span::new(29, 54));
    assert!(matches!(b.body, FunctionBody::Expr(_)));
}

#[test]
fn class_member_modifiers_in_any_order() {
    let file = ok("class C {\n    abstract override fun f(): Int\n}\n");
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    let method = decl.functions().next().expect("class method");
    assert_eq!(method.modifier, MethodModifier::Abstract);
    assert!(method.is_override);
    assert!(matches!(method.body, FunctionBody::None));
}

#[test]
fn method_modality_defaults_and_explicit_forms() {
    let file = ok(
        "class C {\n    fun a() {}\n    open fun b() {}\n    override fun c() {}\n    final override fun d() {}\n}\n",
    );
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    let methods = decl.functions().collect::<Vec<_>>();
    assert_eq!(methods[0].modifier, MethodModifier::Final);
    assert_eq!(methods[1].modifier, MethodModifier::Open);
    // An override remains open unless explicitly closed.
    assert_eq!(methods[2].modifier, MethodModifier::Open);
    assert!(methods[2].is_override);
    assert_eq!(methods[3].modifier, MethodModifier::Final);
    assert!(methods[3].is_override);
}

#[test]
fn method_modality_keywords_are_mutually_exclusive() {
    let (_, message) = err("class C { open final fun f() {} }");
    assert_eq!(
        message,
        "`open` and `final` cannot be combined on a member function"
    );
}

#[test]
fn interface_methods_cannot_be_final_or_open() {
    let (_, final_message) = err("interface I { final fun f() }");
    assert_eq!(
        final_message,
        "`final` modifier is not allowed on interface methods"
    );
    let (_, open_message) = err("interface I { open fun f() }");
    assert_eq!(
        open_message,
        "`open` modifier is not allowed on interface methods"
    );
}
