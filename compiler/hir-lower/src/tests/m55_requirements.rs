use super::*;

fn check(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
}

#[test]
fn m55_requirement_declarations_normalize_virtual_names_and_block_order() {
    let errors = check(r#"
        fun <T> use(x: T): Int where requires(a: T) { a.read(): Int; a.check(x: Int): Boolean } = 0
        fun <U> use(x: U): Int where requires(y: U) { y.check(z: Int): Boolean }, requires(z: U) { z.read(): Int } = 1
    "#).unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("already declared with the same signature")),
        "{errors:#?}"
    );
}

#[test]
fn m55_requirement_declarations_reject_invalid_subjects_effects_and_duplicates() {
    for (source, expected) in [
        (
            "fun use(): Int where requires(x: Int) { x.read(): Int } = 0",
            "generic functions",
        ),
        (
            "fun <T> use(x: T): Int where requires(y: Int) { y.read(): Int } = 0",
            "type parameter",
        ),
        (
            "fun <T> use(x: T): Int where requires(y: T) { y.read(): Int; y.read(): Int } = 0",
            "duplicate member requirement",
        ),
        (
            "fun <T> use(x: T): Int where requires { suspend T() } = 0",
            "construction",
        ),
        (
            "fun <T> use(x: T): Int where requires(y: T) { @NoGC suspend y.read(): Int } = 0",
            "NoGC",
        ),
        (
            "fun <T> use(x: T): Int where requires(y: T) { @Deprecated y.read(): Int } = 0",
            "annotation",
        ),
        (
            "fun <T> use(x: T): Int where requires { T(): Int } = 0",
            "result",
        ),
    ] {
        let errors = check(source).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{source}\n{errors:#?}"
        );
    }
}

#[test]
fn m55_requirement_predicates_use_public_members_defaults_and_exact_forwarding() {
    check(r#"
        public struct Box(val value: Int) {
            public fun <U> read(value: U, count: Int = 1): Int = count
            @NoGC public fun marker(): Int = 2
        }
        fun <T> accept(value: T): Int where requires(x: T) { x.read<Int>(value = y: Int): Int } = 1
        fun <T> forward(value: T): Int where requires(z: T) { z.read<Int>(value = n: Int): Int } = accept(value)
        fun <T> pure(value: T): Int where requires(x: T) { @NoGC x.marker(): Int } = 3
        fun <T> construct(value: T): Int where requires { T(value = n: Int) } = 4
        fun main() { val x = Box(7); forward(x); pure(x); construct(x) }
    "#).unwrap();
}

#[test]
fn m55_requirement_predicates_do_not_reselect_by_result_or_accept_extensions() {
    for (members, required, expected) in [
        (
            "public fun read(x: Int): String = \"x\"; public fun read(x: Any): Int = 1",
            "x.read(n: Int): Int",
            "returns `String`",
        ),
        (
            "internal fun read(x: Int): Int = 1",
            "x.read(n: Int): Int",
            "public member",
        ),
        (
            "public fun read(): Int = 1",
            "@NoGC x.read(): Int",
            "effects",
        ),
        (
            "@Unsafe public fun read(): Int = 1",
            "x.read(): Int",
            "effects",
        ),
    ] {
        let source = format!(
            "public struct Box() {{ {members} }}\nfun <T> accept(value: T): Int where requires(x: T) {{ {required} }} = 1\nfun main() {{ accept(Box()) }}"
        );
        let errors = check(&source).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{source}\n{errors:#?}"
        );
    }
    let errors = check(
        r#"
        public struct Box()
        fun Box.read(): Int = 1
        fun <T> accept(value: T): Int where requires(x: T) { x.read(): Int } = 1
        fun main() { accept(Box()) }
    "#,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("public member")),
        "{errors:#?}"
    );
}

#[test]
fn m55_requirement_bodies_expand_members_constructors_defaults_and_forwarding() {
    check(r#"
        public struct Box(val value: Int) {
            public fun read(count: Int = 1): Int = value + count
            @NoGC public fun marker(): Int = 2
        }
        fun <T> read(value: T): Int where requires(x: T) { x.read(): Int } = value.read()
        fun <T> forward(value: T): Int where requires(z: T) { z.read(): Int } = read(value)
        @NoGC fun <T> pure(value: T): Int where requires(x: T) { @NoGC x.marker(): Int } = value.marker()
        fun <T> construct(n: Int): T where requires { T(value = x: Int) } = T(value = n)
        fun <T> fromDefault(value: T, n: Int = value.read()): Int where requires(x: T) { x.read(): Int } = n
        fun <T> wrapped(value: T): () -> Int where requires(x: T) { x.read(): Int } = { value.read() }
        fun main() { val value = construct<Box>(7); forward(value); pure(value); fromDefault(value); wrapped(value)() }
    "#).unwrap();
}

#[test]
fn m55_requirement_nominal_members_precede_structural_requirements() {
    let errors = check(r#"
        public interface Readable { fun read(): Int }
        @NoGC fun <T: Readable> read(value: T): Int where requires(x: T) { @NoGC x.read(): Int } = value.read()
    "#).unwrap_err();
    assert!(
        errors.iter().any(|error| error.message.contains("NoGC")),
        "{errors:#?}"
    );
}

#[test]
fn m55_requirement_default_plans_keep_closures_and_selected_generic_calls() {
    check(include_str!(
        "../../../../tests/fixtures/language/requirements/default-plans.scoop"
    ))
    .unwrap();
}

#[test]
fn m55_imported_default_closures_keep_original_requirement_binders() {
    super::m23_generic_body_consumption::with_provider_consumer(
        include_str!(
            "../../../../tests/fixtures/language/requirements/cross-cone/provider/src/api.scoop"
        ),
        r#"
        import dev.m55.requirements.m55.requirements.*
        private struct Local(val value: Int) { public fun read(): Int = value }
        private struct Pure(val value: Int) { @NoGC public fun read(): Int = value }
        fun main() {
            defaultClosure(Remote(7))
            defaultClosure(Local(9))
            defaultForward(Remote(7))
            val reference: (Local) -> Int = ::read
            val constructor: (Int) -> Local = ::construct
            reference(constructor(17))
            Api("tag").withDefault(Local(19))
            pure(Pure(21))
        }
        "#,
        |_, _, _, _, _| (),
    )
    .unwrap();
}

#[test]
fn m55_requirement_combinations_keep_references_and_capture_free_local_constructors() {
    check(include_str!(
        "../../../../tests/fixtures/language/requirements/combinations.scoop"
    ))
    .unwrap();
}
