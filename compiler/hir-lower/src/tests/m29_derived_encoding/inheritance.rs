use super::*;

const DATA: &str = r#"
    public open class Base public constructor(public val id: Long) {
        public companion object : Encodable<Base> {
            public override fun encode(value: Base, encoder: Encoder) { Long.encode(value.id, encoder) }
        }
    }
    public class Child public constructor(id: Long, public val extra: Leaf) : Base(id)
"#;

#[test]
fn a_child_field_does_not_fall_back_to_the_base_companion() {
    rejects(
        &format!(
            r#"
        {DATA}
        public struct Holder(val child: Child) {{ public companion object : Encodable<Holder> }}
    "#
        ),
        "has no Encodable<Child>",
    );
}

#[test]
fn explicit_base_codec_accepts_the_ordinary_upcast_without_data_conformance() {
    compile(&format!(
        r#"
        {DATA}
        public fun use(encoder: Encoder) {{
            val child = Child(7L, Leaf())
            Base.Companion.encode(child, encoder)
            val erased: Any = child
            if (erased is Encodable<Base>) {{}}
        }}
    "#
    ))
    .unwrap();
    rejects(
        &format!(
            r#"
        {DATA}
        public fun use(): Encodable<Child> = Base.Companion
    "#
        ),
        "is invariant",
    );
}

#[test]
fn a_companion_can_read_private_storage_and_an_external_codec_cannot() {
    compile(
        r#"
        public class Value(private val value: Leaf) {
            public companion object : Encodable<Value>
        }
    "#,
    )
    .unwrap();
    rejects(
        r#"
        public class Value(private val value: Leaf)
        public object External : Encodable<Value>
    "#,
        "cannot read property `value` from this codec",
    );
}

#[test]
fn an_unrelated_valid_override_does_not_block_derivation() {
    compile(
        r#"
        public interface Other {
            public fun encode(value: Leaf): Leaf
        }
        public struct Item(val value: Leaf) {
            public companion object : Other, Encodable<Item> {
                public override fun encode(value: Leaf): Leaf = value
            }
        }
    "#,
    )
    .unwrap();
}
