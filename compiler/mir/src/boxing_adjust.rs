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
        if !matches!(
            slot.key().role(),
            DispatchRole::InterfaceMethod
                | DispatchRole::PropertyGetter
                | DispatchRole::PropertySetter
        ) {
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
    target: FunctionId,
    identity: BoxingAdjustIdentity,
}

impl BoxingAdjust {
    pub fn checked(
        functions: &Arena<Function>,
        classes: &Arena<ClassDef>,
        interfaces: &Arena<InterfaceDef>,
        location: BoxingAdjustLocation,
        target: FunctionId,
        identity: BoxingAdjustIdentity,
    ) -> Option<Self> {
        arena_get(functions, location.function)?;
        arena_get(functions, target)?;
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
        Some(Self {
            location,
            target,
            identity,
        })
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

    /// The concrete conformance target used when lowering the thunk body.
    pub const fn target(&self) -> FunctionId {
        self.target
    }

    pub const fn identity(&self) -> &BoxingAdjustIdentity {
        &self.identity
    }
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    (id.into_raw().into_u32() < arena.len() as u32).then(|| &arena[id])
}

#[cfg(test)]
mod tests;
