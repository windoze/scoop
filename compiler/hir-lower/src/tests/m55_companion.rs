use super::*;

fn check(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
}

#[test]
fn m55_companion_bound_body_signature_default_and_forwarding() {
    check(
        r#"
        interface Encoder<T> { fun encode(value: T): Int }
        struct Box(val value: Int) {
            public companion object Factory : Encoder<Box> {
                public override fun encode(value: Box): Int = value.value
            }
        }
        fun <T> encode(value: T, encoder: T.Companion = T.Companion): Int
            where T.Companion : Encoder<T> = encoder.encode(value)
        fun <T> forward(value: T): Int where T.Companion : Encoder<T> = encode(value)
        fun <T> short(value: T): Int where T.Companion : Encoder<T> = T.encode(value)
        fun <T> projection(value: T): T.Companion where T.Companion : ref = T.Companion
        fun main() {
            val b = Box(2)
            val a = encode(b)
            val c = forward(b)
            val d = short(b)
            val e = projection(b)
        }
    "#,
    )
    .unwrap();
}

#[test]
fn m55_companion_bound_checks_existence_and_has_no_inverse_inference() {
    for (source, message) in [
        (
            "fun <T> probe(value: T): Int where T.Companion : ref = 1\nfun main() { probe((1, 2)) }",
            "no public companion",
        ),
        (
            "fun <T> probe(value: T.Companion): Int where T.Companion : ref = 1\nstruct Box() { public companion object {} }\nfun main() { probe(Box.Companion) }",
            "cannot infer",
        ),
        (
            "fun <T> probe(value: T): Int where T.Companion : value = 1",
            "reference singleton",
        ),
        (
            "fun <T> probe(value: T.Companion): Int = 1",
            "does not require a public companion",
        ),
    ] {
        let errors = check(source).unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains(message)),
            "{source}\n{errors:#?}"
        );
    }
}

#[test]
fn m55_companion_bounds_can_infer_other_parameters_after_the_host() {
    check(
        r#"
        interface Marker<T> { fun make(): T }
        struct Host() { public companion object : Marker<Int> { public override fun make(): Int = 7 } }
        fun <T, U> choose(value: T): U where T.Companion : Marker<U> = T.make()
        fun main() { val value = choose(Host()) }
    "#,
    )
    .unwrap();
}
