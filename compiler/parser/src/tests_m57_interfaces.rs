use crate::tests::{err, ok};
use scoop_ast::{
    ClassMember, Decl, InterfaceImplementationMember, StructMember, TypeConstraintSubject,
};

#[test]
fn implementation_is_distinct_from_a_nested_interface_and_retains_conditions() {
    let file = ok(r#"
        struct Box<T>(val value: T) {
            interface Nested {}
            override interface Hash where T : Hash {
                public override fun hash(): Long = value.hash()
            }
            public companion object {
                override interface ValueEncoder<Box<T>> where T.Companion : ValueEncoder<T> {}
            }
        }
    "#);
    let Decl::Struct(host) = &file.declarations[0] else {
        panic!("struct")
    };
    assert!(matches!(host.members[0], StructMember::Nested(_)));
    let StructMember::InterfaceImplementation(block) = &host.members[1] else {
        panic!("implementation")
    };
    assert_eq!(block.functions().next().unwrap().name.text, "hash");
    assert_eq!(block.where_clause.constraints.len(), 1);
    let StructMember::Companion(companion) = &host.members[2] else {
        panic!("companion")
    };
    let ClassMember::InterfaceImplementation(block) = &companion.members[0] else {
        panic!("implementation")
    };
    assert!(matches!(
        block.where_clause.constraints[0].subject,
        TypeConstraintSubject::Companion(_)
    ));
    assert!(block.members.is_empty());
    assert!(
        scoop_ast::dump(&file).contains(
            "override interface ValueEncoder<Box<T>> where T.Companion : ValueEncoder<T>"
        )
    );
}

#[test]
fn ordinary_owners_allow_conditional_blocks_and_computed_members() {
    for source in [
        "class C<T> { override interface I where T : ref { public override val value: Int get() = 1 } }",
        "object O { override interface I where T : ref { public override val value: Int get() = 1 } }",
        "enum E<T> { One\n override interface I where T : ref { public override val value: Int get() = 1 } }",
    ] {
        let file = ok(source);
        let block = match &file.declarations[0] {
            Decl::Class(host) => match &host.members[0] {
                ClassMember::InterfaceImplementation(block) => block.as_ref(),
                _ => panic!("implementation"),
            },
            Decl::Object(host) => match &host.members[0] {
                ClassMember::InterfaceImplementation(block) => block.as_ref(),
                _ => panic!("implementation"),
            },
            Decl::Enum(host) => &host.interface_implementations[0],
            _ => panic!("owner"),
        };
        assert_eq!(block.where_clause.constraints.len(), 1);
        assert!(matches!(
            block.members[0],
            InterfaceImplementationMember::Property(_)
        ));
    }
}

#[test]
fn blocks_require_explicit_nonempty_conditions() {
    for source in [
        "class C<T> { override interface I {} }",
        "struct S<T>() { override interface I {} }",
        "enum E<T> { One\n override interface I {} }",
        "object O { override interface I {} }",
        "class C<T> { companion object { override interface I {} } }",
    ] {
        let (span, message) = err(source);
        assert_eq!(&source[span.start as usize..span.end as usize], "I");
        assert!(
            message.contains("requires a non-empty `where` clause"),
            "{message}"
        );
        assert!(message.contains("type or companion's interface list"));
    }
}

#[test]
fn blocks_reject_external_locations_new_binders_and_nonpositive_conditions() {
    for source in [
        "override interface I {}",
        "interface I { override interface J {} }",
        "fun f() { override interface I {} }",
        "class C<T> { override interface <U> I<U> {} }",
        "class C<T> { override interface I where {} }",
        "class C<T> { override interface I where T : !J {} }",
        "class C<T> { override interface I where T.Companion : !J {} }",
        "class C<T> { override interface I where requires(x:T) { x.f(): Int } {} }",
        "class C { public override interface I {} }",
        "class C { @A override interface I {} }",
        "class C { sealed override interface I {} }",
        "class C { context(x: Int) override interface I {} }",
    ] {
        err(source);
    }
}

#[test]
fn blocks_cannot_add_storage_initialization_helpers_or_generic_slots() {
    for member in [
        "val field: Int = 1",
        "public override val field: Int = 1",
        "public override val field: Int by delegate",
        "fun helper(): Int = 1",
        "init {}",
        "constructor() {}",
        "companion object {}",
        "struct Nested()",
        "public override fun <T> f(value: T): T = value",
    ] {
        err(&format!(
            "class C<T> {{ override interface I where T : ref {{ {member} }} }}"
        ));
    }
}
