//! Unit tests for the M9 syntax: generic struct declarations
//! (`struct Name<T, U>(...)` — the type parameter list sits between the
//! name and the constructor parentheses, like `enum`), fields typed by
//! type parameters, coexistence with generic enums, and the empty type
//! parameter list diagnostic. Explicit type arguments at construction
//! sites (`PinnedPtr<UInt>(1)`) stay unsupported, as for generic
//! function calls since M3.

use scoop_ast::{Decl, Span, TypeRefKind};

use crate::tests::{err, ok};

#[test]
fn generic_struct_single_type_param() {
    // The motivating case: `sysroot/lib/scoop.core/src/gc.scoop`.
    let file = ok("struct PinnedPtr<T>(val raw: UInt)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.name.text, "PinnedPtr");
    assert_eq!(decl.type_params.len(), 1);
    assert_eq!(decl.type_params[0].name.text, "T");
    assert_eq!(decl.type_params[0].span, Span::new(17, 18));
    assert_eq!(decl.fields.len(), 1);
    let TypeRefKind::Named(ty) = &decl.fields[0].ty.kind else {
        panic!("expected a named field type");
    };
    assert_eq!(ty.text, "UInt");
    assert_eq!(decl.span, Span::new(0, 34));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n  struct PinnedPtr<T>\n    field raw: UInt\n"
    );
}

#[test]
fn generic_struct_field_references_type_param() {
    let file = ok("struct Wrapper<T>(val value: T)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.type_params.len(), 1);
    let TypeRefKind::Named(ty) = &decl.fields[0].ty.kind else {
        panic!("expected a named field type");
    };
    assert_eq!(ty.text, "T");
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n  struct Wrapper<T>\n    field value: T\n"
    );
}

#[test]
fn generic_struct_multiple_type_params() {
    let file = ok("struct Pair<T, U>(val first: T, val second: U)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    let names: Vec<&str> = decl
        .type_params
        .iter()
        .map(|p| p.name.text.as_str())
        .collect();
    assert_eq!(names, ["T", "U"]);
    assert_eq!(decl.fields.len(), 2);
    let TypeRefKind::Named(first) = &decl.fields[0].ty.kind else {
        panic!("expected a named field type");
    };
    let TypeRefKind::Named(second) = &decl.fields[1].ty.kind else {
        panic!("expected a named field type");
    };
    assert_eq!(first.text, "T");
    assert_eq!(second.text, "U");
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n  struct Pair<T, U>\n    field first: T\n    field second: U\n"
    );
}

#[test]
fn generic_struct_with_interfaces_and_methods() {
    let file = ok("struct S<T>(val x: T) : Describable {\n    fun describe(): String = \"s\"\n}\n");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.type_params.len(), 1);
    assert_eq!(decl.supertypes.len(), 1);
    assert!(
        matches!(&decl.supertypes[0].ty.kind, TypeRefKind::Named(name) if name.text == "Describable")
    );
    assert_eq!(decl.functions().count(), 1);
}

#[test]
fn generic_structs_coexist_with_generic_enums_and_plain_structs() {
    let file = ok(
        "enum Option<T> { Some(T), None }\nstruct PinnedPtr<T>(val raw: UInt)\nstruct Point(val x: Int)",
    );
    assert_eq!(file.declarations.len(), 3);
    let Decl::Enum(enum_) = &file.declarations[0] else {
        panic!("expected an enum declaration");
    };
    assert_eq!(enum_.type_params.len(), 1);
    let Decl::Struct(generic) = &file.declarations[1] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(generic.type_params.len(), 1);
    assert_eq!(generic.type_params[0].name.text, "T");
    let Decl::Struct(plain) = &file.declarations[2] else {
        panic!("expected a struct declaration");
    };
    assert!(plain.type_params.is_empty());
}

#[test]
fn empty_type_parameter_list_is_an_error() {
    let (span, message) = err("struct S<>(val x: Int)");
    assert_eq!(span, Span::new(9, 10));
    assert_eq!(message, "expected type parameter name, found `>`");
}
