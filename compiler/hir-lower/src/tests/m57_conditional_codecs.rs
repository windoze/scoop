use super::*;

const ENVELOPE: &str = r#"
public struct Envelope<T>(val value: T) {
    public companion object {
        override interface ValueEncoder<Envelope<T>> where T.Companion : ValueEncoder<T> {}
        override interface ValueDecoder<Envelope<T>> where T.Companion : ValueDecoder<T> {}
    }
}
"#;

#[test]
fn m57_synthesized_codecs_retain_their_original_conditions() {
    let output = lower_with_sysroot(&format!(
        r#"{ENVELOPE}
public fun <T> write(value: Envelope<T>, encoder: Encoder)
    where T.Companion : ValueEncoder<T> {{ Envelope<T>.encode(value, encoder) }}
public fun <T> read(decoder: Decoder): Envelope<T>
    where T.Companion : ValueDecoder<T> = Envelope<T>.decode(decoder)
public fun use(encoder: Encoder, decoder: Decoder) {{
    write(Envelope(1), encoder)
    val result = read<Int>(decoder)
}}
fun main() {{}}
"#
    ))
    .unwrap();
    for name in ["Envelope.Companion.encode", "Envelope.Companion.decode"] {
        let function = output
            .export
            .functions
            .values()
            .find(|f| f.name == name)
            .unwrap();
        let condition = function.method.unwrap().availability.unwrap();
        let owner = output
            .export
            .classes
            .values()
            .find(|class| {
                class
                    .conditional_interfaces
                    .iter()
                    .any(|block| block.id() == condition)
            })
            .unwrap();
        let block = owner
            .conditional_interfaces
            .iter()
            .find(|block| block.id() == condition)
            .unwrap();
        assert_eq!(
            function.type_params(),
            block.parameters.iter().collect::<Vec<_>>()
        );
        assert!(function.type_params()[0].companion.is_some());
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("conditional codecs have ordinary checked bodies")
        };
        assert!(!body.statements.is_empty());
    }
}

#[test]
fn m57_synthesis_does_not_supply_missing_field_conditions_or_leak_them() {
    for (declaration, message) in [
        (
            "public struct Wrong<T>(val value:T) { public companion object { override interface ValueEncoder<Wrong<T>> where T.Companion : ValueDecoder<T> {} } }",
            "automatic encode has no ValueEncoder<T>",
        ),
        (
            "public struct Wrong<T>(val value:T) { public companion object { override interface ValueDecoder<Wrong<T>> where T.Companion : ValueEncoder<T> {} } }",
            "automatic decode has no ValueDecoder<T>",
        ),
    ] {
        let errors = lower_with_sysroot(declaration).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(message)),
            "{errors:#?}"
        );
    }
    for use_ in [
        "public fun <T> invalid(value: Envelope<T>, encoder: Encoder) { Envelope<T>.encode(value, encoder) }",
        "public fun <T> invalid(decoder: Decoder): Envelope<T> = Envelope<T>.decode(decoder)",
    ] {
        assert!(lower_with_sysroot(&format!("{ENVELOPE}\n{use_}")).is_err());
    }
}
