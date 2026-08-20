use object::macho;
use scoop_identity::{
    DefinitionAtomRole, DigestPatchIntentId, LinkageClass, ObjectDefinitionPlanId,
    PersistentSymbolKey, PersistentSymbolRequest, StrongDefinitionRole,
};
use scoop_lir::{
    StrongCallableRegistrationPlanSetV1, StrongCallableRegistrationPlanV1,
    StrongImmortalObjectRegistrationPlanSetV1, StrongImmortalObjectRegistrationPlanV1,
    StrongSafepointRegistrationPlanSetV1, StrongSafepointRegistrationPlanV1,
    StrongStaticStorageInitialArtifactPlanV1, StrongStaticStorageRegistrationPlanSetV1,
    StrongStaticStorageRegistrationPlanV1, StrongTypeRegistrationPlanSetV1,
    StrongTypeRegistrationPlanV1,
};

use crate::link_object::{PlannedMemberStrongObjectSymbolsV1, PlannedStrongObjectSymbolRoleV1};

use super::Corruption;

mod emission;
mod headers;
mod layout;
mod records;
mod relocations;
mod sections;
mod symbols;

use emission::macho_object;
use headers::*;
use layout::*;
use records::*;
use relocations::push_relocations;
use sections::push_sections;
use symbols::*;

const TEXT_SIZE: u64 = 16;
const STACK_SIZE: u64 = 64;
const SAFEPOINT_REGISTRATION_SIZE: u64 = 200;
const CALLABLE_REGISTRATION_SIZE: u64 = 176;
const TYPE_DESCRIPTOR_SIZE: u64 = 152;
const LAYOUT_SIZE: u64 = 8;
const TYPE_REGISTRATION_SIZE: u64 = 208;
const IMMORTAL_REGISTRATION_SIZE: u64 = 152;
const STATIC_LAYOUT_SIZE: u64 = 8;
const STATIC_STORAGE_REGISTRATION_SIZE: u64 = 264;
const STATIC_EMPTY_SENTINELS_SIZE: u64 = 24;

pub(crate) struct ObjectFixture {
    pub(crate) bytes: Vec<u8>,
    pub(crate) patch_offsets: Vec<(DigestPatchIntentId, u64)>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn object_bytes(
    module: &scoop_lir::Module,
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    safepoints: &[u64],
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
    type_registrations: &StrongTypeRegistrationPlanSetV1,
    immortal_registrations: &StrongImmortalObjectRegistrationPlanSetV1,
    static_storage_registrations: &StrongStaticStorageRegistrationPlanSetV1,
    corruption: Corruption,
) -> ObjectFixture {
    let mut record_ids = [safepoints[1], safepoints[0]];
    if matches!(corruption, Corruption::UnknownSafepoint) {
        record_ids[0] = u64::MAX;
    }
    let stackmap = stackmap_blob(&record_ids);
    let text = match corruption {
        Corruption::MissingFrameChain => [0xd503_201f, 0x9100_03fd, 0x9400_0000, 0x9400_0000],
        Corruption::NonCallReturnPc => [0xa9bf_7bfd, 0x9100_03fd, 0x9400_0000, 0xd503_201f],
        Corruption::None
        | Corruption::UnknownSafepoint
        | Corruption::WrongStackmapAtomRole
        | Corruption::RegistrationMagic
        | Corruption::CallableRegistrationMagic
        | Corruption::CallableEntryRelocationTarget
        | Corruption::TypeRegistrationMagic
        | Corruption::TypeDescriptorRelocationTarget
        | Corruption::TypeDescriptorScalar
        | Corruption::TypeDescriptorDiagnosticBytes
        | Corruption::TypeDescriptorDiagnosticRelocationTarget
        | Corruption::ImmortalRegistrationMagic
        | Corruption::ImmortalObjectLength
        | Corruption::ImmortalObjectDescriptorRelocationTarget
        | Corruption::ImmortalObjectRelocationTarget
        | Corruption::ImmortalTypeRegistrationRelocationTarget
        | Corruption::StaticRegistrationMagic
        | Corruption::StaticScanProgram
        | Corruption::StaticStorageRelocationTarget
        | Corruption::StaticInitialStorageRelocationTarget
        | Corruption::StaticInitialTableRelocationTarget
        | Corruption::StaticZeroedInitialState
        | Corruption::StaticZeroedWritableSection
        | Corruption::StaticEncodedEmptyInitialState
        | Corruption::StaticEncodedZeroFillSection
        | Corruption::StaticAliasedSentinels
        | Corruption::WritableRegistrationSection
        | Corruption::RelocatedRegistration => [0xa9bf_7bfd, 0x9100_03fd, 0x9400_0000, 0x9400_0000],
    };
    macho_object(
        module,
        symbols,
        &text,
        &stackmap,
        registrations,
        callable_registrations,
        type_registrations,
        immortal_registrations,
        static_storage_registrations,
        corruption,
    )
}

fn stackmap_blob(record_ids: &[u64; 2]) -> Vec<u8> {
    let mut bytes = vec![3, 0, 0, 0];
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 2);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, STACK_SIZE);
    push_u64(&mut bytes, 2);
    push_record(&mut bytes, record_ids[0], 16);
    push_record(&mut bytes, record_ids[1], 12);
    bytes
}

fn push_record(bytes: &mut Vec<u8>, safepoint: u64, instruction_offset: u32) {
    push_u64(bytes, safepoint);
    push_u32(bytes, instruction_offset);
    push_u16(bytes, 0);
    push_u16(bytes, 3);
    for _ in 0..3 {
        bytes.push(4);
        bytes.push(0);
        push_u16(bytes, 8);
        push_u16(bytes, 0);
        push_u16(bytes, 0);
        push_u32(bytes, 0);
    }
    align_zero(bytes, 8);
    push_u16(bytes, 0);
    push_u16(bytes, 0);
    align_zero(bytes, 8);
}

fn align_zero(bytes: &mut Vec<u8>, alignment: usize) {
    while !bytes.len().is_multiple_of(alignment) {
        bytes.push(0);
    }
}

fn push_fixed_name(bytes: &mut Vec<u8>, name: &[u8]) {
    let mut fixed = [0; 16];
    fixed[..name.len()].copy_from_slice(name);
    bytes.extend_from_slice(&fixed);
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
