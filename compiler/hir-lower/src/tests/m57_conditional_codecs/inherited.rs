use super::*;

pub(in crate::tests) const CODECS: &str = r#"
public struct Payload<T>(val value: T)
public struct Wrapper<T>(val value: T) {
    public companion object {
        override interface ValueEncoder<Wrapper<T>> where T.Companion : ValueEncoder<T> {}
        override interface ValueDecoder<Wrapper<T>> where T.Companion : ValueDecoder<T> {}
        override interface ValueInspector<Wrapper<T>> where T.Companion : ValueInspector<T> {}
    }
}
public abstract class Base<T> protected constructor() {
    override interface ValueEncoder<Payload<T>> where T.Companion : ValueEncoder<T> {}
    override interface ValueDecoder<Payload<T>> where T.Companion : ValueDecoder<T> {}
    override interface ValueInspector<Payload<T>> where T.Companion : ValueInspector<T> {}
}
public class Derived<T> public constructor() : Base<T>()
public class Constructed<T> public constructor() : Base<Wrapper<T>>()
public struct Host<T>() { public companion object : Base<T>() }
public struct Absent()
"#;

#[test]
fn m57_inherited_synthesis_preserves_complete_base_conditions() {
    let output = lower_with_sysroot(&format!(
        r#"{CODECS}
public fun <T> write(value: Payload<T>, encoder: Encoder)
    where T.Companion : ValueEncoder<T> {{
    Derived<T>().encode(value, encoder)
    Host<T>.encode(value, encoder)
    val encode: (Payload<T>, Encoder) -> Unit = Derived<T>()::encode
    encode(value, encoder)
    Constructed<T>().encode(Payload(Wrapper(value.value)), encoder)
}}
public fun <T> read(decoder: Decoder): Payload<Wrapper<T>>
    where T.Companion : ValueDecoder<T> = Constructed<T>().decode(decoder)
public fun <T> inspect(value: Payload<T>): ValueView
    where T.Companion : ValueInspector<T> = Derived<T>().inspect(value)
fun main() {{
    val plain = Derived<Absent>()
    val wrapped = Constructed<Absent>()
}}
"#
    ))
    .unwrap();
    for name in [
        "Derived.encode",
        "Derived.decode",
        "Derived.inspect",
        "Constructed.encode",
        "Host.Companion.encode",
    ] {
        let method = output
            .export
            .functions
            .values()
            .find(|function| function.name == name)
            .unwrap();
        let guard = method.method.unwrap().availability.unwrap();
        assert_ne!(guard.owner, method.method.unwrap().owner);
        assert!(method.type_params()[0].companion.is_none());
        let hir::FunctionKind::User(body) = &method.kind else {
            panic!("synthesis produces a checked body")
        };
        assert!(!body.statements.is_empty());
    }
}

#[test]
fn m57_inherited_generated_members_require_their_full_condition() {
    for body in [
        "public fun <T> invalid(value: Payload<T>, output: Encoder) { Derived<T>().encode(value, output) }",
        "public fun <T> invalid(value: Payload<Wrapper<T>>, output: Encoder) { Constructed<T>().encode(value, output) }",
        "public fun <T> invalid(): (Payload<T>, Encoder) -> Unit = Derived<T>()::encode",
        "public fun invalid(value: Payload<Absent>, output: Encoder) { Derived<Absent>().encode(value, output) }",
        "public fun invalid(input: Decoder): Payload<Absent> = Host<Absent>.decode(input)",
    ] {
        assert!(
            lower_with_sysroot(&format!("{CODECS}\n{body}\nfun main() {{}}")).is_err(),
            "{body}"
        );
    }
}
