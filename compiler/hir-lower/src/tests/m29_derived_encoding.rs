use super::*;

const LEAF: &str = r#"
    public struct Leaf() : Encodable {
        public override fun encode(encoder: Encoder) {
            encoder.singleValue().writeLong(7L)
        }
    }
"#;

fn compile(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower(&[
        complete_core_file(),
        scoop_parser::parse(include_str!(
            "../../../../sysroot/lib/scoop.core/src/encoding.scoop"
        ))
        .unwrap(),
        scoop_parser::parse(include_str!(
            "../../../../sysroot/lib/scoop.core/src/serialization_annotations.scoop"
        ))
        .unwrap(),
        scoop_parser::parse(&format!("{LEAF}\n{source}\nfun main() {{}}"))
            .expect("the test uses valid syntax"),
    ])
}

#[test]
fn nominal_encoding_becomes_complete_ordinary_methods() {
    let output = compile(
        r#"
        public struct Box<T : Encodable>(val value: T) : Encodable
        public struct Envelope(val value: Box<Leaf>) : Encodable
        public enum Choice : Encodable {
            Empty,
            Pair(Leaf, Leaf),
            Named(val value: Leaf),
            Record { item: Leaf }
        }
        public class Row public constructor(public val value: Leaf) : Encodable {
            public var state: Leaf = value
            public val computed: Any get() = value
        }
        public fun <T : Encodable> send(value: T, encoder: Encoder) {
            value.encode(encoder)
        }
        public fun use(encoder: Encoder) {
            send(Envelope(Box<Leaf>(Leaf())), encoder)
            send(Choice.Pair(Leaf(), Leaf()), encoder)
            send(Row(Leaf()), encoder)
        }
        "#,
    )
    .unwrap();
    for name in [
        "Box.encode",
        "Envelope.encode",
        "Choice.encode",
        "Row.encode",
    ] {
        let methods = output
            .export
            .functions
            .values()
            .filter(|function| function.name == name)
            .collect::<Vec<_>>();
        assert_eq!(methods.len(), 1, "one ordinary method for {name}");
        let function = methods[0];
        assert_eq!(function.params.len(), 2, "receiver and encoder for {name}");
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("{name} must be an ordinary body")
        };
        assert!(!body.statements.is_empty(), "{name} has a complete body");
    }
}

#[test]
fn explicit_inherited_and_default_encoding_wins_over_synthesis() {
    compile(
        r#"
        public struct Explicit(val opaque: Any) : Encodable {
            public override fun encode(encoder: Encoder) {
                encoder.singleValue().writeNull()
            }
        }
        public interface Default : Encodable {
            public override fun encode(encoder: Encoder) {
                encoder.singleValue().writeNull()
            }
        }
        public struct UsesDefault(val opaque: Any) : Default
        public open class Base public constructor() : Encodable {
            public override fun encode(encoder: Encoder) {
                encoder.singleValue().writeNull()
            }
        }
        public class Child public constructor(public val opaque: Any) : Base()
        "#,
    )
    .unwrap();
}

#[test]
fn unrelated_overload_does_not_block_encoding_synthesis() {
    let output = compile(
        r#"
        public struct Item(val value: Leaf) : Encodable {
            public fun encode(value: Leaf): Leaf = value
            public fun another(encoder: Encoder) {}
        }
        "#,
    )
    .unwrap();
    assert_eq!(
        output
            .export
            .functions
            .values()
            .filter(|function| function.name == "Item.encode")
            .count(),
        2
    );
}

#[test]
fn encoding_requires_the_declared_field_capability() {
    for source in [
        "public struct Item<T>(val value: T) : Encodable",
        "public struct Item(val value: Any) : Encodable",
    ] {
        let errors = compile(source).unwrap_err();
        assert!(
            errors.iter().any(|error| error
                .message
                .contains("automatic encode requires field `value`")),
            "{errors:?}"
        );
    }
}

#[test]
fn encoding_annotations_only_affect_participating_fields() {
    compile(
        r#"
        public struct Item(
            @SerialName("wire") val value: Leaf,
            @Transient val opaque: Any
        ) : Encodable
        "#,
    )
    .unwrap();
    let errors = compile(
        r#"
        public struct Item(
            @SerialName("same") val first: Leaf,
            @SerialName("same") val second: Leaf
        ) : Encodable
        "#,
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("automatic encode has duplicate field name `same`")
    }));
}

#[test]
fn invalid_explicit_encoding_keeps_ordinary_override_diagnostics() {
    let errors = compile(
        r#"
        public struct Item(val value: Leaf) : Encodable {
            public override fun encode(encoder: Encoder): Long = 1L
        }
        "#,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("override"))
    );
    assert!(
        errors
            .iter()
            .all(|error| !error.message.contains("automatic encode requires field"))
    );
}

#[test]
fn custom_stored_accessors_require_an_explicit_codec() {
    let errors = compile(
        r#"
        public class Item public constructor() : Encodable {
            public val value: Leaf = Leaf()
                get() = field
        }
        "#,
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("stored property `value` has a custom accessor")
    }));
}

#[test]
fn a_user_interface_named_encodable_does_not_request_synthesis() {
    let errors = compile(
        r#"
        public interface Encodable { public fun encode(encoder: Encoder) }
        public struct Item(val value: Leaf) : Encodable
        "#,
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("does not implement interface method `Encodable.encode`")
    }));
}
