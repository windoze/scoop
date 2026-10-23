use super::*;

#[test]
fn m57_projection_targets_and_members_are_substituted_only_in_their_domain() {
    check(
        r#"
interface Value<X> { public fun get(): X }
struct Present() { public companion object {} }
struct Absent()
open class Base<T> {
    override interface Value<T.Companion> where T.Companion : ref {
        public open override fun get(): T.Companion = T.Companion
    }
}
class Child<T> : Base<T>()
class NoCompanion : Base<Absent>()
class Ordinary : Base<Absent>() { public fun get(): Int = 3 }
fun main() {
    val child: Value<Present.Companion> = Child<Present>()
    val value = child.get()
    val absent = NoCompanion()
    val ordinary = Ordinary().get()
}
"#,
    )
    .unwrap();
}

#[test]
fn m57_projection_existence_precedes_bounds_on_other_parameters() {
    check(
        r#"
interface Needs<X> {}
interface Mark {}
struct Absent()
open class Base<A, B> {
    override interface Mark where A : Needs<B.Companion>, B.Companion : ref {}
}
class NoCompanion<A> : Base<A, Absent>()
fun main() { val absent = NoCompanion<Int>() }
"#,
    )
    .unwrap();
}

#[test]
fn m57_unavailable_projection_members_report_the_missing_condition() {
    for body in [
        "fun invalid(value: Base<Absent>): Any = value.get()",
        "fun <T> invalid(value: Base<T>): Any = value.get()",
    ] {
        let errors = check(&format!(
            r#"
interface Value<X> {{ public fun get(): X }}
struct Absent()
open class Base<T> {{
    override interface Value<T.Companion> where T.Companion : ref {{
        public open override fun get(): T.Companion = T.Companion
    }}
}}
{body}
fun main() {{}}
"#,
        ))
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("companion")),
            "{errors:#?}"
        );
    }
}
