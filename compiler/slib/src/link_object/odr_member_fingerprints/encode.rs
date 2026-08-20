use super::*;
use scoop_wire::{Encoder, WireEncode};

pub(super) struct RegistrationAbi {
    pub(super) group: OdrGroupId,
    pub(super) member: OdrMemberId,
    pub(super) table: RegistrationTableV1,
}

impl WireEncode for RegistrationAbi {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.group.encode(encoder)?;
        encoder.field(2)?;
        self.member.encode(encoder)?;
        encoder.field(3)?;
        OdrMemberRole::RegistrationRecord.encode(encoder)?;
        encoder.field(4)?;
        let (kind, version, size) = match self.table {
            RegistrationTableV1::StaticStorage => {
                use crate::link_object::static_storage_registrations::record;
                (1, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationTableV1::ImmortalObject => {
                use crate::link_object::immortal_registrations::record;
                (2, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationTableV1::InitializationUnit => {
                use crate::link_object::initialization_registrations::record;
                (3, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationTableV1::Type => {
                use crate::link_object::type_registrations::record;
                (4, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationTableV1::Safepoint => {
                use crate::link_object::safepoint_registrations::record;
                (5, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationTableV1::Callable => {
                use crate::link_object::callable_registrations::record;
                (6, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
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
