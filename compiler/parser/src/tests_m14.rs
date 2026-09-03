//! M14 declaration-surface tests. Semantic legality of bounds, variance,
//! operator signatures and intrinsic providers belongs to HIR.

use scoop_ast::{
    ClassConstructorDecl, Decl, StatementKind, StructRepresentationDecl, TypeBound,
    TypeParamKindBound, TypeRefKind, Variance,
};

use crate::tests::{block_body, err, ok};

#[test]
fn generic_class_preserves_complete_host_and_supertype_syntax() {
    let file = ok(
        "class Derived<out T : ToString>(val value: T) : Base<T>(value), Render<T> \
         where T : Hash { operator fun equals(other: Derived<T>): Boolean = true }",
    );
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class");
    };
    assert_eq!(class.type_params.len(), 1);
    assert_eq!(class.type_params[0].variance, Variance::Out);
    assert!(matches!(
        &class.type_params[0].inline_bound,
        Some(TypeBound::Upper(ty))
            if matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "ToString")
    ));
    assert!(matches!(
        class.constructor,
        ClassConstructorDecl::Declared(_)
    ));
    let base = &class.supertypes[0].ty;
    let args = class.supertypes[0]
        .constructor_arguments
        .as_ref()
        .expect("generic base arguments");
    assert!(matches!(
        &base.kind,
        TypeRefKind::Generic(name, arguments)
            if name.text == "Base" && arguments.len() == 1
    ));
    assert_eq!(args.len(), 1);
    assert!(matches!(
        &class.supertypes[1].ty.kind,
        TypeRefKind::Generic(name, arguments)
            if name.text == "Render" && arguments.len() == 1
    ));
    let clause = class.where_clause.as_ref().expect("where clause");
    assert_eq!(clause.constraints.len(), 1);
    assert_eq!(clause.constraints[0].parameter.text, "T");
    assert!(
        class
            .functions()
            .next()
            .expect("class method")
            .operator
            .is_some()
    );

    let dump = scoop_ast::dump(&file);
    assert!(dump.contains(
        "class Derived<out T : ToString>(val value: T) : Base<T>(<1 args>), Render<T> where T : Hash"
    ));
    assert!(dump.contains("operator fun equals"));
}

#[test]
fn where_clauses_are_structured_on_every_generic_declaration_kind() {
    let file = ok(
        "fun <T> render(value: T): String where T : ToString, T : Hash = value.toString()\n\
         struct Box<T>(val value: T) where T : ToString\n\
         enum Choice<T> where T : ToString { Value(T), Empty }\n\
         interface Source<out T> : Parent<T> where T : ToString { fun get(): T }",
    );

    let Decl::Function(function) = &file.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(
        function
            .where_clause
            .as_ref()
            .expect("function where")
            .constraints
            .len(),
        2
    );
    let Decl::Struct(struct_) = &file.declarations[1] else {
        panic!("expected struct");
    };
    assert!(struct_.where_clause.is_some());
    let Decl::Enum(enum_) = &file.declarations[2] else {
        panic!("expected enum");
    };
    assert!(enum_.where_clause.is_some());
    let Decl::Interface(interface) = &file.declarations[3] else {
        panic!("expected interface");
    };
    assert!(interface.where_clause.is_some());
    assert!(matches!(
        &interface.supertypes[0].ty.kind,
        TypeRefKind::Generic(name, arguments)
            if name.text == "Parent" && arguments.len() == 1
    ));
}

#[test]
fn parser_preserves_variance_and_kind_bounds_for_hir() {
    let file = ok("struct Box<in T : value>(val value: T)");
    let Decl::Struct(struct_) = &file.declarations[0] else {
        panic!("expected struct");
    };
    assert_eq!(struct_.type_params[0].variance, Variance::In);
    assert_eq!(
        struct_.type_params[0].inline_bound,
        Some(TypeBound::Kind(TypeParamKindBound::Value))
    );
}

#[test]
fn omitted_and_explicit_empty_representations_stay_distinct() {
    let file = ok("@Intrinsic(\"core_int\") struct Int\n\
         struct Marker()\n\
         @Intrinsic(\"core_array\") class Array<T>\n\
         class Empty()");
    let Decl::Struct(int) = &file.declarations[0] else {
        panic!("expected intrinsic struct");
    };
    assert!(matches!(int.fields, StructRepresentationDecl::Omitted));
    let Decl::Struct(marker) = &file.declarations[1] else {
        panic!("expected marker struct");
    };
    assert!(matches!(
        marker.fields,
        StructRepresentationDecl::Declared(ref fields) if fields.is_empty()
    ));
    let Decl::Class(array) = &file.declarations[2] else {
        panic!("expected intrinsic class");
    };
    assert!(array.constructor.is_omitted());
    let Decl::Class(empty) = &file.declarations[3] else {
        panic!("expected ordinary class");
    };
    assert!(matches!(
        empty.constructor,
        ClassConstructorDecl::Declared(ref properties) if properties.is_empty()
    ));
}

#[test]
fn operator_modifier_is_preserved_on_all_function_locations() {
    let file = ok("operator fun top(value: Int): Boolean = true\n\
         class C { final operator fun member(value: Int): Boolean = true }\n\
         fun main() { operator fun local(value: Int): Boolean = true }");
    let Decl::Function(top) = &file.declarations[0] else {
        panic!("expected top-level function");
    };
    assert!(top.operator.is_some());
    let Decl::Class(class) = &file.declarations[1] else {
        panic!("expected class");
    };
    assert!(
        class
            .functions()
            .next()
            .expect("class method")
            .operator
            .is_some()
    );
    let Decl::Function(main) = &file.declarations[2] else {
        panic!("expected main");
    };
    let StatementKind::LocalFunction(local) = &block_body(main).statements[0].kind else {
        panic!("expected local function");
    };
    assert!(local.operator.is_some());

    let (span, message) = err("operator operator fun duplicate() = true");
    assert_eq!(span.start, 9);
    assert_eq!(message, "duplicate `operator` modifier on function");
}
