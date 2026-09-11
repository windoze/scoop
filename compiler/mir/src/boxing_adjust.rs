//! Persistent identity for boxed-value interface adjust thunks.

use std::fmt;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOdrMemberId, CallableOwner,
    CallableTemplateOwner, CborIdentityRecord, DispatchRole, DispatchSlotKey,
    ExactCallableSignature, ExactTypeKey, GeneratedCallableIdentityError, GeneratedCallableKey,
    OdrMemberDiscriminator, OdrMemberIdentityError, OdrMemberRole, PersistentDispatchSlotId,
    PersistentExactTypeId, PersistentGeneratedCallableId, SpecializationKey,
};

use crate::{
    CallableSignatureRecord, CallableSignatureSubject, ClassDef, ClassId, ExactOwnerRoot,
    ExactOwnerRootError, Function, FunctionId, InterfaceDef, InterfaceId, TableSlot,
};

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type DispatchSlotRecord = CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>;
type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type OdrGroupRecord = CborIdentityRecord<scoop_identity::OdrGroupId, SpecializationKey>;

/// Complete persistent identity projection for one boxed-value interface
/// adjust thunk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxingAdjustIdentity {
    slot: DispatchSlotRecord,
    callable: GeneratedCallableRecord,
    root: ExactOwnerRoot,
    signature: CallableSignatureRecord,
}

