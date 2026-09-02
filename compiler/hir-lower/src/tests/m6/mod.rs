//! M6 tests: the reference-type hierarchy — class and interface
//! declarations, inheritance and override rules, interface
//! implementation, method calls (resolved against the receiver's
//! static type), class field reads with base-chain layout, boxing at
//! subtype crossings, `is` / `as` / `as?` / `===`, smart casts, and
//! struct / enum member functions. One negative test per diagnostic;
//! golden dumps and structural assertions lock the output shape.

use super::*;

use ast::ClassModifier::{Abstract, Final, Open};

// --- shared fixtures ---

/// `interface Describable { fun describe(): String }`.
fn describable() -> Decl {
    interface_decl(
        "Describable",
        vec![bodyless_method(
            false,
            "describe",
            vec![],
            Some(ty_named("String")),
        )],
    )
}

/// `open class Shape(val name: String) : Describable { override fun describe(): String = name }`.
fn shape() -> Decl {
    class_decl(
        Open,
        "Shape",
        vec![(false, "name", ty_named("String"))],
        None,
        vec!["Describable"],
        vec![override_method_expr(
            "describe",
            vec![],
            Some(ty_named("String")),
            var("name"),
        )],
    )
}

/// `class Point(val x: Int, var y: Int) : Shape("point") { fun moveTo(nx, ny) }`.
fn point() -> Decl {
    class_decl(
        Final,
        "Point",
        vec![(false, "x", ty_named("Int")), (true, "y", ty_named("Int"))],
        Some(("Shape", vec![str_lit("point")])),
        vec![],
        vec![method(
            "moveTo",
            vec![("nx", ty_named("Int")), ("ny", ty_named("Int"))],
            None,
            vec![stmt(call("println", vec![var("ny")]))],
        )],
    )
}

/// `struct S(val v: Int)`.
fn struct_s() -> Decl {
    struct_decl("S", vec![("v", ty_named("Int"))])
}

fn find_fn(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("function `{name}` must exist"))
}

fn body_of<'m>(module: &'m hir::Module, name: &str) -> &'m hir::Body {
    match &module.functions[find_fn(module, name)].kind {
        hir::FunctionKind::User(body) => body,
        other => panic!("function `{name}` must have a user body, found {other:?}"),
    }
}

/// The single `return <value>` expression of a body.
fn returned(body: &hir::Body) -> &hir::Expr {
    match &body.statements.last().expect("a return").kind {
        hir::StatementKind::Return { value: Some(value) } => value,
        other => panic!("expected a return statement, found {other:?}"),
    }
}

fn class_id(module: &hir::Module, name: &str) -> hir::ClassId {
    module
        .classes
        .iter()
        .find(|(_, c)| c.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("class `{name}` must exist"))
}

fn interface_id(module: &hir::Module, name: &str) -> hir::InterfaceId {
    module
        .interfaces
        .iter()
        .find(|(_, i)| i.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("interface `{name}` must exist"))
}

fn interface_ty(module: &hir::Module, name: &str) -> hir::TypeId {
    let id = interface_id(module, name);
    module.interface_applications[module.interfaces[id].self_application].canonical_type
}

fn class_application(module: &hir::Module, id: hir::ClassId) -> hir::ClassApplicationId {
    module.classes[id].self_application
}

mod conversions;
mod declarations;
mod inheritance_errors;
mod members;
mod type_operators;
mod value_interfaces;
mod value_members;
mod variant_resolution;
