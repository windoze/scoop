use super::*;

const LEAF: &str = r#"
    public struct Leaf() {
        public companion object : Decodable<Leaf> {
            public override fun decode(decoder: Decoder): Leaf = Leaf()
        }
    }
"#;

fn compile(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let decoding = include_str!("../../../../sysroot/lib/scoop.core/src/decoding.scoop");
    lower(&[
        complete_core_file(),
        scoop_parser::parse(&format!(
            r#"
            public interface List<T> {{
                public val size: Long
                public operator fun get(index: Long): T
            }}
            {decoding}
        "#
        ))
        .unwrap(),
        scoop_parser::parse(include_str!(
            "../../../../sysroot/lib/scoop.core/src/serialization_annotations.scoop"
        ))
        .unwrap(),
        scoop_parser::parse(&format!("{LEAF}\n{source}\nfun main() {{}}"))
            .expect("the test uses valid source syntax"),
    ])
}

fn rejects(source: &str, message: &str) {
    let errors = compile(source).unwrap_err();
    assert!(
        errors.iter().any(|error| error.message.contains(message)),
        "{errors:?}"
    );
}

#[test]
fn nominal_and_tuple_decoding_have_complete_ordinary_bodies() {
    let output = compile(r#"
        public struct Record(@SerialName("item") val value: Leaf, @Transient val ignored: Leaf = Leaf()) {
            public companion object : Decodable<Record>
        }
        public class Row public constructor(public val value: Leaf, plain: Leaf = Leaf()) {
            @Transient private val ignored: Leaf = plain
        }
        public object RowDecoder : Decodable<Row>
        public enum Choice {
            Empty,
            Pair(Leaf, Leaf),
            Named(val value: Leaf = Leaf()),
            Record { item: Leaf }
        }
        public object ChoiceDecoder : Decodable<Choice>
        public object TupleDecoder : Decodable<(Leaf, Leaf)>
    "#).unwrap();
    for name in [
        "Record.Companion.decode",
        "RowDecoder.decode",
        "ChoiceDecoder.decode",
        "TupleDecoder.decode",
    ] {
        let methods = output
            .export
            .functions
            .values()
            .filter(|function| function.name == name)
            .collect::<Vec<_>>();
        assert_eq!(methods.len(), 1, "one ordinary method for {name}");
        assert_eq!(
            methods[0].params.len(),
            2,
            "receiver and decoder for {name}"
        );
        let hir::FunctionKind::User(body) = &methods[0].kind else {
            panic!("{name} requires a normal body")
        };
        assert!(!body.statements.is_empty(), "{name} is complete");
    }
}

#[test]
fn generic_fields_use_primary_val_dependencies_and_nested_tuple_closures() {
    let output = compile(r#"
        public struct Pair<A, B>(val first: A, val second: B)
        public class PairDecoder<A, B>(private val first: Decodable<A>, private val second: Decodable<B>) : Decodable<Pair<A, B>>
        public struct Nested<T>(val pair: (T, Leaf))
        public struct NestedDecoder<T>(val element: Decodable<T>) : Decodable<Nested<T>>
    "#).unwrap();
    assert!(
        !output.export.lambdas.is_empty(),
        "tuple composition uses a normal closure"
    );
}

#[test]
fn explicit_inherited_and_default_decoders_keep_normal_priority() {
    compile(
        r#"
        public interface Existing : Decodable<Any> {
            public override fun decode(decoder: Decoder): Any = Leaf()
        }
        public object InheritsDefault : Existing
        public open class Base public constructor() : Decodable<Any> {
            public override fun decode(decoder: Decoder): Any = Leaf()
        }
        public class Child public constructor() : Base()
        public struct Explicit(val opaque: Any) : Decodable<Any> {
            public override fun decode(decoder: Decoder): Any = opaque
        }
    "#,
    )
    .unwrap();
}

#[test]
fn unrelated_decode_overload_does_not_replace_the_requirement() {
    let output = compile(
        r#"
        public struct Record(val value: Leaf)
        public object Reader : Decodable<Record> {
            public fun decode(value: Leaf): Leaf = value
        }
    "#,
    )
    .unwrap();
    assert_eq!(
        output
            .export
            .functions
            .values()
            .filter(|function| function.name == "Reader.decode")
            .count(),
        2
    );
}

#[test]
fn missing_and_ambiguous_field_dependencies_are_definition_errors() {
    rejects(
        r#"
        public struct Box<T>(val value: T)
        public class Reader<T>() : Decodable<Box<T>>
    "#,
        "has no Decodable<T>",
    );
    rejects(
        r#"
        public struct Box<T>(val value: T)
        public class Reader<T>(private val first: Decodable<T>, private val second: Decodable<T>) : Decodable<Box<T>>
    "#,
        "ambiguous primary val dependencies",
    );
    rejects(
        r#"
        public struct Box<T>(val value: T)
        public class Reader<T>(private var element: Decodable<T>) : Decodable<Box<T>>
    "#,
        "has no Decodable<T>",
    );
}

#[test]
fn transient_and_duplicate_wire_names_use_actual_participating_fields() {
    rejects(
        r#"
        public struct Record(@Transient val value: Leaf)
        public object Reader : Decodable<Record>
    "#,
        "declared default for omitted constructor parameter `value`",
    );
    rejects(
        r#"
        public struct Record(@SerialName("same") val first: Leaf, @SerialName("same") val second: Leaf)
        public object Reader : Decodable<Record>
    "#,
        "duplicate field name `same`",
    );
    rejects(
        r#"
        public enum Choice { @SerialName("same") A, @SerialName("same") B }
        public object Reader : Decodable<Choice>
    "#,
        "duplicate variant name `same`",
    );
}

#[test]
fn automatic_class_decoding_requires_primary_storage_mapping() {
    rejects(
        r#"
        public object Record
        public object Reader : Decodable<Record>
    "#,
        "cannot construct singleton Record",
    );
    rejects(
        r#"
        public open class Record public constructor()
        public object Reader : Decodable<Record>
    "#,
        "final class without a class base",
    );
    rejects(
        r#"
        public class Record public constructor() { public val value: Leaf = Leaf() }
        public object Reader : Decodable<Record>
    "#,
        "primary constructor parameter or Transient",
    );
}

#[test]
fn a_user_interface_with_the_same_name_is_not_a_core_protocol() {
    rejects(
        r#"
        public interface Decodable<T> { public fun decode(decoder: Decoder): T }
        public struct Record(val value: Leaf)
        public object Reader : Decodable<Record>
    "#,
        "does not implement",
    );
}
