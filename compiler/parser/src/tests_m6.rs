//! Unit tests for the M6 syntax: class declarations (constructor
//! properties, base-class delegation, interfaces, member functions),
//! interface declarations, `open` / `abstract` / `override` modifiers,
//! struct/enum interface lists and member functions, field assignment,
//! `this`, method calls, the type operators `is` / `!is` / `as` / `as?`,
//! reference equality (`===` / `!==`), and every M6 "not supported"
//! diagnostic.

use scoop_ast::{
    AssignTarget, ClassModifier, Decl, Expr, FunctionBody, MethodModifier, Span, StatementKind,
    TypeRefKind,
};

use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::{init_expr, stmt_dump};

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
    assert!(!x.mutable);
    assert_eq!(x.name.text, "x");
    assert_eq!(x.span, Span::new(12, 22));
    let y = &decl.constructor[1];
    assert!(y.mutable);
    assert_eq!(y.name.text, "y");
    assert_eq!(y.span, Span::new(24, 34));
    assert!(decl.base_class.is_none());
    assert!(decl.interfaces.is_empty());
    assert!(decl.methods.is_empty());
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  class Point(val x: Int, var y: Int)\n"
    );
}

#[test]
fn class_with_base_and_interfaces() {
    let file = ok("class Point(val x: Int) : Shape(\"point\"), Describable {}");
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    let (base, args) = decl.base_class.as_ref().expect("a base class");
    assert_eq!(base.text, "Shape");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::StringLiteral { value, .. } if value == "point"));
    assert_eq!(decl.interfaces.len(), 1);
    assert!(
        matches!(&decl.interfaces[0].kind, TypeRefKind::Named(name) if name.text == "Describable")
    );
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  class Point(val x: Int) : Shape(<1 args>), Describable\n"
    );
}

