use super::*;

mod derived;
mod derived_binding;

#[test]
fn explicit_core_bound_and_operator_select_the_same_member() {
    let output = lower_user_output(
        scoop_parser::parse(
            r#"
        class OperatorOnly() {
            public operator fun equals(other: OperatorOnly): Boolean = false
        }
        class Value() : Equals<Value> {
            public override operator fun equals(other: Value): Boolean = true
        }
        fun <T : Equals<T>> same(left: T, right: T): Boolean = left == right
        fun main() {
            val operator = OperatorOnly() == OperatorOnly()
            val value = Value()
            val direct = value == value
            val ordinary = value.equals(value)
            val bound = same(value, value)
            val view: Equals<Value> = value
            val dynamic = view == value
        }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
    let dump = hir::dump(&output.export);
    for target in [
        "MethodCall OperatorOnly.equals",
        "MethodCall Value.equals",
        "via Equals<T0> -> Equals.equals",
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
fn non_operator_bound_does_not_supply_an_operator() {
    let source = r#"
        interface Ordinary<T> { public fun equals(other: T): Boolean }
        fun <T : Ordinary<T>> same(left: T, right: T): Boolean = left == right
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
        "struct Point(val x: Int)\nfun main() { val value: Equals<Point> = Point(1) }",
        "fun main() { val value: Equals<(Int, Boolean)> = (1, false) }",
        "struct Value(val number: Int) : Equals<Value>\nfun main() {}",
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
