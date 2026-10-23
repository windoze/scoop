use super::*;

mod defaults;
mod inheritance;
mod projections;

const READ: &str = r#"
    interface Read { public fun read(): Int }
    struct Good(val number: Int) : Read {
        public override fun read(): Int = number
    }
    struct Empty()
    struct Box<T>(val value: T) {
        override interface Read where T : Read {
            public override fun read(): Int = value.read()
        }
    }
"#;

fn check(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
}

#[test]
fn m57_checked_conformance_keeps_its_owner_condition() {
    let output = check(&format!(
        r#"{READ}
        fun <T : Read> forward(value: Box<T>): Read = value
        fun main() {{
            val enabled = Box(Good(7))
            val ordinary = Box(Empty())
            val result = forward(enabled).read()
            val direct = enabled.read()
        }}
    "#
    ))
    .unwrap();
    let (_, host) = output
        .export
        .structs
        .iter()
        .find(|(_, host)| host.name == "Box")
        .unwrap();
    assert_eq!(host.conditional_interfaces.len(), 1);
    assert!(host.interfaces.is_empty());
    let condition = host.conditional_interfaces[0].id();
    assert!(host.interface_implementations.iter().any(|implementation| {
        implementation
            .condition
            .as_ref()
            .is_some_and(|application| application.declaration == condition)
    }));
}

#[test]
fn m57_unknown_condition_does_not_enable_conversion_or_a_direct_member() {
    for body in [
        "fun <T> invalid(value: Box<T>): Read = value",
        "fun <T> invalid(value: Box<T>): Int = value.read()",
        "fun invalid(value: Box<Empty>): Read = value",
        "fun invalid(value: Box<Empty>): Int = value.read()",
    ] {
        assert!(
            check(&format!("{READ}\n{body}\nfun main() {{}} ")).is_err(),
            "{body}"
        );
    }
}

#[test]
fn m57_block_facts_do_not_escape_into_ordinary_members() {
    let errors = check(
        r#"
        interface Read { public fun read(): Int }
        struct Box<T>(val value: T) {
            override interface Read where T : Read {
                public override fun read(): Int = value.read()
            }
            fun invalid(): Int = value.read()
        }
        fun main() {}
    "#,
    )
    .unwrap_err();
    assert!(
        errors.iter().any(|error| error.message.contains("read")),
        "{errors:#?}"
    );
}

#[test]
fn m57_overlap_is_rejected_even_for_empty_interfaces_and_stronger_conditions() {
    for (parents, declarations) in [
        (": Mark", "override interface Mark where T : Read {}"),
        (
            "",
            "override interface Mark where T : Read {}\n override interface Mark where T : Read, T : Hash {}",
        ),
        (
            "",
            "override interface Mark where T : Read {}\n override interface Mark where T : Hash {}",
        ),
    ] {
        let errors = check(&format!("interface Read {{}}\n interface Mark {{}}\n struct Value<T>(val value: T) {parents} {{ {declarations} }}\n fun main() {{}} ")).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("may overlap")),
            "{errors:#?}"
        );
    }
}

#[test]
fn m57_potential_conformance_cannot_be_used_as_a_negative_bound() {
    let errors = check(&format!(
        r#"{READ}
        fun <X : !Read> absent(value: X): Int = 1
        fun <T> invalid(value: Box<T>): Int = absent(value)
        fun main() {{}}
    "#
    ))
    .unwrap_err();
    assert!(
        errors.iter().any(|error| error.message.contains("Read")),
        "{errors:#?}"
    );
    check(&format!(
        r#"{READ}
        fun <X : !Read> absent(value: X): Int = 1
        fun main() {{ val result = absent(Box(Empty())) }}
    "#
    ))
    .unwrap();
}

#[test]
fn m57_class_enum_and_companion_blocks_use_their_original_owner() {
    check(
        r#"
        interface Read { public fun read(): Int }
        struct Good(val number: Int) : Read { public override fun read(): Int = number }
        class RefBox<T>(val value: T) {
            override interface Read where T : Read {
                public override fun read(): Int = value.read()
            }
        }
        enum Choice<T> {
            Value(val value: T)
            override interface Read where T : Read {
                public override fun read(): Int = 1
            }
        }
        struct Container<T>(val value: T) {
            public companion object {
                override interface Read where T : Read {
                    public override fun read(): Int = 2
                }
            }
        }
        fun take(value: Read): Int = value.read()
        fun main() {
            val a = take(RefBox(Good(7)))
            val b = take(Choice<Good>.Value(Good(8)))
            val c = take(Container<Good>.Companion)
        }
    "#,
    )
    .unwrap();
}

