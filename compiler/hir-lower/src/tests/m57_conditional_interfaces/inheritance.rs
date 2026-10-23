use super::*;

const BASE: &str = r#"
interface Read { public fun read(): Int }
struct Good() : Read { public override fun read(): Int = 7 }
struct Empty()
open class Base<T>(val value: T) {
    override interface Read where T : Read {
        public open override fun read(): Int = value.read()
    }
}
"#;

#[test]
fn m57_inherited_interfaces_keep_the_original_guard_and_owner() {
    let output = check(&format!(
        r#"{BASE}
class Derived<T>(value: T) : Base<T>(value)
class Replaced<T>(value: T) : Base<T>(value) {{
    public override fun read(): Int = 13
}}
fun main() {{
    val a: Read = Derived(Good())
    val b = Derived(Empty())
    val c: Read = Replaced(Good())
    val d = Replaced(Empty()).read()
}}
"#
    ))
    .unwrap();
    let base = output
        .export
        .classes
        .values()
        .find(|class| class.name == "Base")
        .unwrap();
    let original = base.conditional_interfaces[0].id();
    for name in ["Derived", "Replaced"] {
        let class = output
            .export
            .classes
            .values()
            .find(|class| class.name == name)
            .unwrap();
        assert_eq!(class.interface_implementations.len(), 1, "{name}");
        assert_eq!(
            class.interface_implementations[0]
                .condition
                .as_ref()
                .unwrap()
                .declaration,
            original
        );
    }
}

#[test]
fn m57_equivalent_relisting_and_constructed_base_arguments_are_ordinary_overrides() {
    check(&format!(
        r#"{BASE}
class Relisted<T>(value: T) : Base<T>(value) {{
    override interface Read where T : Read {{ public override fun read(): Int = 11 }}
}}
class Restricted<T : Read>(value: T) : Base<T>(value) {{
    override interface Read {{ public override fun read(): Int = 12 }}
}}
struct Wrap<T>(val value: T) {{
    override interface Read where T : Read {{ public override fun read(): Int = value.read() }}
}}
class Nested<T>(value: Wrap<T>) : Base<Wrap<T>>(value) {{
    public override fun read(): Int = 13
}}
fun main() {{
    val a: Read = Relisted(Good())
    val b = Relisted(Empty())
    val c: Read = Restricted(Good())
    val d: Read = Nested(Wrap(Good()))
    val e = Nested(Wrap(Empty())).read()
}}
"#
    ))
    .unwrap();
}

#[test]
fn m57_conditional_abstract_obligations_cover_the_entire_derived_domain() {
    check(
        r#"
interface Read { public fun read(): Int }
struct Empty()
abstract class Base<T> {
    override interface Read where T : Read { public abstract override fun read(): Int }
}
class Complete<T> : Base<T>() { public override fun read(): Int = 9 }
class NoObligation : Base<Empty>()
abstract class Implicit<T> { override interface Read where T : Read {} }
class Implemented<T> : Implicit<T>() { public override fun read(): Int = 10 }
fun main() { val a = Complete<Empty>().read(); val b = NoObligation() }
"#,
    )
    .unwrap();
    let errors = check(
        r#"
interface Read { public fun read(): Int }
abstract class Base<T> {
    override interface Read where T : Read { public abstract override fun read(): Int }
}
class Missing<T> : Base<T>()
fun main() {}
"#,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("abstract") && error.message.contains("read")),
        "{errors:#?}"
    );
}

#[test]
fn m57_overrides_cannot_narrow_the_inherited_condition() {
    let Err(errors) = check(&format!(
        r#"{BASE}
class Narrow<T>(value: T) : Base<T>(value) {{
    override interface Read where T : Read, T : Hash {{
        public override fun read(): Int = 9
    }}
}}
fun main() {{}}
"#
    )) else {
        panic!("a derived override must not narrow the inherited condition");
    };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("condition")),
        "{errors:#?}"
    );
}

#[test]
fn m57_overrides_in_other_blocks_still_cover_the_base_slot() {
    for (base, derived) in [
        (BASE.to_owned(), "class Narrow<T>(value: T) : Base<T>(value) { override interface Other where T : Hash { public override fun read(): Int = 9 } }"),
        ("interface Read { public fun read(): Int }\nopen class Base<T> : Read { public open override fun read(): Int = 1 }".to_owned(), "class Narrow<T> : Base<T>() { override interface Other where T : Hash { public override fun read(): Int = 9 } }"),
    ] {
        let source = format!("{base}\ninterface Other {{ public fun read(): Int }}\n{derived}\nfun main() {{}}");
        let Err(errors) = check(&source) else { panic!("a block cannot narrow another inherited slot") };
        assert!(errors.iter().any(|error| error.message.contains("entire inherited method condition")), "{errors:#?}");
    }
}

#[test]
fn m57_conditional_override_keeps_the_exact_return_type_contract() {
    let Err(errors) = check(
        r#"
interface Read { public fun read(): Int }
interface Extract { public fun get(): Read }
struct Good() : Read { public override fun read(): Int = 7 }
struct Empty()
open class Base<T>(val value: T) {
    override interface Extract where T : Read {
        public open override fun get(): Read = value
    }
}
class Derived<T>(val own: T) : Base<T>(own) { public override fun get(): T = own }
fun main() { val a: Extract = Derived(Good()); val b = Derived(Empty()).get() }
"#,
    ) else {
        panic!("conditional interfaces do not add covariant override results");
    };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("does not override")),
        "{errors:#?}"
    );
}