#[test]
fn class_with_multiple_interfaces() {
    let file = ok("class C : I1, I2 {}");
    let Decl::Class(decl) = &file.declarations[0] else {
        panic!("expected a class declaration");
    };
    assert!(decl.base_class.is_none());
    assert!(decl.constructor.is_empty());
    let names: Vec<&str> = decl
        .interfaces
        .iter()
        .map(|ty| match &ty.kind {
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
    assert_eq!(decl.methods.len(), 1);
    let method = &decl.methods[0];
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
    assert_eq!(decl.methods.len(), 2);
    assert!(!decl.methods[0].is_override);
    assert!(matches!(decl.methods[0].body, FunctionBody::Block(_)));
    let b = &decl.methods[1];
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
    let method = &decl.methods[0];
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
    assert_eq!(decl.methods[0].modifier, MethodModifier::Final);
    assert_eq!(decl.methods[1].modifier, MethodModifier::Open);
    // An override remains open unless explicitly closed.
    assert_eq!(decl.methods[2].modifier, MethodModifier::Open);
    assert!(decl.methods[2].is_override);
    assert_eq!(decl.methods[3].modifier, MethodModifier::Final);
    assert!(decl.methods[3].is_override);
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

// --- class diagnostics ---------------------------------------------------------

#[test]
fn class_constructor_parameter_must_be_a_property() {
    let (span, message) = err("class C(x: Int)");
    assert_eq!(span, Span::new(8, 9));
    assert_eq!(
        message,
        "class constructor parameters must be properties declared with `val` or `var`"
    );
}

#[test]
fn class_body_property_not_supported() {
    let (span, message) = err("class C {\n    val y = 1\n}\n");
    assert_eq!(span, Span::new(14, 17));
    assert_eq!(
        message,
        "member properties are not supported yet (milestone M6)"
    );
}

#[test]
fn class_init_block_not_supported() {
    let (span, message) = err("class C {\n    init {}\n}\n");
    assert_eq!(span, Span::new(14, 18));
    assert_eq!(
        message,
        "`init` blocks are not supported yet (milestone M6)"
    );
}

#[test]
fn secondary_constructor_not_supported() {
    let (span, message) = err("class C {\n    constructor() {}\n}\n");
    assert_eq!(span, Span::new(14, 25));
    assert_eq!(
        message,
        "secondary constructors are not supported yet (milestone M6)"
    );
}

#[test]
fn companion_object_not_supported() {
    let (span, message) = err("class C {\n    companion object {}\n}\n");
    assert_eq!(span, Span::new(14, 23));
    assert_eq!(
        message,
        "companion objects are not supported yet (milestone M6)"
    );
}

#[test]
fn nested_object_declaration_not_supported() {
    let (span, message) = err("class C {\n    object O {}\n}\n");
    assert_eq!(span, Span::new(14, 20));
    assert_eq!(
        message,
        "`object` declarations are not supported yet (milestone M6)"
    );
}

#[test]
fn nested_class_declaration_not_supported() {
    let (span, message) = err("class C {\n    class D {}\n}\n");
    assert_eq!(span, Span::new(14, 19));
    assert_eq!(
        message,
        "nested type declarations are not supported yet (milestone M6)"
    );
}

#[test]
fn object_declaration_not_supported() {
    let (span, message) = err("object O {}");
    assert_eq!(span, Span::new(0, 6));
    assert_eq!(
        message,
        "`object` declarations are not supported yet (milestone M6)"
    );
}

#[test]
fn sealed_class_not_supported() {
    let (span, message) = err("sealed class C {}");
    assert_eq!(span, Span::new(0, 6));
    assert_eq!(
        message,
        "`sealed` classes are not supported yet (milestone M6)"
    );
}

#[test]
fn open_must_be_followed_by_class() {
    let (_, message) = err("open fun f() {}");
    assert_eq!(message, "expected `class`, found `fun`");
}

#[test]
fn bodyless_member_is_preserved_for_hir_validation() {
    let file = ok("class C {\n    fun f()\n}\n");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class");
    };
    assert!(matches!(class.methods[0].body, FunctionBody::None));
}

#[test]
fn abstract_function_must_not_have_a_body() {
    let (span, message) = err("abstract class C {\n    abstract fun f() {}\n}\n");
    assert_eq!(span, Span::new(40, 41));
    assert_eq!(message, "`abstract` functions must not have a body");
}

#[test]
fn duplicate_member_modifier_is_an_error() {
    let (_, message) = err("class C {\n    override override fun f() = 1\n}\n");
    assert_eq!(message, "duplicate `override` modifier on member function");
}

#[test]
fn constructor_arguments_only_on_the_base_class() {
    let (span, message) = err("class C : I, Base(1) {}");
    assert_eq!(span, Span::new(17, 18));
    assert_eq!(
        message,
        "constructor arguments are only allowed on the base class (the first supertype)"
    );
}

// --- interface declarations ---------------------------------------------------

#[test]
fn interface_decl() {
    let file = ok("interface Describable {\n    fun describe(): String\n}\n");
    let Decl::Interface(decl) = &file.declarations[0] else {
        panic!("expected an interface declaration");
    };
    assert_eq!(decl.name.text, "Describable");
    assert_eq!(decl.name.span, Span::new(10, 21));
    assert_eq!(decl.span, Span::new(0, 52));
    assert_eq!(decl.methods.len(), 1);
    let method = &decl.methods[0];
    assert_eq!(method.name.text, "describe");
    assert_eq!(method.span, Span::new(28, 50));
    assert!(!method.is_override);
    assert_eq!(method.modifier, MethodModifier::Abstract);
    assert!(matches!(method.body, FunctionBody::None));
    assert!(method.return_ty.is_some());
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  interface Describable\n    fun describe\n"
    );
}

#[test]
fn interface_with_multiple_methods() {
    let file = ok("interface I {\n    fun a(x: Int): String\n    fun b()\n}\n");
    let Decl::Interface(decl) = &file.declarations[0] else {
        panic!("expected an interface declaration");
    };
    assert_eq!(decl.methods.len(), 2);
    assert_eq!(decl.methods[0].params.len(), 1);
    assert!(decl.methods[1].return_ty.is_none());
    assert!(
        decl.methods
            .iter()
            .all(|m| matches!(m.body, FunctionBody::None))
    );
}

#[test]
fn generic_interface_variance_and_applied_supertype() {
    let file = ok(
        "interface Flow<out T, in E, U> { fun next(): T\n fun fail(e: E) }\nclass C : Flow<Int, String, Boolean> {}",
    );
    let Decl::Interface(interface) = &file.declarations[0] else {
        panic!("expected interface");
    };
    assert_eq!(interface.type_params.len(), 3);
    assert_eq!(interface.type_params[0].variance, scoop_ast::Variance::Out);
    assert_eq!(interface.type_params[1].variance, scoop_ast::Variance::In);
    assert_eq!(
        interface.type_params[2].variance,
        scoop_ast::Variance::Invariant
    );
    let Decl::Class(class) = &file.declarations[1] else {
        panic!("expected class");
    };
    assert!(matches!(
        &class.interfaces[0].kind,
        TypeRefKind::Generic(name, args) if name.text == "Flow" && args.len() == 3
    ));
}

#[test]
fn interface_method_body_not_supported() {
    let (span, message) = err("interface I {\n    fun f() = 1\n}\n");
    assert_eq!(span, Span::new(26, 27));
    assert_eq!(
        message,
        "interface method bodies are not supported yet (milestone M6)"
    );
}

#[test]
fn interface_property_not_supported() {
    let (span, message) = err("interface I {\n    val x: Int\n}\n");
    assert_eq!(span, Span::new(18, 21));
    assert_eq!(
        message,
        "member properties are not supported yet (milestone M6)"
    );
}

// --- struct / enum member functions --------------------------------------------

#[test]
fn struct_member_functions() {
    let file = ok("struct S(val v: Int) {\n    fun describe(): String = \"S\"\n}\n");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.fields.len(), 1);
    assert_eq!(decl.methods.len(), 1);
    assert_eq!(decl.methods[0].name.text, "describe");
    assert!(matches!(decl.methods[0].body, FunctionBody::Expr(_)));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  struct S\n    field v: Int\n    fun describe\n"
    );
}

