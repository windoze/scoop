use super::*;

#[test]
fn recursive_bounds_use_complete_nominal_relationships_and_parameter_scopes() {
    lower_user_output(
        scoop_parser::parse(
            r#"
        interface Step<T : Step<T>> { public fun next(): T }
        struct Early(val value: Step<Later>)
        struct Later(val value: Int) : Step<Later> {
            public override fun next(): Later = Later(value + 1)
        }
        class Box<T : Step<T>>(val value: T) : Step<Box<T>> {
            public override fun next(): Box<T> = Box(value.next())
        }
        fun <T : Step<T>> advance(value: T): T = value.next()
        fun main() {
            val value = advance(Later(1))
            val box = advance(Box(value))
            struct Local(val value: Int) : Step<Local> {
                public override fun next(): Local = Local(value + 1)
            }
            val local = advance(Local(1))
        }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn deferred_nominal_bounds_still_reject_missing_conformance() {
    let errors = lower_user_output(
        scoop_parser::parse(
            r#"
        interface Step<T : Step<T>> { public fun next(): T }
        struct Other(val value: Int)
        class Invalid() : Step<Other> {
            public override fun next(): Other = Other(1)
        }
        fun main() {}
    "#,
        )
        .unwrap(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| error.message.contains(
        "type argument `Other` for `T` of interface `Step` must satisfy interface upper bound `Step<Other>`"
    )), "{errors:?}");
}
