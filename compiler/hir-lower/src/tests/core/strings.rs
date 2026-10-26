use super::super::*;

pub(super) fn string_intrinsic_declarations() -> Vec<Decl> {
    let mut copy = intrinsic_fun(
        "coreStringCopyUtf8Unchecked",
        "string_copy_utf8_unchecked",
        vec![
            ("bytes", ty_generic("Ptr", vec![ty_named("Int8")])),
            ("length", ty_named("Long")),
        ],
        Some(ty_named("String")),
    );
    let Decl::Function(function) = &mut copy else {
        unreachable!()
    };
    function.annotations.push(ast::Annotation {
        use_site: None,
        name: ident("Unsafe").into(),
        args: Vec::new(),
        span: sp(),
    });
    vec![
        intrinsic_fun(
            "coreStringByteLength",
            "string_byte_length",
            vec![("value", ty_named("String"))],
            Some(ty_named("Long")),
        ),
        intrinsic_fun(
            "coreStringByteAt",
            "string_byte_at",
            vec![("value", ty_named("String")), ("index", ty_named("Long"))],
            Some(ty_named("Int8")),
        ),
        intrinsic_fun(
            "coreStringCharacterAtByte",
            "string_character_at_byte",
            vec![("value", ty_named("String")), ("index", ty_named("Long"))],
            Some(ty_named("Char")),
        ),
        copy,
        intrinsic_fun(
            "coreStringSliceBytes",
            "string_slice_bytes",
            vec![
                ("value", ty_named("String")),
                ("start", ty_named("Long")),
                ("end", ty_named("Long")),
            ],
            Some(ty_named("String")),
        ),
        intrinsic_fun(
            "coreStringFromBytes",
            "string_from_bytes",
            vec![("bytes", ty_generic("Array", vec![ty_named("Int8")]))],
            Some(ty_named("String")),
        ),
        intrinsic_fun(
            "coreStringToBytes",
            "string_to_bytes",
            vec![("value", ty_named("String"))],
            Some(ty_generic("Array", vec![ty_named("Int8")])),
        ),
        intrinsic_fun(
            "coreStringToMutableBytes",
            "string_to_mutable_bytes",
            vec![("value", ty_named("String"))],
            Some(ty_generic("MutableArray", vec![ty_named("Int8")])),
        ),
    ]
}

#[test]
fn string_intrinsics_reject_wrong_signature_at_the_declaration() {
    for (name, intrinsic, invalid) in [
        (
            "coreStringCopyUtf8Unchecked",
            "string_copy_utf8_unchecked",
            "@Unsafe fun bad(bytes: Ptr<Long>, length: Long): String",
        ),
        (
            "coreStringSliceBytes",
            "string_slice_bytes",
            "fun bad(value: String, start: Int, end: Long): String",
        ),
        (
            "coreStringFromBytes",
            "string_from_bytes",
            "fun bad(bytes: List<Int8>): String",
        ),
        (
            "coreStringToBytes",
            "string_to_bytes",
            "fun bad(value: String): MutableArray<Int8>",
        ),
        (
            "coreStringToMutableBytes",
            "string_to_mutable_bytes",
            "fun bad(value: String): Array<Int8>",
        ),
        (
            "coreStringByteLength",
            "string_byte_length",
            "fun bad(value: String): Int",
        ),
        (
            "coreStringByteAt",
            "string_byte_at",
            "fun bad(value: String, index: Int): Int8",
        ),
        (
            "coreStringCharacterAtByte",
            "string_character_at_byte",
            "fun bad(value: String, index: Long): Int",
        ),
        (
            "coreStringByteLength",
            "string_byte_length",
            "fun <T> bad(value: String): Long",
        ),
        (
            "coreStringByteAt",
            "string_byte_at",
            "suspend fun bad(value: String, index: Long): Int8",
        ),
        (
            "coreStringByteLength",
            "string_byte_length",
            "fun String.bad(value: String): Long",
        ),
    ] {
        let mut core = core_file();
        core.declarations
            .retain(|decl| !matches!(decl, Decl::Function(f) if f.name.text == name));
        let replacement =
            scoop_parser::parse(&format!("@Intrinsic(\"{intrinsic}\") {invalid}")).unwrap();
        let Decl::Function(function) = &replacement.declarations[0] else {
            unreachable!()
        };
        let span = function.span;
        core.declarations.extend(replacement.declarations);
        let user = scoop_parser::parse("fun main() {}").unwrap();
        let errors = lower(&[core, user]).unwrap_err();
        let message = format!("malformed core String intrinsic `{intrinsic}`");
        let error = errors
            .iter()
            .find(|e| e.message == message)
            .unwrap_or_else(|| panic!("{errors:?}"));
        assert_eq!(error.file, 0);
        assert_eq!(error.span, Some(span));
    }
}
