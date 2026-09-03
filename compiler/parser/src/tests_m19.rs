use scoop_ast::{
    ClassConstructorDecl, ClassMember, ConstructorDelegation, Decl, Expr, PrimaryParameterProperty,
    StatementKind, StructMember,
};

use crate::parse;
use crate::tests::{block_body, err, ok, only_function};

#[test]
fn primary_parameters_keep_plain_val_var_and_calling_shape() {
    let file = ok("class User(raw: String, val id: Int = 1, vararg var flags: Int) : Base(id) {}");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    let ClassConstructorDecl::Declared(parameters) = &class.constructor else {
        panic!("expected explicit primary constructor")
    };
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].property, PrimaryParameterProperty::Plain);
    assert_eq!(parameters[1].property, PrimaryParameterProperty::Val);
    assert_eq!(parameters[2].property, PrimaryParameterProperty::Var);
    assert!(matches!(
        parameters[2].syntax,
        scoop_ast::ParameterSyntax::Vararg { .. }
    ));
    assert_eq!(class.supertypes.len(), 1);
    assert!(class.supertypes[0].constructor_arguments.is_some());
}

#[test]
fn class_members_preserve_initialization_source_order() {
    let file = ok("class C(value: Int) {\n\
             val first: Int = value\n\
             fun between() {}\n\
             init { println(first) }\n\
             constructor() : this(1) {}\n\
             var last: Int = first\n\
         }");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    assert!(matches!(class.members[0], ClassMember::StoredProperty(_)));
    assert!(matches!(class.members[1], ClassMember::Function(_)));
    assert!(matches!(class.members[2], ClassMember::InitBlock(_)));
    let ClassMember::SecondaryConstructor(constructor) = &class.members[3] else {
        panic!("expected secondary constructor")
    };
    assert!(matches!(
        constructor.delegation,
        Some(ConstructorDelegation::This { .. })
    ));
    assert!(matches!(class.members[4], ClassMember::StoredProperty(_)));
}

#[test]
fn secondary_constructor_forms_parse_for_classes_and_structs() {
    let file = ok("class Base { constructor(value: Int = 1) {} }\n\
         class Child : Base { constructor() : super(2) {} }\n\
         struct S(val value: Int) { constructor() : this(1) {} }");
    let Decl::Class(base) = &file.declarations[0] else {
        panic!("expected base class")
    };
    let ClassMember::SecondaryConstructor(base_constructor) = &base.members[0] else {
        panic!("expected base constructor")
    };
    assert!(base_constructor.delegation.is_none());
    let Decl::Class(child) = &file.declarations[1] else {
        panic!("expected child class")
    };
    let ClassMember::SecondaryConstructor(child_constructor) = &child.members[0] else {
        panic!("expected child constructor")
    };
    assert!(matches!(
        child_constructor.delegation,
        Some(ConstructorDelegation::Super { .. })
    ));
    let Decl::Struct(structure) = &file.declarations[2] else {
        panic!("expected struct")
    };
    assert!(matches!(
        structure.members[0],
        StructMember::SecondaryConstructor(_)
    ));
}

#[test]
fn every_supertype_keeps_parentheses_independently() {
    let file = ok("class C : I(), Base, J(1) {}\ninterface K : I() {} ");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    assert!(class.supertypes[0].constructor_arguments.is_some());
    assert!(class.supertypes[1].constructor_arguments.is_none());
    assert!(class.supertypes[2].constructor_arguments.is_some());
    let Decl::Interface(interface) = &file.declarations[1] else {
        panic!("expected interface")
    };
    assert!(interface.supertypes[0].constructor_arguments.is_some());
}

#[test]
fn super_method_call_has_a_dedicated_ast_shape() {
    let file = ok("fun main() { super.render<String>(prefix = value, *rest) { 1 } }");
    let StatementKind::Expr(Expr::SuperMethodCall {
        name,
        type_args,
        args,
        ..
    }) = &block_body(only_function(&file)).statements[0].kind
    else {
        panic!("expected super call")
    };
    assert_eq!(name.text, "render");
    assert_eq!(type_args.len(), 1);
    assert_eq!(args.len(), 3);
}

#[test]
fn malformed_initialization_members_recover_independently() {
    let diagnostics = parse(
        "class C {\n\
             val missingType = 1\n\
             init()\n\
             constructor\n\
             val good: Int = 1\n\
         }\n\
         fun good() {}",
    )
    .expect_err("three malformed members must be reported");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "class stored properties require an explicit type",
            "`init` must be followed by a block",
            "expected `(` after `constructor`, found `val`",
        ]
    );
}

#[test]
fn illegal_constructor_and_super_shapes_have_specific_diagnostics() {
    assert_eq!(
        err("class C { constructor(val x: Int) {} }").1,
        "secondary constructor parameters cannot declare `val` or `var` properties"
    );
    assert_eq!(
        err("class C { constructor(): other() {} }").1,
        "constructor delegation target must be `this` or `super`"
    );
    assert_eq!(
        err("fun f() { super.field }").1,
        "`super` only supports method calls, not field access"
    );
    assert_eq!(
        err("fun f() { super::render }").1,
        "callable references cannot target `super`"
    );
    assert_eq!(
        err("fun f() { this() }").1,
        "`this(...)` is only valid in a constructor delegation clause"
    );
}
