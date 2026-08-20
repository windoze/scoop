use super::*;

mod derived;
mod derived_binding;

#[test]
fn independent_operator_and_library_equality_select_their_own_members() {
    let output = lower_user_output(
        scoop_parser::parse(
            r#"
        class OperatorOnly() {
            public operator fun equals(other: OperatorOnly): Boolean = false
        }
        class LibraryOnly() : Equality<LibraryOnly> {
            public override fun equalTo(other: LibraryOnly): Boolean = true
        }
        class Both() : Equality<Both> {
            public operator fun equals(other: Both): Boolean = false
            public override fun equalTo(other: Both): Boolean = true
        }
        fun <T : Equality<T>> library(left: T, right: T): Boolean = left.equalTo(right)
        fun main() {
            val operator = OperatorOnly() == OperatorOnly()
            val library = library(LibraryOnly(), LibraryOnly())
            val both = Both()
            val direct = both == both
            val ordinary = both.equalTo(both)
            val bound = library(both, both)
        }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
    let dump = hir::dump(&output.export);
    for target in [
        "MethodCall OperatorOnly.equals",
        "MethodCall Both.equals",
        "MethodCall Both.equalTo",
        "via Equality<T0> -> Equality.equalTo",
    ] {
        assert!(dump.contains(target), "missing {target}: {dump}");
    }
}

#[test]
fn user_operator_bound_does_not_require_core_equality() {
    let output = lower_user_output(
        scoop_parser::parse(
            r#"
        interface EqualOperator<T> {
            public operator fun equals(other: T): Boolean
        }
        class Value() : EqualOperator<Value> {
            public override operator fun equals(other: Value): Boolean = true
        }
        fun <T : EqualOperator<T>> same(left: T, right: T): Boolean = left == right
        fun main() { val result = same(Value(), Value()) }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
    let dump = hir::dump(&output.export);
    assert!(
        dump.contains("via EqualOperator<T0> -> EqualOperator.equals"),
        "{dump}"
    );
}

#[test]
fn equality_bound_alone_does_not_supply_an_operator() {
    let source = r#"
        fun <T : Equality<T>> same(left: T, right: T): Boolean = left == right
        fun main() {}
    "#;
    let errors = lower_user(scoop_parser::parse(source).unwrap()).unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("has no applicable member operator `equals`")),
        "{errors:?}"
    );
}

#[test]
fn structural_comparison_does_not_add_library_equality() {
    let output = lower_user_output(
        scoop_parser::parse(
            r#"
        struct Point(val x: Int)
        enum Choice { Item(Int), Empty }
        fun main() {
            val point = Point(1) == Point(1)
            val choice = Choice.Item(1) == Choice.Item(1)
            val tuple = (1, false) == (1, false)
            val unit = Unit == Unit
        }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
    for name in ["Point", "Choice", "(Int, Boolean)"] {
        assert!(
            output
                .export
                .derived_equality_applications
                .iter()
                .any(|(_, application)| {
                    hir::type_name(&output.export, application.owner_ty) == name
                }),
            "missing comparison for {name}"
        );
    }

    for source in [
        "struct Point(val x: Int)\nfun main() { val value: Equality<Point> = Point(1) }",
        "fun main() { val value: Equality<(Int, Boolean)> = (1, false) }",
        "class Value() : Equality<Value> { public operator fun equals(other: Value): Boolean = true }\nfun main() {}",
    ] {
        let errors = lower_user(scoop_parser::parse(source).unwrap()).unwrap_err();
        assert!(!errors.is_empty());
    }
}

#[test]
fn construction_and_boxing_do_not_materialize_structural_comparison() {
    let output = lower_user_output(
        scoop_parser::parse(
            r#"
        class Opaque()
        struct Box<T>(val value: T)
        enum Choice<T> { Item(T), Empty }
        fun main() {
            val box: Any = Box(Opaque())
            val choice: Any = Choice<Opaque>.Item(Opaque())
            val tuple: Any = (Opaque(), Opaque())
        }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        output
            .export
            .derived_equality_applications
            .iter()
            .all(|(_, application)| {
                !hir::type_name(&output.export, application.owner_ty).contains("Opaque")
            })
    );
}

#[test]
fn phantom_parameters_do_not_restrict_structural_comparison() {
    lower_user_output(
        scoop_parser::parse(
            r#"
        class Opaque()
        struct Phantom<T>()
        fun main() { val result = Phantom<Opaque>() == Phantom<Opaque>() }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
}
