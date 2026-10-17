use super::*;

#[test]
fn generic_codecs_keep_definition_bindings_when_actual_types_coincide() {
    let output = compile(r#"
        public struct Pair<A, B>(val first: A, val second: B)
        public class PairEncoder<A, B>(private val first: ValueEncoder<A>, private val second: ValueEncoder<B>) : ValueEncoder<Pair<A, B>>
        public struct Nested<T>(val pair: (T, Leaf), val optional: Option<T>)
        public struct NestedEncoder<T>(val element: ValueEncoder<T>) : ValueEncoder<Nested<T>>
        public fun use(encoder: Encoder) {
            val pair = Pair<Leaf, Leaf>(Leaf(), Leaf())
            PairEncoder<Leaf, Leaf>(Leaf.Companion, Leaf.Companion).encode(pair, encoder)
            val nested = Nested<Leaf>((Leaf(), Leaf()), Some(Leaf()))
            NestedEncoder<Leaf>(Leaf.Companion).encode(nested, encoder)
        }
    "#).unwrap();
    assert!(
        !output.export.lambdas.is_empty(),
        "tuple composition uses a normal closure"
    );
}

#[test]
fn only_unique_primary_val_dependencies_supply_field_codecs() {
    rejects(
        r#"
        public struct Box<T>(val value: T)
        public class Writer<T>(private val first: ValueEncoder<T>, private val second: ValueEncoder<T>) : ValueEncoder<Box<T>>
    "#,
        "ambiguous primary val dependencies",
    );
    rejects(
        r#"
        public struct Box<T>(val value: T)
        public class Writer<T>(private var element: ValueEncoder<T>) : ValueEncoder<Box<T>>
    "#,
        "has no ValueEncoder<T>",
    );
    rejects(
        r#"
        public struct Box<T>(val value: T)
        public class Writer<T>(element: ValueEncoder<T>) : ValueEncoder<Box<T>> {
            private val saved: ValueEncoder<T> = element
        }
    "#,
        "has no ValueEncoder<T>",
    );
    rejects(
        "public class Writer<T> : ValueEncoder<T>",
        "target T requires an explicit implementation",
    );
}

#[test]
fn ordinary_combined_protocol_derives_both_directions_independently() {
    compile(
        r#"
        public interface Both<T> : ValueEncoder<T>, ValueDecoder<T> {}
        public struct Box<T>(val value: T)
        public class BoxCodec<T>(private val element: Both<T>) : Both<Box<T>>
        public struct Value(val value: Long) { public companion object : Both<Value> }
        public struct DecodeOnly(val value: Leaf) {
            public companion object : ValueDecoder<DecodeOnly> {
                public override fun decode(decoder: Decoder): DecodeOnly = DecodeOnly(Leaf())
            }
        }
    "#,
    )
    .unwrap();
}

#[test]
fn recursive_fields_use_the_current_codec_through_container_and_tuple_values() {
    compile(r#"
        public class Node(public val value: Leaf, public val children: Array<(Leaf, Option<Node>)>) {
            public companion object : ValueEncoder<Node>
        }
        public object PairWriter : ValueEncoder<(Leaf, Unit)>
    "#).unwrap();
}