#[test]
fn struct_without_body_still_parses() {
    // The M2 form (constructor only, no member body) stays legal.
    let file = ok("struct S(val v: Int)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert!(decl.methods.is_empty());
    assert_eq!(decl.span, Span::new(0, 20));
}

#[test]
fn struct_override_member_function() {
    // Value types implementing an interface also write `override`
    // (DESIGN.md 5.2).
    let file = ok("struct S(val v: Int) {\n    override fun describe(): String = \"S\"\n}\n");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert!(decl.methods[0].is_override);
}

#[test]
fn struct_interface_list() {
    let file = ok(
        "struct S(val v: Int) : Describable {\n    override fun describe(): String = \"S\"\n}\n",
    );
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.interfaces.len(), 1);
    assert!(
        matches!(&decl.interfaces[0].kind, TypeRefKind::Named(name) if name.text == "Describable")
    );
    assert_eq!(decl.methods.len(), 1);
    assert!(decl.methods[0].is_override);
}

#[test]
fn struct_interface_list_without_body() {
    let file = ok("struct S(val v: Int) : I1, I2");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    let names: Vec<&str> = decl
        .interfaces
        .iter()
        .map(|ty| match &ty.kind {
            TypeRefKind::Named(name) => name.text.as_str(),
            _ => panic!("expected a named interface"),
        })
        .collect();
    assert_eq!(names, ["I1", "I2"]);
    assert!(decl.methods.is_empty());
    assert_eq!(decl.span, Span::new(0, 29));
}

#[test]
fn struct_base_class_is_an_error() {
    let (span, message) = err("struct S(val v: Int) : Base(1)");
    assert_eq!(span, Span::new(23, 27));
    assert_eq!(message, "value types cannot have a base class (spec 4.4)");
}

