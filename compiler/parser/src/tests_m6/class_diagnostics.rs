use scoop_ast::{Decl, FunctionBody, Span};

use crate::tests::{err, ok};

// --- class diagnostics ---------------------------------------------------------

#[test]
fn ordinary_class_constructor_parameter_is_preserved() {
    let file = ok("class C(x: Int)");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(
        class.constructor[0].property,
        scoop_ast::PrimaryParameterProperty::Plain
    );
}

#[test]
fn class_body_property_requires_an_explicit_type() {
    let (span, message) = err("class C {\n    val y = 1\n}\n");
    assert_eq!(span, Span::new(20, 21));
    assert_eq!(message, "class stored properties require an explicit type");
}

#[test]
fn class_init_block_is_preserved() {
    let file = ok("class C {\n    init {}\n}\n");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    assert!(matches!(
        class.members[0],
        scoop_ast::ClassMember::InitBlock(_)
    ));
}

#[test]
fn secondary_constructor_is_preserved() {
    let file = ok("class C {\n    constructor() {}\n}\n");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    assert!(matches!(
        class.members[0],
        scoop_ast::ClassMember::SecondaryConstructor(_)
    ));
}

#[test]
fn companion_object_not_supported() {
    let (span, message) = err("class C {\n    companion object {}\n}\n");
    assert_eq!(span, Span::new(14, 23));
    assert_eq!(
        message,
        "companion objects are not supported yet (milestone M21)"
    );
}

#[test]
fn nested_object_declaration_not_supported() {
    let (span, message) = err("class C {\n    object O {}\n}\n");
    assert_eq!(span, Span::new(14, 20));
    assert_eq!(
        message,
        "`object` declarations are not supported yet (milestone M21)"
    );
}

#[test]
fn nested_class_declaration_not_supported() {
    let (span, message) = err("class C {\n    class D {}\n}\n");
    assert_eq!(span, Span::new(14, 19));
    assert_eq!(
        message,
        "nested type declarations are not supported yet (milestone M21)"
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
    assert!(matches!(
        class.functions().next().expect("class method").body,
        FunctionBody::None
    ));
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
fn constructor_arguments_are_preserved_until_nominal_kind_resolution() {
    let file = ok("class C : I, Base(1) {}");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(class.supertypes.len(), 2);
    assert!(class.supertypes[0].constructor_arguments.is_none());
    assert!(class.supertypes[1].constructor_arguments.is_some());
}
