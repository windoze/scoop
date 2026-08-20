use super::*;

#[test]
fn unit_encoding_uses_ordinary_members_and_keeps_builtin_identity() {
    let output = lower_with_sysroot(
        r#"
            public fun <T> send(value: T, codec: Encodable<T>, encoder: Encoder) {
                codec.encode(value, encoder)
            }
            public fun use(encoder: Encoder) {
                UnitEncoder.encode(Unit, encoder)
                UnitEncoder.encode((), encoder)
                send(Unit, UnitEncoder, encoder)
                val erased: Any = Unit
                val codec: Encodable<Unit> = UnitEncoder
                codec.encode(Unit, encoder)
                val reference = codec::encode
                reference(Unit, encoder)
                if (erased is Encodable<Unit>) { erased.encode(Unit, encoder) }
            }
            fun main() {}
            "#,
    )
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
    let (equality, _) = module
        .interfaces
        .iter()
        .find(|(_, declaration)| declaration.name == "Equality")
        .unwrap();
    assert_eq!(declaration.interfaces.len(), 1);
    let hir::Type::Interface(application) = &module.types[declaration.interfaces[0]] else {
        panic!("Unit explicitly implements Equality<Unit>");
    };
    let application = &module.interface_applications[*application];
    assert_eq!(
        application.template,
        module.nominal_identities[equality].declaration_id()
    );
    assert_eq!(application.arguments.len(), 1);
    assert_eq!(module.types[application.arguments[0]], hir::Type::Unit);
    let method = module
        .functions
        .values()
        .find(|function| function.name == "UnitEncoder.encode")
        .unwrap();
    assert!(matches!(method.kind, hir::FunctionKind::User(_)));
    assert_eq!(method.params.len(), 3);
}
