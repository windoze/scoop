use super::wire_equivalence::compare_consumers;
use super::*;

const CONDITIONAL_FAMILY: &str = r#"
    package dependency.api
    public sealed interface Root<T> {}
    public interface Mark {}
    public interface Tagged {}
    public struct Seed<X>() : Mark { public companion object : Mark {} }
    public struct Absent<X>()
    public class Always<T> public constructor() : Root<T>
    public struct Conditional<T>(val value: T) {
        override interface Root<T> where T : Mark {}
        override interface Tagged where T : Mark {}
    }
    public struct Host<T>() {
        public companion object {
            override interface Root<T> where T.Companion : Mark {}
        }
    }
    private class Hidden<T> : Tagged {
        override interface Root<T> where T : ref {}
    }
"#;

#[test]
fn m57_conditional_sealed_families_survive_wire_consumption() {
    compare_consumers(
        "conditional sealed families",
        CONDITIONAL_FAMILY,
        r#"
        import dependency.api.*
        fun <T> classify(value: Root<T>): Int = when (value) {
            is Always<T> -> 1
            is Tagged -> 2
            is Host<T>.Companion -> 3
        }
        fun absent(value: Root<Absent<Int>>): Int = when (value) {
            is Always<Absent<Int>> -> 4
        }
        fun use(): Int = classify(Conditional(Seed<Int>())) + classify(Host<Seed<Int>>.Companion) + absent(Always<Absent<Int>>())
    "#,
    );
}

#[test]
fn m57_imported_sealed_conditions_do_not_allow_omissions_or_foreign_subtypes() {
    for (source, expected) in [
        (
            "import dependency.api.*\nfun <T> invalid(value: Root<T>): Int = when (value) { is Always<T> -> 1 }",
            "non-exhaustive when",
        ),
        (
            "package dependency.api\nimport dependency.api.*\nclass Foreign<T> { override interface Root<T> where T : Mark {} }",
            "same origin Cone and package",
        ),
    ] {
        let errors =
            with_provider_consumer(CONDITIONAL_FAMILY, source, |_, _, _, _, _| ()).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{errors:#?}"
        );
    }
}
