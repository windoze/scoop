use scoop_ast::{Decl, FunctionBody, MethodModifier, Span, TypeRefKind};

use crate::tests::ok;

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
        "SourceFile\n  RootPackage\n  interface Describable\n    fun describe\n"
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
fn generic_interface_and_applied_supertype() {
    let file = ok(
        "interface Flow<T, E, U> { fun next(): T\n fun fail(e: E) }\nclass C : Flow<Int, String, Boolean> {}",
    );
    let Decl::Interface(interface) = &file.declarations[0] else {
        panic!("expected interface");
    };
    assert_eq!(interface.type_params.len(), 3);
    let Decl::Class(class) = &file.declarations[1] else {
        panic!("expected class");
    };
    assert!(matches!(
        &class.supertypes[0].ty.kind,
        TypeRefKind::Generic(name, args) if name.text == "Flow" && args.len() == 3
    ));
}

#[test]
fn interface_method_body_is_preserved() {
    let file = ok("interface I {\n    fun f() = 1\n}\n");
    let Decl::Interface(interface) = &file.declarations[0] else {
        panic!("expected interface")
    };
    assert!(matches!(interface.methods[0].body, FunctionBody::Expr(_)));
}

#[test]
fn interface_property_is_preserved() {
    let file = ok("interface I {\n    val x: Int\n}\n");
    let Decl::Interface(interface) = &file.declarations[0] else {
        panic!("expected interface")
    };
    assert!(matches!(
        interface.properties[0].body,
        scoop_ast::PropertyBodySyntax::Computed(_)
    ));
}
