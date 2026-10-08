use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, Effect, NonEmptyVec,
    PackagePath, PersistentFunctionId, PersistentGenericTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

use super::identity::{DispatchSlotRecord, ExactTypeRecord};
use super::*;
use crate::{CallableSignatureSubject, ExactOwnerRoot};
use scoop_identity::{
    CallableOdrMemberId, CallableOwner, CborIdentityRecord, DispatchSlotKey,
    ExactCallableSignature, ExactTypeKey, GeneratedCallableKey, OdrMemberDiscriminator,
    OdrMemberRole, PersistentExactTypeId, SpecializationKey,
};

fn declaration(name: &str, kind: SourceNominalKind, parameters: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        parameters,
    )
}

fn concrete(name: &str, kind: SourceNominalKind) -> ExactTypeRecord {
    let owner =
        scoop_identity::PersistentTypeId::from_source_declaration(&declaration(name, kind, 0))
            .unwrap();
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap()
}

fn interface() -> ExactTypeRecord {
    concrete("Description", SourceNominalKind::Interface)
}

fn interface_slot() -> DispatchSlotRecord {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("describe").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    CborIdentityRecord::from_key(DispatchSlotKey::interface_method(function)).unwrap()
}

fn signature(interface: &ExactTypeRecord, result: PersistentExactTypeId) -> ExactCallableSignature {
    ExactCallableSignature::new(Effect::Ordinary, Some(interface.id()), Vec::new(), result)
}

#[test]
fn source_payload_owns_a_strong_boxing_adjust() {
    let payload = concrete("Value", SourceNominalKind::Struct);
    let interface = interface();
    let identity = InterfaceAdjustIdentity::new(
        &payload,
        None,
        &interface_slot(),
        &interface,
        signature(&interface, payload.id()),
    )
    .unwrap();

    assert!(matches!(
        identity.callable_record().key(),
        GeneratedCallableKey::BoxingAdjust {
            slot,
            payload: found_payload,
            interface: found_interface,
        } if *slot == identity.slot_record().id()
            && *found_payload == payload.id()
            && *found_interface == interface.id()
    ));
    assert_eq!(
        identity.root(),
        &ExactOwnerRoot::SourceNominal(match payload.key() {
            ExactTypeKey::Nominal(owner) => *owner,
            _ => unreachable!(),
        })
    );
    assert_eq!(
        identity.signature_record().subject(),
        CallableSignatureSubject::strong(CallableOwner::Generated(identity.callable_record().id()))
    );
    assert_eq!(
        identity.materialization().generated_template(),
        Some(identity.callable_record().id())
    );
}

#[test]
fn generic_payload_uses_a_dispatch_adapter_member_of_its_nominal_group() {
    let argument = concrete("Element", SourceNominalKind::Struct).id();
    let origin = PersistentGenericTypeId::from_source_declaration(&declaration(
        "Box",
        SourceNominalKind::Struct,
        1,
    ))
    .unwrap();
    let arguments = NonEmptyVec::from_first(argument, []);
    let payload = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
        origin,
        arguments: arguments.clone(),
    })
    .unwrap();
    let group =
        CborIdentityRecord::from_key(SpecializationKey::Nominal { origin, arguments }).unwrap();
    let interface = interface();
    let identity = InterfaceAdjustIdentity::new(
        &payload,
        Some(&group),
        &interface_slot(),
        &interface,
        signature(&interface, argument),
    )
    .unwrap();

    let ExactOwnerRoot::NominalApplication(root) = identity.root() else {
        panic!("generic payload adjust must be ODR-owned")
    };
    assert_eq!(root.group(), group.id());
    assert_eq!(
        root.member_record().key().role(),
        OdrMemberRole::DispatchAdapter
    );
    assert_eq!(
        root.member_record().key().discriminator(),
        &OdrMemberDiscriminator::GeneratedCallable(identity.callable_record().id())
    );
    assert_eq!(
        identity.signature_record().subject(),
        CallableSignatureSubject::odr(
            CallableOdrMemberId::from_key(root.member_record().key()).unwrap()
        )
    );
}

#[test]
fn identity_rejects_a_virtual_slot_and_a_different_receiver() {
    let payload = concrete("Value", SourceNominalKind::Struct);
    let interface = interface();
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("describe").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let virtual_slot =
        CborIdentityRecord::from_key(DispatchSlotKey::virtual_method(function)).unwrap();
    assert_eq!(
        InterfaceAdjustIdentity::new(
            &payload,
            None,
            &virtual_slot,
            &interface,
            signature(&interface, payload.id()),
        ),
        Err(InterfaceAdjustIdentityError::ExpectedInterfaceSlot)
    );
    let different = concrete("Other", SourceNominalKind::Interface);
    assert_eq!(
        InterfaceAdjustIdentity::new(
            &payload,
            None,
            &interface_slot(),
            &interface,
            ExactCallableSignature::new(
                Effect::Ordinary,
                Some(different.id()),
                Vec::new(),
                payload.id(),
            ),
        ),
        Err(InterfaceAdjustIdentityError::InterfaceReceiverMismatch)
    );
}

#[test]
fn property_accessor_slots_keep_distinct_boxing_adjust_identities() {
    use scoop_identity::{
        AccessorRole, PersistentPropertyAccessorId, PersistentPropertyId, PropertyAccessorKey,
        PropertyOwner,
    };
    let property = SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("value").unwrap(),
    );
    let property = PersistentPropertyId::from_source_declaration(&property).unwrap();
    let payload = concrete("Value", SourceNominalKind::Struct);
    let interface = interface();
    let mut generated = Vec::new();
    for role in [AccessorRole::Getter, AccessorRole::Setter] {
        let accessor = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            PropertyOwner::Property(property),
            role,
        ))
        .unwrap();
        let slot = CborIdentityRecord::from_key(match role {
            AccessorRole::Getter => DispatchSlotKey::property_getter(accessor),
            AccessorRole::Setter => DispatchSlotKey::property_setter(accessor),
        })
        .unwrap();
        let signature = match role {
            AccessorRole::Getter => signature(&interface, payload.id()),
            AccessorRole::Setter => ExactCallableSignature::new(
                Effect::Ordinary,
                Some(interface.id()),
                vec![payload.id()],
                PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
                    scoop_identity::CoreBuiltinNominal::Unit
                        .identity_record()
                        .id(),
                ))
                .unwrap(),
            ),
        };
        let identity =
            InterfaceAdjustIdentity::new(&payload, None, &slot, &interface, signature).unwrap();
        assert_eq!(identity.slot_record(), &slot);
        generated.push(identity.callable_record().id());
    }
    assert_ne!(generated[0], generated[1]);
}
