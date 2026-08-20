use super::*;
use scoop_identity::{
    CallableMaterializationContext, CallableTemplateOwner, CoreBuiltinNominal,
    GeneratedCallableKey, PersistentExactTypeId, PersistentGeneratedCallableId, SpecializationKey,
};

#[test]
fn builtin_unit_equality_has_an_exact_odr_root() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-cli-unit-equality/cross-cone/provider/src/main.scoop"
    ));
    with_source(source, |_, mir| {
        let unit = PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let generated =
            PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::DerivedEquality {
                exact_owner: unit,
            })
            .unwrap();
        let equality = mir
            .meta
            .source_callable_materializations
            .iter()
            .find(|entry| {
                entry.materialization().template() == CallableTemplateOwner::Generated(generated)
            })
            .unwrap();
        assert_eq!(
            equality.materialization().context(),
            CallableMaterializationContext::NoSubstitution
        );
        let group = equality
            .exact_owner()
            .unwrap()
            .mir_odr_group_record()
            .unwrap();
        assert_eq!(
            group.key(),
            &SpecializationKey::StructuralType { exact_type: unit }
        );
        assert_eq!(
            equality.odr_member_record().unwrap().key().group(),
            group.id()
        );
        assert!(matches!(
            equality.signature_record().subject(),
            scoop_mir::CallableSignatureSubject::Odr(_)
        ));
    });
}
