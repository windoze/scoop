use super::*;

#[test]
fn unit_encoding_uses_ordinary_members_and_keeps_builtin_identity() {
    let mut core = complete_core_file();
    core.declarations.retain(|declaration| {
        !matches!(declaration, ast::Decl::Struct(declaration) if declaration.name.text == "Unit")
    });
    let output = lower(&[
        core,
        scoop_parser::parse(include_str!(
            "../../../../sysroot/lib/scoop.core/src/encoding.scoop"
        ))
        .unwrap(),
        scoop_parser::parse(include_str!(
            "../../../../sysroot/lib/scoop.core/src/unit.scoop"
        ))
        .unwrap(),
        scoop_parser::parse(
            r#"
            public struct Record(val empty: Unit) : Encodable
            public fun <T : Encodable> bounded(value: T, encoder: Encoder) {
                value.encode(encoder)
            }
            public fun use(encoder: Encoder) {
                Unit.encode(encoder)
                ().encode(encoder)
                bounded(Unit, encoder)
                val erased: Any = Unit
                val boxed: Encodable = Unit
                boxed.encode(encoder)
                val reference = Unit::encode
                reference(encoder)
                Record(Unit).encode(encoder)
                if (erased is Encodable) { erased.encode(encoder) }
                val directCheck = Unit is Encodable
                val combined = erased is Record && erased is Encodable
            }
            fun main() {}
            "#,
        )
        .unwrap(),
    ])
    .unwrap();
    let module = &output.export;
    let (owner, declaration) = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Unit")
        .unwrap();
    assert_eq!(
        module.nominal_identities[owner].declaration_id(),
        hir::SourceNominalId::Concrete(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id()
        )
    );
    assert_eq!(declaration.interfaces.len(), 1);
    let method = module
        .functions
        .values()
        .find(|function| function.name == "Unit.encode")
        .unwrap();
    assert!(matches!(method.kind, hir::FunctionKind::User(_)));
    assert_eq!(method.params.len(), 2);
}
