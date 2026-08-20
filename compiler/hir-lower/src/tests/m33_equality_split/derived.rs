use super::*;

#[test]
fn ordinary_member_calls_can_use_structural_comparison() {
    lower_user_output(
        scoop_parser::parse(
            r#"
        struct Box<T>(val value: T)
        struct Point(val x: Int) : Equality<Point> {
            public override fun equalTo(other: Point): Boolean = this.equals(other)
        }
        fun main() {
            val box = Box(1).equals(Box(1))
            val tuple = (1, false).equals((1, false))
            val point = Point(1).equalTo(Point(1))
        }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn explicit_operator_interface_uses_its_declared_implementation() {
    lower_user_output(
        scoop_parser::parse(
            r#"
        interface EqualOperator<T> {
            public operator fun equals(other: T): Boolean
        }
        struct Point(val x: Int) : EqualOperator<Point> {
            public override operator fun equals(other: Point): Boolean = x == other.x
        }
        fun <T : EqualOperator<T>> same(left: T, right: T): Boolean = left == right
        fun main() { val result = same(Point(1), Point(1)) }
    "#,
        )
        .unwrap(),
    )
    .unwrap();
}
