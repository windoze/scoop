use super::*;

#[test]
fn specialization_rejects_newly_equal_reference_and_array_keys() {
    rejects(
        r#"
        context(a: A, b: B)
        fun <A: ref, B: ref> read() {}
        fun use() { read<String, String>() }
        "#,
        "duplicate context key after specialization",
    );
    rejects(
        r#"
        context(a: Array<A>, b: Array<B>)
        fun <A, B> read() {}
        fun use() { read<Int, Int>() }
        "#,
        "duplicate context key after specialization",
    );
}

#[test]
fn an_owner_type_alone_checks_abstract_and_unused_context_contracts() {
    for declaration in [
        "interface Reader<A: ref, B: ref> { context(a: A, b: B) fun read(): Int }",
        "abstract class Reader<A: ref, B: ref> { context(a: A, b: B) abstract fun read(): Int }",
        "class Reader<A: ref, B: ref> { context(a: A, b: B) fun read(): Int = 1 }",
        "struct Reader<A: ref, B: ref>(val id: Int) { context(a: A, b: B) fun read(): Int = id }",
        "enum Reader<A: ref, B: ref> { Ready\n context(a: A, b: B) fun read(): Int = 1 }",
    ] {
        rejects(
            &format!("{declaration}\nfun use(reader: Reader<String, String>) {{}}"),
            "duplicate context key after specialization",
        );
    }
}

#[test]
fn distinct_concrete_keys_remain_valid_after_owner_substitution() {
    with_source(
        r#"
        interface Reader<A: ref, B: ref> { context(a: A, b: B) fun read(): Int }
        class Implementation<A: ref, B: ref> : Reader<A, B> {
            context(a: A, b: B)
            override fun read(): Int = 1
        }
        fun use(reader: Reader<String, Any>) {}
        fun create(): Implementation<String, Any> = Implementation<String, Any>()
        "#,
        |_| {},
    );
}
