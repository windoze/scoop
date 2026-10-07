use super::super::*;

#[test]
fn data_borrow_intrinsics_reject_incorrect_source_signatures() {
    for (name, declaration) in [
        (
            "array_with_data_pointer",
            "fun <T : value, R> borrow(values: Array<T>, block: (Ptr<T>, Int) -> R): R",
        ),
        (
            "array_with_data_pointer",
            "fun <T : value, R> borrow(values: MutableArray<T>, block: (Ptr<T>, Long) -> R): R",
        ),
        (
            "mutable_array_with_data_pointer",
            "fun <T : value, R> borrow(values: MutableArray<T>, block: suspend (Ptr<T>, Long) -> R): R",
        ),
        (
            "mutable_array_with_data_pointer",
            "fun <T : value, R> borrow(values: MutableArray<T>, block: (Ptr<T>, Long) -> R): Long",
        ),
        (
            "string_with_utf8_bytes",
            "fun <R> borrow(value: String, block: (Ptr<Int>, Long) -> R): R",
        ),
        ("string_with_utf8_bytes", "fun <R> borrow(value: String): R"),
    ] {
        let mut core = core_file();
        let declaration =
            scoop_parser::parse(&format!("@Unsafe @Intrinsic(\"{name}\") {declaration}")).unwrap();
        let Decl::Function(function) = &declaration.declarations[0] else {
            unreachable!("the test parses a function declaration")
        };
        let span = function.span;
        core.declarations.extend(declaration.declarations);
        let user = scoop_parser::parse("fun main() {}").unwrap();
        let errors = lower(&[core, user]).expect_err("a malformed intrinsic must be rejected");
        let message = format!("malformed core data borrow intrinsic `{name}`");
        let error = errors
            .iter()
            .find(|error| error.message == message)
            .unwrap_or_else(|| panic!("missing {message}: {errors:?}"));
        assert_eq!(error.file, 0);
        assert_eq!(error.span, Some(span));
    }
}
