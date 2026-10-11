use super::*;

#[test]
fn qualified_variant_does_not_inherit_a_top_level_constructor_inference_rule() {
    let source = r#"
        enum E<T> { Array(T, T) }
        fun <T> select(first: E<T>, second: E<T>): E<T> = first
        fun main() {
            val typed: E<Int8> = E.Array(1, 2)
            val result = select(E.Array(20, 22), typed)
            val checked: E<Int8> = result
        }
    "#;
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
        .expect("the qualified variant retains its own generic inference shape");
}