impl BoxingAdjustIdentity {
    pub fn new(
        payload: &ExactTypeRecord,
        payload_group: Option<&OdrGroupRecord>,
        slot: &DispatchSlotRecord,
        interface: &ExactTypeRecord,
        signature: ExactCallableSignature,
    ) -> Result<Self, BoxingAdjustIdentityError> {
        if slot.key().role() != DispatchRole::InterfaceMethod {
            return Err(BoxingAdjustIdentityError::ExpectedInterfaceSlot);
        }
        if !matches!(
            interface.key(),
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. }
        ) {
            return Err(BoxingAdjustIdentityError::ExpectedNominalInterface);
        }
        if signature.receiver() != scoop_identity::OptionalExactOwner::Present(interface.id()) {
            return Err(BoxingAdjustIdentityError::InterfaceReceiverMismatch);
        }
        let callable = CborIdentityRecord::from_key(GeneratedCallableKey::BoxingAdjust {
            slot: slot.id(),
            payload: payload.id(),
            interface: interface.id(),
        })
        .map_err(BoxingAdjustIdentityError::GeneratedCallable)?;
        let root = ExactOwnerRoot::for_member(
            payload,
            payload_group,
            OdrMemberRole::DispatchAdapter,
            OdrMemberDiscriminator::GeneratedCallable(callable.id()),
        )
        .map_err(BoxingAdjustIdentityError::Root)?;
        let subject = match root.member_record() {
            Some(member) => CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(member.key())
                    .map_err(BoxingAdjustIdentityError::OdrMember)?,
            ),
            None => CallableSignatureSubject::strong(CallableOwner::Generated(callable.id())),
        };
        Ok(Self {
            slot: slot.clone(),
            callable,
            root,
            signature: CallableSignatureRecord::new(subject, signature),
        })
    }

    pub const fn slot_record(&self) -> &DispatchSlotRecord {
        &self.slot
    }

    pub const fn callable_record(&self) -> &GeneratedCallableRecord {
        &self.callable
    }

    pub const fn root(&self) -> &ExactOwnerRoot {
        &self.root
    }

    pub const fn signature_record(&self) -> &CallableSignatureRecord {
        &self.signature
    }

    pub const fn materialization(&self) -> CallableMaterialization {
        CallableMaterialization::new(
            CallableTemplateOwner::Generated(self.callable.id()),
            CallableMaterializationContext::NoSubstitution,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoxingAdjustIdentityError {
    ExpectedInterfaceSlot,
    ExpectedNominalInterface,
    InterfaceReceiverMismatch,
    GeneratedCallable(GeneratedCallableIdentityError),
    Root(ExactOwnerRootError),
    OdrMember(OdrMemberIdentityError),
}

impl fmt::Display for BoxingAdjustIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedInterfaceSlot => {
                formatter.write_str("boxing adjust must implement an interface dispatch slot")
            }
            Self::ExpectedNominalInterface => {
                formatter.write_str("boxing adjust interface must be an exact nominal type")
            }
            Self::InterfaceReceiverMismatch => {
                formatter.write_str("boxing adjust signature receiver does not match its interface")
            }
            Self::GeneratedCallable(error) => error.fmt(formatter),
            Self::Root(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BoxingAdjustIdentityError {}

/// Exact physical itable location materializing a boxing adjust identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BoxingAdjustLocation {
    boxed: ClassId,
    interface: InterfaceId,
    slot: u32,
    function: FunctionId,
}

impl BoxingAdjustLocation {
    pub const fn new(
        boxed: ClassId,
        interface: InterfaceId,
        slot: u32,
        function: FunctionId,
    ) -> Self {
        Self {
            boxed,
            interface,
            slot,
            function,
        }
    }

    pub const fn boxed(self) -> ClassId {
        self.boxed
    }

    pub const fn interface(self) -> InterfaceId {
        self.interface
    }

    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn function(self) -> FunctionId {
        self.function
    }
}

/// One checked physical boxing-adjust materialization.
#[derive(Clone, Debug)]
pub struct BoxingAdjust {
    location: BoxingAdjustLocation,
    identity: BoxingAdjustIdentity,
}

impl BoxingAdjust {
    pub fn checked(
        functions: &Arena<Function>,
        classes: &Arena<ClassDef>,
        interfaces: &Arena<InterfaceDef>,
        location: BoxingAdjustLocation,
        identity: BoxingAdjustIdentity,
    ) -> Option<Self> {
        arena_get(functions, location.function)?;
        let interface_definition = arena_get(interfaces, location.interface)?;
        interface_definition.methods.get(location.slot as usize)?;
        let class = arena_get(classes, location.boxed)?;
        let mut matching_tables = class
            .itables
            .iter()
            .filter(|table| table.interface == location.interface);
        let table = matching_tables.next()?;
        if matching_tables.next().is_some()
            || !matches!(table.slots.get(location.slot as usize), Some(TableSlot::Function(found)) if *found == location.function)
        {
            return None;
        }
        Some(Self { location, identity })
    }

    pub const fn location(&self) -> BoxingAdjustLocation {
        self.location
    }

    pub const fn boxed(&self) -> ClassId {
        self.location.boxed()
    }

    pub const fn interface(&self) -> InterfaceId {
        self.location.interface()
    }

    pub const fn slot(&self) -> u32 {
        self.location.slot()
    }

    pub const fn function(&self) -> FunctionId {
        self.location.function()
    }

    pub const fn identity(&self) -> &BoxingAdjustIdentity {
        &self.identity
    }
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    (id.into_raw().into_u32() < arena.len() as u32).then(|| &arena[id])
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, Effect,
        NonEmptyVec, PackagePath, PersistentFunctionId, PersistentGenericTypeId,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    use super::*;

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

    fn signature(
        interface: &ExactTypeRecord,
        result: PersistentExactTypeId,
    ) -> ExactCallableSignature {
        ExactCallableSignature::new(Effect::Ordinary, Some(interface.id()), Vec::new(), result)
    }

    #[test]
    fn source_payload_owns_a_strong_boxing_adjust() {
        let payload = concrete("Value", SourceNominalKind::Struct);
        let interface = interface();
        let identity = BoxingAdjustIdentity::new(
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
            CallableSignatureSubject::strong(CallableOwner::Generated(
                identity.callable_record().id()
            ))
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
        let identity = BoxingAdjustIdentity::new(
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
            BoxingAdjustIdentity::new(
                &payload,
                None,
                &virtual_slot,
                &interface,
                signature(&interface, payload.id()),
            ),
            Err(BoxingAdjustIdentityError::ExpectedInterfaceSlot)
        );
        let different = concrete("Other", SourceNominalKind::Interface);
        assert_eq!(
            BoxingAdjustIdentity::new(
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
            Err(BoxingAdjustIdentityError::InterfaceReceiverMismatch)
        );
    }
}
