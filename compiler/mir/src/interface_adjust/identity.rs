//! Persistent identities for interface receiver and boxed-value adjust thunks.

use std::fmt;

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOdrMemberId, CallableOwner,
    CallableTemplateOwner, CborIdentityRecord, DispatchRole, DispatchSlotKey,
    ExactCallableSignature, ExactTypeKey, GeneratedCallableIdentityError, GeneratedCallableKey,
    OdrMemberDiscriminator, OdrMemberIdentityError, OdrMemberRole, PersistentDispatchSlotId,
    PersistentExactTypeId, PersistentGeneratedCallableId, SpecializationKey,
};

use crate::{
    CallableSignatureRecord, CallableSignatureSubject, ExactOwnerRoot, ExactOwnerRootError,
};

pub(super) type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
pub(super) type DispatchSlotRecord = CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>;
type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;
type OdrGroupRecord = CborIdentityRecord<scoop_identity::OdrGroupId, SpecializationKey>;

/// Persistent identity shared by the two concrete interface receiver adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceAdjustIdentity {
    slot: DispatchSlotRecord,
    callable: GeneratedCallableRecord,
    root: ExactOwnerRoot,
    signature: CallableSignatureRecord,
}

impl InterfaceAdjustIdentity {
    pub fn new(
        payload: &ExactTypeRecord,
        payload_group: Option<&OdrGroupRecord>,
        slot: &DispatchSlotRecord,
        interface: &ExactTypeRecord,
        signature: ExactCallableSignature,
    ) -> Result<Self, InterfaceAdjustIdentityError> {
        if !matches!(
            slot.key().role(),
            DispatchRole::InterfaceMethod
                | DispatchRole::PropertyGetter
                | DispatchRole::PropertySetter
        ) {
            return Err(InterfaceAdjustIdentityError::ExpectedInterfaceSlot);
        }
        if !matches!(
            interface.key(),
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. }
        ) {
            return Err(InterfaceAdjustIdentityError::ExpectedNominalInterface);
        }
        if signature.receiver() != scoop_identity::OptionalExactOwner::Present(interface.id()) {
            return Err(InterfaceAdjustIdentityError::InterfaceReceiverMismatch);
        }
        let callable = CborIdentityRecord::from_key(GeneratedCallableKey::BoxingAdjust {
            slot: slot.id(),
            payload: payload.id(),
            interface: interface.id(),
        })
        .map_err(InterfaceAdjustIdentityError::GeneratedCallable)?;
        let signature = ExactCallableSignature::new(
            signature.effect(),
            Some(Self::boxed_receiver(payload.id())?),
            signature.parameters().to_vec(),
            signature.result(),
        );
        Self::finish(payload, payload_group, slot, callable, signature)
    }

    pub fn reference(
        owner: &ExactTypeRecord,
        owner_group: Option<&OdrGroupRecord>,
        slot: &DispatchSlotRecord,
        target: CallableMaterialization,
        signature: ExactCallableSignature,
    ) -> Result<Self, InterfaceAdjustIdentityError> {
        if signature.receiver() != scoop_identity::OptionalExactOwner::Present(owner.id()) {
            return Err(InterfaceAdjustIdentityError::InterfaceReceiverMismatch);
        }
        let callable = CborIdentityRecord::from_key(GeneratedCallableKey::DispatchAdjust {
            slot: slot.id(),
            implementor: owner.id(),
            target,
        })
        .map_err(InterfaceAdjustIdentityError::GeneratedCallable)?;
        Self::finish(owner, owner_group, slot, callable, signature)
    }

    pub fn boxed_receiver(
        payload: PersistentExactTypeId,
    ) -> Result<PersistentExactTypeId, InterfaceAdjustIdentityError> {
        let nominal = scoop_identity::PersistentTypeId::from_generated_key(
            &scoop_identity::GeneratedNominalKey::BoxedValue { payload },
        )
        .map_err(InterfaceAdjustIdentityError::GeneratedType)?;
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal))
            .map_err(InterfaceAdjustIdentityError::Hash)
    }

    fn finish(
        payload: &ExactTypeRecord,
        payload_group: Option<&OdrGroupRecord>,
        slot: &DispatchSlotRecord,
        callable: GeneratedCallableRecord,
        signature: ExactCallableSignature,
    ) -> Result<Self, InterfaceAdjustIdentityError> {
        let root = ExactOwnerRoot::for_member(
            payload,
            payload_group,
            OdrMemberRole::DispatchAdapter,
            OdrMemberDiscriminator::GeneratedCallable(callable.id()),
        )
        .map_err(InterfaceAdjustIdentityError::Root)?;
        let subject = match root.member_record() {
            Some(member) => CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(member.key())
                    .map_err(InterfaceAdjustIdentityError::OdrMember)?,
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
pub enum InterfaceAdjustIdentityError {
    ExpectedInterfaceSlot,
    ExpectedNominalInterface,
    InterfaceReceiverMismatch,
    GeneratedCallable(GeneratedCallableIdentityError),
    GeneratedType(scoop_identity::GeneratedNominalIdentityError),
    Hash(scoop_wire::HashError),
    Root(ExactOwnerRootError),
    OdrMember(OdrMemberIdentityError),
}

impl fmt::Display for InterfaceAdjustIdentityError {
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
            Self::GeneratedType(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
            Self::Root(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InterfaceAdjustIdentityError {}
