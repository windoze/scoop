use super::*;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, EnumVariantFieldKey,
    EnumVariantFieldSelector, EnumVariantIdentityKey, PackagePath, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

pub(super) fn variant_def(name: &str, fields: Vec<Type>) -> VariantDef {
    let owner = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("TestEnum").unwrap(),
        SourceNominalKind::Enum,
        0,
    );
    let key =
        EnumVariantIdentityKey::source(&owner, CanonicalIdentifier::new(name).unwrap()).unwrap();
    let identity = PersistentEnumVariantId::from_key(&key).unwrap();
    VariantDef {
        identity,
        name: name.to_string(),
        gc_free: true,
        fields: fields
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                let key = EnumVariantFieldKey::new(
                    identity,
                    EnumVariantFieldSelector::Positional {
                        declaration_index: u32::try_from(index).unwrap(),
                    },
                );
                VariantField {
                    identity: PersistentEnumVariantFieldId::from_key(&key).unwrap(),
                    name: format!("_{index}"),
                    ty,
                }
            })
            .collect(),
    }
}

pub(super) fn step_variants(result: &Type) -> Vec<VariantDef> {
    let identity = test_step_identity(result);
    let mut completed = variant_def("Completed", vec![result.clone()]);
    completed.identity = identity.completed_variant_record().id();
    completed.fields[0].identity = identity.completed_payload_record().id();
    let mut suspended = variant_def("Suspended", Vec::new());
    suspended.identity = identity.suspended_variant_record().id();
    vec![completed, suspended]
}

pub(super) fn slot_variants(value: &Type) -> Vec<VariantDef> {
    let identity = test_slot_identity(value);
    let mut empty = variant_def("Empty", Vec::new());
    empty.identity = identity.empty_variant_record().id();
    let mut payload = variant_def("Value", vec![value.clone()]);
    payload.identity = identity.value_variant_record().id();
    payload.fields[0].identity = identity.value_payload_record().id();
    vec![empty, payload]
}

#[test]
fn coroutine_step_rejects_wrong_member_identities() {
    let result = Type::Integer(IntegerKind::SIGNED_32);
    let identity = test_step_identity(&result);
    for member in 0..3 {
        let (mut module, id) = module_with_variants(step_variants(&result));
        let variant = MirVariantRef::new(&module.enums, id, 0).unwrap();
        let payload = MirVariantFieldRef::new(&module.enums, variant, 0).unwrap();
        let suspended = MirVariantRef::new(&module.enums, id, 1).unwrap();
        assert!(
            CoroutineStep::checked(
                &module.enums,
                payload,
                suspended,
                result.clone(),
                identity.clone()
            )
            .is_some()
        );
        let foreign = variant_def("Other", vec![result.clone()]);
        match member {
            0 => module.enums[id].variants[0].identity = foreign.identity,
            1 => module.enums[id].variants[1].identity = foreign.identity,
            2 => module.enums[id].variants[0].fields[0].identity = foreign.fields[0].identity,
            _ => unreachable!(),
        }
        assert!(
            CoroutineStep::checked(
                &module.enums,
                payload,
                suspended,
                result.clone(),
                identity.clone()
            )
            .is_none()
        );
    }
}

#[test]
fn coroutine_slot_rejects_wrong_member_identities() {
    let value = Type::Integer(IntegerKind::SIGNED_32);
    let identity = test_slot_identity(&value);
    for member in 0..3 {
        let (mut module, id) = module_with_variants(slot_variants(&value));
        let empty = MirVariantRef::new(&module.enums, id, 0).unwrap();
        let variant = MirVariantRef::new(&module.enums, id, 1).unwrap();
        let payload = MirVariantFieldRef::new(&module.enums, variant, 0).unwrap();
        assert!(
            CoroutineSlot::checked(
                &module.enums,
                payload,
                empty,
                value.clone(),
                identity.clone()
            )
            .is_some()
        );
        let foreign = variant_def("Other", vec![value.clone()]);
        match member {
            0 => module.enums[id].variants[0].identity = foreign.identity,
            1 => module.enums[id].variants[1].identity = foreign.identity,
            2 => module.enums[id].variants[1].fields[0].identity = foreign.fields[0].identity,
            _ => unreachable!(),
        }
        assert!(
            CoroutineSlot::checked(
                &module.enums,
                payload,
                empty,
                value.clone(),
                identity.clone()
            )
            .is_none()
        );
    }
}