#[test]
fn enum_base_class_is_an_error() {
    let (span, message) = err("enum E : Base(1) { A }");
    assert_eq!(span, Span::new(9, 13));
    assert_eq!(message, "value types cannot have a base class (spec 4.4)");
}

#[test]
fn enum_interface_list() {
    let file =
        ok("enum E : Describable {\n    A,\n    override fun describe(): String = \"E\"\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.interfaces.len(), 1);
    assert!(
        matches!(&decl.interfaces[0].kind, TypeRefKind::Named(name) if name.text == "Describable")
    );
    assert_eq!(decl.variants.len(), 1);
    assert_eq!(decl.methods.len(), 1);
    assert!(decl.methods[0].is_override);
}

#[test]
fn enum_generic_with_interface_list() {
    let file = ok("enum Option<T> : Describable {\n    Some(T),\n    None\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.type_params.len(), 1);
    assert_eq!(decl.interfaces.len(), 1);
    assert_eq!(decl.variants.len(), 2);
}

#[test]
fn enum_member_functions_after_variants() {
    let file = ok("enum E {\n    A,\n    B(Int),\n    fun f(): Int = 1\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.variants.len(), 2);
    assert_eq!(decl.methods.len(), 1);
    assert_eq!(decl.methods[0].name.text, "f");
    assert!(matches!(decl.methods[0].body, FunctionBody::Expr(_)));
}

#[test]
fn enum_override_member_function() {
    let file = ok("enum E {\n    A\n    override fun f() = 1\n}\n");
    let Decl::Enum(decl) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(decl.variants.len(), 1);
    assert!(decl.methods[0].is_override);
}

// --- this / method calls ---------------------------------------------------------

#[test]
fn this_expression() {
    // `fun main() {\n    val x = this\n}\n` — `this` is at offset 25.
    let expr = init_expr("this");
    let Expr::This { span } = &expr else {
        panic!("expected a this expression");
    };
    assert_eq!(*span, Span::new(25, 29));
}

#[test]
fn method_call() {
    // `fun main() {\n    val x = p.moveTo(1, 2)\n}\n` — `p` is at offset 25.
    let expr = init_expr("p.moveTo(1, 2)");
    let Expr::MethodCall {
        receiver,
        name,
        args,
        span,
    } = &expr
    else {
        panic!("expected a method call");
    };
    assert_eq!(*span, Span::new(25, 39));
    assert_eq!(name.text, "moveTo");
    assert_eq!(name.span, Span::new(27, 33));
    assert!(matches!(receiver.as_ref(), Expr::Var(name) if name.text == "p"));
    assert_eq!(args.len(), 2);
    assert_eq!(
        stmt_dump("p.moveTo(1, 2)"),
        "MethodCall moveTo\n  Var p\n  IntLiteral 1\n  IntLiteral 2\n"
    );
}

#[test]
fn method_call_without_args() {
    let expr = init_expr("p.describe()");
    let Expr::MethodCall { name, args, .. } = &expr else {
        panic!("expected a method call");
    };
    assert_eq!(name.text, "describe");
    assert!(args.is_empty());
}

#[test]
fn method_call_on_field_access_chains() {
    let expr = init_expr("a.b.c(1)");
    let Expr::MethodCall { receiver, name, .. } = &expr else {
        panic!("expected a method call");
    };
    assert_eq!(name.text, "c");
    assert!(matches!(receiver.as_ref(), Expr::FieldAccess(_)));
}

#[test]
fn dot_name_without_parens_stays_a_field_access() {
    // `.name` without `(` is a field access, not a method call.
    let expr = init_expr("p.x");
    assert!(matches!(expr, Expr::FieldAccess(_)));
}

#[test]
fn method_call_after_safe_navigation_not_supported() {
    let (span, message) = err("fun main() { a?.m() }");
    assert_eq!(span, Span::new(17, 18));
    assert_eq!(
        message,
        "method calls with `?.` are not supported yet (milestone M6)"
    );
}

#[test]
fn super_call_not_supported() {
    let (span, message) = err("fun main() { super.foo() }");
    assert_eq!(span, Span::new(13, 18));
    assert_eq!(
        message,
        "`super` calls are not supported yet (milestone M6)"
    );
}

#[test]
fn bare_super_not_supported() {
    let (_, message) = err("fun main() { super }");
    assert_eq!(
        message,
        "`super` calls are not supported yet (milestone M6)"
    );
}

// --- is / !is / as / as? ---------------------------------------------------------

#[test]
fn is_expression() {
    // `fun main() {\n    val x = a is Int\n}\n` — `a` is at offset 25.
    let expr = init_expr("a is Int");
    let Expr::Is {
        operand,
        ty,
        negated,
        span,
    } = &expr
    else {
        panic!("expected an is expression");
    };
    assert_eq!(*span, Span::new(25, 33));
    assert!(!negated);
    assert!(matches!(operand.as_ref(), Expr::Var(name) if name.text == "a"));
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Int"));
}

#[test]
fn bang_is_expression() {
    let expr = init_expr("a !is Int");
    let Expr::Is { negated, span, .. } = &expr else {
        panic!("expected an is expression");
    };
    assert!(negated);
    assert_eq!(*span, Span::new(25, 34));
}

#[test]
fn is_binds_tighter_than_equality() {
    // `a is Int == true` is `(a is Int) == true` — the type operators sit
    // at comparison precedence, above `==`.
    let expr = init_expr("a is Int == true");
    let Expr::Binary { op, lhs, rhs, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::Eq);
    assert!(matches!(lhs.as_ref(), Expr::Is { .. }));
    assert!(matches!(
        rhs.as_ref(),
        Expr::BoolLiteral { value: true, .. }
    ));
}

#[test]
fn is_chains_left() {
    let expr = init_expr("a is Int is Any");
    let Expr::Is { operand, ty, .. } = &expr else {
        panic!("expected an is expression");
    };
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Any"));
    assert!(matches!(operand.as_ref(), Expr::Is { .. }));
}

#[test]
fn as_expression() {
    let expr = init_expr("a as Point");
    let Expr::Cast {
        operand,
        ty,
        optional,
        span,
    } = &expr
    else {
        panic!("expected a cast expression");
    };
    assert_eq!(*span, Span::new(25, 35));
    assert!(!optional);
    assert!(matches!(operand.as_ref(), Expr::Var(name) if name.text == "a"));
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Point"));
    assert_eq!(
        stmt_dump("val s = a as? S"),
        "val s\n  Cast S optional=true\n    Var a\n"
    );
}

#[test]
fn safe_cast_after_safe_navigation() {
    // `a?.x as? Point` is `(a?.x) as? Point` — postfix binds tighter.
    let expr = init_expr("a?.x as? Point");
    let Expr::Cast {
        operand,
        ty,
        optional,
        ..
    } = &expr
    else {
        panic!("expected a cast expression");
    };
    assert!(optional);
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Point"));
    let Expr::FieldAccess(access) = operand.as_ref() else {
        panic!("expected a field access operand");
    };
    assert!(access.safe);
}

#[test]
fn cast_with_nullable_type() {
    // `a as Point?` casts to the nullable type; the `?` belongs to the
    // type, so this is not the safe cast.
    let expr = init_expr("a as Point?");
    let Expr::Cast { ty, optional, .. } = &expr else {
        panic!("expected a cast expression");
    };
    assert!(!optional);
    assert!(matches!(&ty.kind, TypeRefKind::Nullable(_)));
}

#[test]
fn bang_is_requires_adjacent_tokens() {
    // `! is` with a space in between is not the `!is` operator.
    let (span, message) = err("fun main() { a ! is Int }");
    assert_eq!(span, Span::new(15, 16));
    assert_eq!(
        message,
        "expected `;` or newline after statement, found `!`"
    );
}

// --- reference equality -----------------------------------------------------------

#[test]
fn reference_equality() {
    let expr = init_expr("x === y");
    let Expr::Binary { op, span, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::RefEq);
    assert_eq!(*span, Span::new(25, 32));
    let expr = init_expr("x !== y");
    let Expr::Binary { op, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::RefNe);
}

#[test]
fn reference_equality_is_left_associative() {
    // `x === y !== z` is `(x === y) !== z`.
    let expr = init_expr("x === y !== z");
    let Expr::Binary { op, lhs, rhs, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::RefNe);
    assert!(matches!(rhs.as_ref(), Expr::Var(name) if name.text == "z"));
    let Expr::Binary {
        op: inner_op,
        lhs: inner_lhs,
        rhs: inner_rhs,
        ..
    } = lhs.as_ref()
    else {
        panic!("expected the left operand to be `x === y`");
    };
    assert_eq!(*inner_op, scoop_ast::BinOp::RefEq);
    assert!(matches!(inner_lhs.as_ref(), Expr::Var(name) if name.text == "x"));
    assert!(matches!(inner_rhs.as_ref(), Expr::Var(name) if name.text == "y"));
}

#[test]
fn reference_equality_at_equality_precedence() {
    // `a === b == c` is `(a === b) == c` — same tier, left associative.
    let expr = init_expr("a === b == c");
    let Expr::Binary { op, lhs, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::Eq);
    assert!(matches!(
        lhs.as_ref(),
        Expr::Binary {
            op: scoop_ast::BinOp::RefEq,
            ..
        }
    ));
}

// --- field assignment -----------------------------------------------------------

#[test]
fn field_assignment() {
    // `fun main() {\n    p.y = 3\n}\n` — `p` is at offset 17.
    let file = ok("fun main() {\n    p.y = 3\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    assert_eq!(stmt.span, Span::new(17, 24));
    let StatementKind::Assign(assign) = &stmt.kind else {
        panic!("expected an assignment");
    };
    assert_eq!(assign.span, Span::new(17, 24));
    let AssignTarget::Field {
        receiver,
        name,
        span,
    } = &assign.target
    else {
        panic!("expected a field assignment target");
    };
    assert_eq!(*span, Span::new(17, 20));
    assert_eq!(name.text, "y");
    assert_eq!(name.span, Span::new(19, 20));
    assert!(matches!(receiver.as_ref(), Expr::Var(name) if name.text == "p"));
    assert!(matches!(assign.value, Expr::IntLiteral { value: 3, .. }));
    assert_eq!(stmt_dump("p.y = 3"), "assign .y\n  Var p\n  IntLiteral 3\n");
}

#[test]
fn field_assignment_nested_receiver() {
    assert_eq!(
        stmt_dump("p.x.f = 1"),
        "assign .f\n  FieldAccess x\n    Var p\n  IntLiteral 1\n"
    );
}

#[test]
fn field_assignment_on_this() {
    assert_eq!(
        stmt_dump("this.x = 1"),
        "assign .x\n  This\n  IntLiteral 1\n"
    );
}

#[test]
fn subscript_of_field_stays_an_index_assignment() {
    // `a.b[i]` is an Index expression, so the assignment target keeps
    // the Index classification (the receiver is the field access).
    assert_eq!(
        stmt_dump("a.b[i] = 1"),
        "assign []\n  FieldAccess b\n    Var a\n  Var i\n  IntLiteral 1\n"
    );
}

#[test]
fn field_assignment_value_is_a_full_expression() {
    assert_eq!(
        stmt_dump("p.y = p.x + 1"),
        "assign .y\n  Var p\n  Binary Add\n    FieldAccess x\n      Var p\n    IntLiteral 1\n"
    );
}

#[test]
fn safe_navigation_assignment_not_allowed() {
    let (span, message) = err("fun main() { a?.b = 1 }");
    assert_eq!(span, Span::new(13, 17));
    assert_eq!(message, "assignments through `?.` are not allowed");
}
