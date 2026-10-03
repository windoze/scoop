use scoop_ast::{Decl, FunctionBody, Span, TypeRefKind};

use crate::tests::{err, ok};

// --- struct / enum member functions --------------------------------------------

#[test]
fn struct_member_functions() {
    let file = ok("struct S(val v: Int) {\n    fun describe(): String = \"S\"\n}\n");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.fields.len(), 1);
    let method = decl.functions().next().expect("struct method");
    assert_eq!(method.name.text, "describe");
    assert!(matches!(method.body, FunctionBody::Expr(_)));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n  struct S\n    field v: Int\n    fun describe\n"
    );
}

#[test]
fn struct_without_body_still_parses() {
    // The M2 form (constructor only, no member body) stays legal.
    let file = ok("struct S(val v: Int)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.functions().count(), 0);
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
    assert!(decl.functions().next().expect("struct method").is_override);
}

#[test]
fn struct_interface_list() {
    let file = ok(
        "struct S(val v: Int) : Describable {\n    override fun describe(): String = \"S\"\n}\n",
    );
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.supertypes.len(), 1);
    assert!(
        matches!(&decl.supertypes[0].ty.kind, TypeRefKind::Named(name) if name.text == "Describable")
    );
    assert_eq!(decl.functions().count(), 1);
    assert!(decl.functions().next().expect("struct method").is_override);
}

#[test]
fn struct_interface_list_without_body() {
    let file = ok("struct S(val v: Int) : I1, I2");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    let names: Vec<&str> = decl
        .supertypes
        .iter()
        .map(|supertype| match &supertype.ty.kind {
            TypeRefKind::Named(name) => name.text.as_str(),
            _ => panic!("expected a named interface"),
        })
        .collect();
    assert_eq!(names, ["I1", "I2"]);
    assert_eq!(decl.functions().count(), 0);
    assert_eq!(decl.span, Span::new(0, 29));
}

#[test]
fn struct_argument_bearing_supertype_is_preserved_for_hir() {
    let file = ok("struct S(val v: Int) : Base(1)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected struct")
    };
    assert!(decl.supertypes[0].constructor_arguments.is_some());
}

#[test]
fn enum_base_class_is_an_error() {
    let (span, message) = err("enum E : Base(1) { A }");
    assert_eq!(span, Span::new(9, 16));
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
