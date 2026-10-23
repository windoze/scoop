use super::*;

const DEFAULTS: &str = r#"
interface Root { public fun read(): Int }
interface Left : Root { public override fun read(): Int = 1 }
interface Right : Root { public override fun read(): Int = 2 }
interface First {}
interface Second {}
struct Both() : First, Second
struct Neither()
"#;

#[test]
fn m57_possible_default_conflicts_are_rejected_without_an_instantiation() {
    for body in [
        "override interface Left where T : First {}\n override interface Right where T : Second {}",
        "override interface Right where T : Second {}\n override interface Left where T : First {}",
    ] {
        let errors = check(&format!(
            "{DEFAULTS}\nstruct Select<T>(val value:T) {{ {body} }}\nfun main() {{}}"
        ))
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("conditions both hold")),
            "{errors:#?}"
        );
    }
}

#[test]
fn m57_a_shared_override_resolves_possible_defaults_for_every_domain() {
    check(&format!(
        r#"{DEFAULTS}
struct Shared<T>(val value:T) {{
    public override fun read(): Int = 3
    override interface Left where T : First {{}}
    override interface Right where T : Second {{}}
}}
fun main() {{ val both: Root = Shared(Both()); val plain = Shared(Neither()).read() }}
"#
    ))
    .unwrap();
    let errors = check(&format!(
        r#"{DEFAULTS}
struct Partial<T>(val value:T) {{
    override interface Left where T : First {{ public override fun read(): Int = 3 }}
    override interface Right where T : Second {{}}
}}
fun main() {{}}
"#
    ))
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("covers both conditions")),
        "{errors:#?}"
    );
}

#[test]
fn m57_subinterface_defaults_and_disjoint_domains_keep_ordinary_rules() {
    check(&format!(
        r#"{DEFAULTS}
interface More : Left {{ public override fun read(): Int = 4 }}
struct Ordered<T>(val value: T) {{
    override interface Left where T : First {{}}
    override interface More where T : Second {{}}
}}
struct Reversed<T>(val value: T) {{
    override interface More where T : Second {{}}
    override interface Left where T : First {{}}
}}
struct Disjoint<T>(val value: T) {{
    override interface Left where T : value {{}}
    override interface Right where T : ref {{}}
}}
open class One
open class Two
struct Separate<T>(val value: T) {{
    override interface Left where T : One {{}}
    override interface Right where T : Two {{}}
}}
fun <T : First> fromFirst(value: Ordered<T>): Int = value.read()
fun main() {{
    val a: Root = Ordered(Both())
    val b: Root = Reversed(Both())
    val c: Root = Disjoint(1)
    val d = Ordered(Both()).read()
    val e = fromFirst(Ordered(Both()))
}}
"#
    ))
    .unwrap();
}

#[test]
fn m57_default_members_require_their_implementation_condition() {
    for parameter in ["T", "Neither"] {
        let errors = check(&format!(
            r#"{DEFAULTS}
struct Select<T>(val value: T) {{ override interface Left where T : First {{}} }}
fun <T> invalid(value: Select<{parameter}>): Int = value.read()
fun main() {{}}
"#
        ))
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains("read")),
            "{errors:#?}"
        );
    }
}
