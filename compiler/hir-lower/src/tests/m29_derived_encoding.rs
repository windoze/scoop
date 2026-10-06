use super::*;

mod dependencies;
mod inheritance;

const LEAF: &str = r#"
    public struct Leaf() {
        public companion object : Encodable<Leaf> {
            public override fun encode(value: Leaf, encoder: Encoder) {
                encoder.singleValue().writeLong(7L)
            }
        }
    }
"#;

fn compile(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower_with_sysroot(&format!("{LEAF}\n{source}\nfun main() {{}}"))
}

fn rejects(source: &str, message: &str) {
    let errors = compile(source).unwrap_err();
    assert!(
        errors.iter().any(|error| error.message.contains(message)),
        "{errors:?}"
    );
}

#[test]
fn nominal_encoding_becomes_complete_ordinary_codec_methods() {
    let output = compile(r#"
        public struct Box<T>(val value: T)
        public class BoxEncoder<T>(private val element: Encodable<T>) : Encodable<Box<T>>
        public struct Envelope(val value: Box<Leaf>)
        public class EnvelopeEncoder(private val box: Encodable<Box<Leaf>>) : Encodable<Envelope>
        public enum Choice {
            Empty,
            Pair(Leaf, Leaf),
            Named(val value: Leaf),
            Record { item: Leaf }
            public companion object : Encodable<Choice>
        }
        public class Row public constructor(public val value: Leaf) {
            public var state: Leaf = value
            public val computed: Any get() = value
            public companion object : Encodable<Row>
        }
        public fun <T> send(value: T, codec: Encodable<T>, encoder: Encoder) {
            codec.encode(value, encoder)
        }
        public fun use(encoder: Encoder) {
            send(Envelope(Box<Leaf>(Leaf())), EnvelopeEncoder(BoxEncoder<Leaf>(Leaf.Companion)), encoder)
            send(Choice.Pair(Leaf(), Leaf()), Choice.Companion, encoder)
            send(Row(Leaf()), Row.Companion, encoder)
        }
    "#).unwrap();
    for name in [
        "BoxEncoder.encode",
        "EnvelopeEncoder.encode",
        "Choice.Companion.encode",
        "Row.Companion.encode",
    ] {
        let methods = output
            .export
            .functions
            .values()
            .filter(|function| function.name == name)
            .collect::<Vec<_>>();
        assert_eq!(methods.len(), 1, "one ordinary method for {name}");
        let function = methods[0];
        assert_eq!(
            function.params.len(),
            3,
            "codec, value and encoder for {name}"
        );
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("{name} must be an ordinary body")
        };
        assert!(!body.statements.is_empty(), "{name} has a complete body");
    }
}

#[test]
fn explicit_inherited_and_default_encoding_wins_over_synthesis() {
    compile(r#"
        public struct Explicit(val opaque: Any) {
            public companion object : Encodable<Explicit> {
                public override fun encode(value: Explicit, encoder: Encoder) { encoder.singleValue().writeNull() }
            }
        }
        public interface Default : Encodable<Any> {
            public override fun encode(value: Any, encoder: Encoder) { encoder.singleValue().writeNull() }
        }
        public object UsesDefault : Default
        public open class Base public constructor() : Encodable<Any> {
            public override fun encode(value: Any, encoder: Encoder) { encoder.singleValue().writeNull() }
        }
        public class Child public constructor() : Base()
    "#).unwrap();
}

#[test]
fn unrelated_overload_does_not_block_encoding_synthesis() {
    let output = compile(
        r#"
        public struct Item(val value: Leaf) {
            public companion object : Encodable<Item> {
                public fun encode(value: Leaf): Leaf = value
                public fun another(encoder: Encoder) {}
            }
        }
    "#,
    )
    .unwrap();
    assert_eq!(
        output
            .export
            .functions
            .values()
            .filter(|function| function.name == "Item.Companion.encode")
            .count(),
        2
    );
}

#[test]
fn encoding_requires_a_codec_for_the_declared_field_type() {
    for source in [
        "public struct Item<T>(val value: T) { public companion object : Encodable<Item<T>> }",
        "public struct Item(val value: Any) { public companion object : Encodable<Item> }",
    ] {
        rejects(source, "automatic encode has no Encodable<");
    }
}

#[test]
fn encoding_annotations_only_affect_participating_fields() {
    compile(
        r#"
        public struct Item(@SerialName("wire") val value: Leaf, @Transient val opaque: Any) {
            public companion object : Encodable<Item>
        }
    "#,
    )
    .unwrap();
    rejects(
        r#"
        public struct Item(@SerialName("same") val first: Leaf, @SerialName("same") val second: Leaf) {
            public companion object : Encodable<Item>
        }
    "#,
        "automatic encode has duplicate field name `same`",
    );
}

#[test]
fn invalid_explicit_encoding_keeps_ordinary_override_diagnostics() {
    for implementation in [
        "public override fun encode(value: Item, encoder: Encoder): Long = 1L",
        "public override fun encode(encoder: Encoder) {}",
    ] {
        let errors = compile(&format!(
            "public struct Item(val value: Any) {{ public companion object : Encodable<Item> {{ {implementation} }} }}"
        ))
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("override"))
        );
        assert!(
            errors
                .iter()
                .all(|error| !error.message.contains("automatic encode")),
            "{errors:?}"
        );
    }
}

#[test]
fn custom_stored_accessors_require_an_explicit_codec() {
    rejects(
        r#"
        public class Item public constructor() {
            public val value: Leaf = Leaf()
                get() = field
            public companion object : Encodable<Item>
        }
    "#,
        "stored property `value` has a custom accessor",
    );
}

#[test]
fn a_user_interface_named_encodable_does_not_request_synthesis() {
    rejects(
        r#"
        public interface Encodable<T> { public fun encode(value: T, encoder: Encoder) }
        public object Item : Encodable<Leaf>
    "#,
        "does not implement interface method `Encodable<Leaf>.encode`",
    );
}