#[test]
fn m57_queries_reject_circular_proofs_but_allow_nested_descent() {
    for (source, message) in [
        (
            "interface Mark {}\n open class Gate<T> { override interface Mark where T : Mark {} }\n class X : Gate<X>()\n fun main() {}",
            "cyclic conditional interface proof",
        ),
        (
            "interface Mark {}\n struct Wrap<T>(val value:T)\n open class Gate<T> { override interface Mark where T : Mark {} }\n class Grow<T> : Gate<Grow<Wrap<T>>>()\n fun main() {}",
            "expanding conditional interface proof",
        ),
    ] {
        let errors = check(source).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(message)),
            "{errors:#?}"
        );
    }
    check(&format!(
        "{READ}\n fun main() {{ val result: Read = Box(Box(Good(9))) }}"
    ))
    .unwrap();
    check("interface Mark {}\n open class Gate<T> { override interface Mark where T : Mark {} }\n class X : Gate<X>(), Mark\n fun main() { val x: Mark = X() }").unwrap();
}

#[test]
fn m57_ordinary_override_can_serve_independent_conditional_slots() {
    check(
        r#"
        interface Read { public fun read(): Int }
        interface Inspect { public fun read(): Int }
        interface First {}
        interface Second {}
        struct Neither()
        struct Both() : First, Second
        struct Shared<T>(val value: T) {
            public override fun read(): Int = 7
            override interface Read where T : First {}
            override interface Inspect where T : Second {}
        }
        fun main() {
            val plain = Shared(Neither()).read()
            val first: Read = Shared(Both())
            val second: Inspect = Shared(Both())
        }
    "#,
    )
    .unwrap();
    let errors = check(
        r#"
        interface Read { public fun read(): Int }
        struct Shared<T>(val value: T) {
            public override fun read(): Int = value.read()
            override interface Read where T : Read {}
        }
        fun main() {}
    "#,
    )
    .unwrap_err();
    assert!(
        errors.iter().any(|error| error.message.contains("read")),
        "{errors:#?}"
    );
}

#[test]
fn m57_conditional_computed_properties_follow_accessor_availability() {
    let source = r#"
        interface Reading { public val number: Int }
        struct Good(val number: Int)
        interface Mark {}
        struct Present() : Mark
        struct Absent()
        struct Box<T>(val value: T) {
            override interface Reading where T : Mark {
                public override val number: Int get() = 7
            }
        }
        class MutableBox<T>(val value: T) {
            override interface Reading where T : Mark {
                public override val number: Int get() = 8
            }
        }
    "#;
    check(&format!("{source}\n fun main() {{ val a = Box(Present()).number; val b: Reading = MutableBox(Present()); val c = b.number }}")).unwrap();
    for body in [
        "fun invalid(value: Box<Absent>): Int = value.number",
        "fun <T> invalid(value: MutableBox<T>): Int = value.number",
    ] {
        assert!(
            check(&format!("{source}\n {body}\n fun main() {{}}")).is_err(),
            "{body}"
        );
    }
}

#[test]
fn m57_block_member_must_implement_its_own_target() {
    let errors = check(
        r#"
        interface Read { public fun read(): Int = 1 }
        interface Mark {}
        struct Invalid<T>(val value: T): Read {
            override interface Mark where T : Read {
                public override fun read(): Int = 2
            }
        }
        fun main() {}
    "#,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("implementation's target")),
        "{errors:#?}"
    );
}

#[test]
fn m57_concrete_relations_do_not_read_partially_built_interface_tables() {
    let output = check(
        r#"
        interface Read { public fun read(): Int = 9 }
        open class Gate<T> { override interface Read where T : Read {} }
        class X : Gate<X>(), Read
        fun main() {
            val base: Gate<X> = X()
            val value: Read = base
            val result = value.read()
        }
    "#,
    )
    .unwrap();
    let gate = output
        .local
        .classes
        .iter()
        .find(|(_, class)| class.name == "Gate")
        .unwrap()
        .1;
    assert_eq!(gate.interfaces.len(), 1);
    assert_eq!(gate.interface_implementations.len(), 1);
}
