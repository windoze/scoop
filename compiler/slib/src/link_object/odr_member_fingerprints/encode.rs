use super::*;
use scoop_wire::{Encoder, WireEncode};

type Result = std::result::Result<(), scoop_wire::cbor::EncodeError>;

#[derive(Clone, Copy)]
pub(super) enum RegistrationProjection<'a> {
    StaticStorage(&'a scoop_lir::StrongStaticStorageRegistrationPlanV1),
    Initialization {
        unit: scoop_identity::PersistentInitializationUnitId,
        diagnostic_path: &'a str,
        storage: scoop_identity::PersistentStaticStorageId,
        failure_root: scoop_identity::PersistentStaticStorageId,
        initializer: scoop_identity::PersistentCallableBodyId,
        ensure: scoop_identity::PersistentCallableBodyId,
        schedule: scoop_lir::StrongInitializationSchedulePlanV1,
    },
    Immortal(scoop_lir::StrongImmortalObjectRegistrationPlanV1),
    Callable(scoop_lir::StrongCallableRegistrationPlanV1),
    Safepoint(scoop_lir::StrongSafepointRegistrationPlanV1),
    Type {
        exact: scoop_identity::PersistentExactTypeId,
        runtime: scoop_identity::RuntimeTypeId,
        descriptor: scoop_identity::PersistentSymbolRequest,
        layout: scoop_identity::PersistentLayoutId,
    },
}

impl WireEncode for RegistrationProjection<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        match self {
            Self::StaticStorage(plan) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                plan.semantic().canonical_projection().encode(encoder)
            }
            Self::Initialization {
                unit,
                diagnostic_path,
                storage,
                failure_root,
                initializer,
                ensure,
                schedule,
            } => {
                encoder.map(8)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                unit.encode(encoder)?;
                encoder.field(2)?;
                encoder.text(diagnostic_path)?;
                encoder.field(3)?;
                encoder.unsigned(u64::from(schedule.tag()))?;
                encoder.field(4)?;
                storage.encode(encoder)?;
                encoder.field(5)?;
                failure_root.encode(encoder)?;
                encoder.field(6)?;
                initializer.encode(encoder)?;
                encoder.field(7)?;
                ensure.encode(encoder)
            }
            Self::Immortal(plan) => {
                encoder.map(6)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                plan.object().encode(encoder)?;
                encoder.field(2)?;
                plan.object_symbol().encode(encoder)?;
                encoder.field(3)?;
                encoder.unsigned(plan.object_size())?;
                encoder.field(4)?;
                encoder.unsigned(plan.required_alignment())?;
                encoder.field(5)?;
                plan.type_registration().encode(encoder)
            }
            Self::Type {
                exact,
                runtime,
                descriptor,
                layout,
            } => {
                encoder.map(5)?;
                encoder.field(0)?;
                encoder.unsigned(4)?;
                encoder.field(1)?;
                exact.encode(encoder)?;
                encoder.field(2)?;
                runtime.encode(encoder)?;
                encoder.field(3)?;
                descriptor.encode(encoder)?;
                encoder.field(4)?;
                layout.encode(encoder)
            }
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

pub(super) struct AbiInput<'a> {
    pub(super) group: OdrGroupId,
    pub(super) member: OdrMemberId,
    pub(super) projection: RegistrationProjection<'a>,
}

impl WireEncode for AbiInput<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        encoder.map(4)?;
        member_fields(
            encoder,
            self.group,
            self.member,
            OdrMemberRole::RegistrationRecord,
        )?;
        encoder.field(4)?;
        let (kind, version, size) = match self.projection {
            RegistrationProjection::StaticStorage(_) => {
                use crate::link_object::static_storage_registrations::record;
                (1, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationProjection::Initialization { .. } => {
                use crate::link_object::initialization_registrations::record;
                (3, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationProjection::Immortal(_) => {
                use crate::link_object::immortal_registrations::record;
                (2, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
            RegistrationProjection::Type { .. } => {
                use crate::link_object::type_registrations::record;
                (4, record::ABI_VERSION, record::DESCRIPTOR_SIZE)
            }
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

pub(super) struct DefinitionInput<'a> {
    pub(super) group: OdrGroupId,
    pub(super) member: OdrMemberId,
    pub(super) role: OdrMemberRole,
    pub(super) atom: ObjectDefinitionAtomId,
    pub(super) lir: Digest256,
    pub(super) object_node: DigestNodeId,
    pub(super) object: ObjectDefinitionFingerprintV1,
    pub(super) additional_inputs: &'a [CanonicalDigestInputV1],
    pub(super) stackmaps: &'a [(PersistentSafepointSiteId, StackmapRecordFingerprintV1)],
}

impl WireEncode for DefinitionInput<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        encoder.map(6)?;
        member_fields(encoder, self.group, self.member, self.role)?;
        encoder.field(4)?;
        encoder.array(1)?;
        encoder.map(2)?;
        encoder.field(1)?;
        self.atom.encode(encoder)?;
        encoder.field(2)?;
        self.lir.encode(encoder)?;
        encoder.field(5)?;
        let mut inputs = Vec::from(self.additional_inputs);
        inputs.push(object_input(self.object_node, self.object));
        inputs.sort_unstable_by_key(|input| input.node);
        encoder.array(inputs.len() as u64)?;
        for input in inputs {
            encoder.map(2)?;
            encoder.field(1)?;
            input.node.encode(encoder)?;
            encoder.field(2)?;
            encoder.bytes(&input.digest)?;
        }
        encoder.field(6)?;
        encoder.array(self.stackmaps.len() as u64)?;
        for (site, fingerprint) in self.stackmaps {
            encoder.map(2)?;
            encoder.field(1)?;
            site.encode(encoder)?;
            encoder.field(2)?;
            fingerprint.encode(encoder)?;
        }
        Ok(())
    }
}

fn member_fields(
    encoder: &mut Encoder,
    group: OdrGroupId,
    member: OdrMemberId,
    role: OdrMemberRole,
) -> Result {
    encoder.field(1)?;
    group.encode(encoder)?;
    encoder.field(2)?;
    member.encode(encoder)?;
    encoder.field(3)?;
    role.encode(encoder)
}
