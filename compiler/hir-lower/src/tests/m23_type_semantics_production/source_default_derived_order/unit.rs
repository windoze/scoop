use super::*;
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, PersistentExactTypeId};

#[test]
fn builtin_unit_equality_uses_its_declared_core_member() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/language/unit-equality/cross-cone/provider/src/main.scoop"
    ));
    with_source(source, |output, mir| {
        let unit = PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let calls = output.executable_dependency_callables().unwrap();
        assert_eq!(calls.len(), 1);
        let selected = calls[0].callable();
        assert_eq!(selected.provider(), ConeIdentity::CORE);
        let callable = selected.capability();
        assert!(callable.direct_declaration().is_some());
        let signature = callable.signature();
        assert_eq!(signature.receiver().into_option(), Some(unit));
        assert_eq!(signature.parameters(), [unit]);
        assert_eq!(mir.meta.external_callables.len(), 1);
        assert!(
            output
                .output()
                .export
                .derived_equality_applications
                .is_empty()
        );
    });
}
