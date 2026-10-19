use super::super::*;

pub(super) fn string_intrinsic_declarations() -> Vec<Decl> {
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
    ]
}

#[test]
fn string_intrinsics_reject_wrong_signature_at_the_declaration() {
    for (name, intrinsic, invalid) in [
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
