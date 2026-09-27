use super::*;
use scoop_wire::{Encoder, WireEncode};

type Result = std::result::Result<(), scoop_wire::cbor::EncodeError>;

#[derive(Clone, Copy)]
pub(super) enum RegistrationProjection {
    Callable(scoop_lir::StrongCallableRegistrationPlanV1),
    Safepoint(scoop_lir::StrongSafepointRegistrationPlanV1),
}

impl WireEncode for RegistrationProjection {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        match self {
            Self::Callable(plan) => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(6)?;
                encoder.field(1)?;
                plan.body().encode(encoder)?;
                encoder.field(2)?;
                plan.entry_symbol().encode(encoder)
            }
            Self::Safepoint(plan) => {
                encoder.map(6)?;
                encoder.field(0)?;
                encoder.unsigned(5)?;
                encoder.field(1)?;
                plan.site().encode(encoder)?;
                encoder.field(2)?;
                plan.safepoint().encode(encoder)?;
                encoder.field(3)?;
                plan.owner().encode(encoder)?;
                encoder.field(4)?;
                plan.role().encode(encoder)?;
                encoder.field(5)?;
                encoder.unsigned(u64::from(plan.root_pair_count()))
            }
        }
    }
}

pub(super) struct AbiInput {
    pub(super) group: OdrGroupId,
    pub(super) member: OdrMemberId,
    pub(super) projection: RegistrationProjection,
}

impl WireEncode for AbiInput {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        encoder.map(4)?;
        member_fields(encoder, self.group, self.member)?;
        encoder.field(4)?;
        let (kind, version, size) = match self.projection {
            RegistrationProjection::Callable(_) => {
                use crate::link_object::callable_registrations::record;
                (6, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationProjection::Safepoint(_) => {
                use crate::link_object::safepoint_registrations::record;
                (5, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
        };
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(kind)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(version))?;
        encoder.field(2)?;
        encoder.unsigned(size as u64)
    }
}

pub(super) struct DefinitionInput {
    pub(super) group: OdrGroupId,
    pub(super) member: OdrMemberId,
    pub(super) atom: ObjectDefinitionAtomId,
    pub(super) lir: Digest256,
    pub(super) object_node: DigestNodeId,
    pub(super) object: ObjectDefinitionFingerprintV1,
    pub(super) stackmap: Option<(PersistentSafepointSiteId, StackmapRecordFingerprintV1)>,
}

impl WireEncode for DefinitionInput {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        encoder.map(6)?;
        member_fields(encoder, self.group, self.member)?;
        encoder.field(4)?;
        encoder.array(1)?;
        encoder.map(2)?;
        encoder.field(1)?;
        self.atom.encode(encoder)?;
        encoder.field(2)?;
        self.lir.encode(encoder)?;
        encoder.field(5)?;
        encoder.array(1)?;
        encoder.map(2)?;
        encoder.field(1)?;
        self.object_node.encode(encoder)?;
        encoder.field(2)?;
        self.object.encode(encoder)?;
        encoder.field(6)?;
        encoder.array(u64::from(self.stackmap.is_some()))?;
        if let Some((site, fingerprint)) = self.stackmap {
            encoder.map(2)?;
            encoder.field(1)?;
            site.encode(encoder)?;
            encoder.field(2)?;
            fingerprint.encode(encoder)?;
        }
        Ok(())
    }
}

fn member_fields(encoder: &mut Encoder, group: OdrGroupId, member: OdrMemberId) -> Result {
    encoder.field(1)?;
    group.encode(encoder)?;
    encoder.field(2)?;
    member.encode(encoder)?;
    encoder.field(3)?;
    OdrMemberRole::RegistrationRecord.encode(encoder)
}
