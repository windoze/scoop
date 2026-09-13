use object::macho;
use scoop_identity::{
    DefinitionAtomRole, DigestPatchIntentId, ObjectDefinitionPlanId, StrongDefinitionRole,
};
use scoop_lir::{
    StrongCallableRegistrationPlanSetV1, StrongCallableRegistrationPlanV1,
    StrongSafepointRegistrationPlanSetV1, StrongSafepointRegistrationPlanV1,
};

use crate::link_object::{PlannedMemberStrongObjectSymbolsV1, PlannedStrongObjectSymbolRoleV1};

use super::Corruption;

const TEXT_SIZE: u64 = 16;
const STACK_SIZE: u64 = 64;
const SAFEPOINT_REGISTRATION_SIZE: u64 = 232;
const CALLABLE_REGISTRATION_SIZE: u64 = 192;

pub(crate) struct ObjectFixture {
    pub(crate) bytes: Vec<u8>,
    pub(crate) patch_offsets: Vec<(DigestPatchIntentId, u64)>,
}

pub(crate) fn object_bytes(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    safepoints: &[u64],
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
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
        | Corruption::WritableRegistrationSection
        | Corruption::RelocatedRegistration => [0xa9bf_7bfd, 0x9100_03fd, 0x9400_0000, 0x9400_0000],
    };
    macho_object(
        symbols,
        &text,
        &stackmap,
        registrations,
        callable_registrations,
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

fn macho_object(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    instructions: &[u32; 4],
    stackmap: &[u8],
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
    corruption: Corruption,
) -> ObjectFixture {
    let segment_size = 72_u32 + 3 * 80;
    let command_bytes = segment_size + 24 + 80;
    let text_offset = 32 + command_bytes;
    let stackmap_offset = text_offset + u32::try_from(TEXT_SIZE).unwrap();
    let stackmap_size = u32::try_from(stackmap.len()).unwrap();
    let registration_offset = stackmap_offset + stackmap_size;
    let registration_size = u32::try_from(
        registrations.registrations().len() * usize::try_from(SAFEPOINT_REGISTRATION_SIZE).unwrap()
            + callable_registrations.registrations().len()
                * usize::try_from(CALLABLE_REGISTRATION_SIZE).unwrap(),
    )
    .unwrap();
    let relocation_offset = registration_offset + registration_size;
    let registration_relocation_count = u32::try_from(callable_registrations.registrations().len())
        .unwrap()
        + u32::from(matches!(corruption, Corruption::RelocatedRegistration));
    let symbol_offset = relocation_offset + 8 * (2 + registration_relocation_count);
    let symbol_bytes = u32::try_from(symbols.symbols().len() * 16).unwrap();
    let string_offset = symbol_offset + symbol_bytes;
    let mut strings = vec![0];
    let string_indexes = symbols
        .symbols()
        .iter()
        .map(|symbol| {
            let index = u32::try_from(strings.len()).unwrap();
            strings.extend_from_slice(symbol.macho_name());
            strings.push(0);
            index
        })
        .collect::<Vec<_>>();
    let string_size = u32::try_from(strings.len()).unwrap();
    let mut bytes = Vec::with_capacity((string_offset + string_size) as usize);

    push_header(&mut bytes, command_bytes);
    push_segment(
        &mut bytes,
        segment_size,
        text_offset,
        stackmap_size,
        registration_size,
    );
    push_section(
        &mut bytes,
        b"__text",
        b"__TEXT",
        0,
        u32::try_from(TEXT_SIZE).unwrap(),
        text_offset,
        2,
        relocation_offset,
        1,
        macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
    );
    push_section(
        &mut bytes,
        b"__llvm_stackmaps",
        b"__LLVM_STACKMAPS",
        TEXT_SIZE,
        stackmap_size,
        stackmap_offset,
        3,
        relocation_offset + 8,
        1,
        macho::S_REGULAR,
    );
    let (registration_section, registration_segment) =
        if matches!(corruption, Corruption::WritableRegistrationSection) {
            (b"__data".as_slice(), b"__DATA".as_slice())
        } else {
            (b"__const".as_slice(), b"__DATA_CONST".as_slice())
        };
    push_section(
        &mut bytes,
        registration_section,
        registration_segment,
        TEXT_SIZE + u64::from(stackmap_size),
        registration_size,
        registration_offset,
        3,
        if registration_relocation_count == 0 {
            0
        } else {
            relocation_offset + 16
        },
        registration_relocation_count,
        macho::S_REGULAR,
    );
    push_symbol_commands(
        &mut bytes,
        symbols.symbols().len(),
        symbol_offset,
        string_offset,
        string_size,
    );

    for instruction in instructions {
        push_u32(&mut bytes, *instruction);
    }
    bytes.extend_from_slice(stackmap);
    for registration in registrations.registrations() {
        push_registration(&mut bytes, *registration);
    }
    for registration in callable_registrations.registrations() {
        push_callable_registration(&mut bytes, *registration);
    }
    if matches!(corruption, Corruption::RegistrationMagic) {
        bytes[usize::try_from(registration_offset).unwrap()] ^= 1;
    }
    if matches!(corruption, Corruption::CallableRegistrationMagic) {
        let offset = u64::from(registration_offset)
            + u64::try_from(registrations.registrations().len()).unwrap()
                * SAFEPOINT_REGISTRATION_SIZE;
        bytes[usize::try_from(offset).unwrap()] ^= 1;
    }
    let target = symbols
        .symbols()
        .iter()
        .position(|symbol| {
            matches!(
                symbol.role(),
                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                    definition_role: StrongDefinitionRole::CallableBody,
                    ..
                }
            )
        })
        .unwrap();
    push_u32(&mut bytes, 8);
    push_u32(
        &mut bytes,
        u32::try_from(target).unwrap()
            | 1 << 24
            | 2 << 25
            | 1 << 27
            | u32::from(macho::ARM64_RELOC_BRANCH26) << 28,
    );
    push_u32(&mut bytes, 16);
    push_u32(
        &mut bytes,
        u32::try_from(target).unwrap() | 3 << 25 | 1 << 27,
    );
    if registration_relocation_count != 0 {
        if matches!(corruption, Corruption::RelocatedRegistration) {
            push_u32(&mut bytes, 56);
            push_u32(
                &mut bytes,
                u32::try_from(target).unwrap() | 3 << 25 | 1 << 27,
            );
        }
        for (index, _) in callable_registrations.registrations().iter().enumerate() {
            let safepoint_bytes = u32::try_from(registrations.registrations().len()).unwrap()
                * u32::try_from(SAFEPOINT_REGISTRATION_SIZE).unwrap();
            let callable_bytes =
                u32::try_from(index).unwrap() * u32::try_from(CALLABLE_REGISTRATION_SIZE).unwrap();
            push_u32(&mut bytes, safepoint_bytes + callable_bytes + 184);
            let relocation_target =
                if matches!(corruption, Corruption::CallableEntryRelocationTarget) {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::CallableRegistration,
                                    ..
                                }
                            )
                        })
                        .unwrap()
                } else {
                    target
                };
            push_u32(
                &mut bytes,
                u32::try_from(relocation_target).unwrap() | 3 << 25 | 1 << 27,
            );
        }
    }
    for (symbol, string_index) in symbols.symbols().iter().zip(string_indexes) {
        let (section, value) = symbol_location(
            symbol.role(),
            stackmap.len(),
            registrations,
            callable_registrations,
        );
        push_u32(&mut bytes, string_index);
        bytes.push(macho::N_SECT | macho::N_EXT);
        bytes.push(section);
        push_u16(&mut bytes, 0);
        push_u64(&mut bytes, value);
    }
    bytes.extend_from_slice(&strings);
    let mut patch_offsets = registrations
        .registrations()
        .iter()
        .enumerate()
        .flat_map(|(index, registration)| {
            let record = u64::from(registration_offset)
                + u64::try_from(index).unwrap() * SAFEPOINT_REGISTRATION_SIZE;
            [
                (registration.registration_definition_patch(), record + 120),
                (registration.normalized_stackmap_patch(), record + 200),
            ]
        })
        .collect::<Vec<_>>();
    let callable_base = u64::from(registration_offset)
        + u64::try_from(registrations.registrations().len()).unwrap() * SAFEPOINT_REGISTRATION_SIZE;
    patch_offsets.extend(
        callable_registrations
            .registrations()
            .iter()
            .enumerate()
            .flat_map(|(index, registration)| {
                let record =
                    callable_base + u64::try_from(index).unwrap() * CALLABLE_REGISTRATION_SIZE;
                [
                    (registration.registration_definition_patch(), record + 120),
                    (registration.body_definition_patch(), record + 152),
                ]
            }),
    );
    patch_offsets.sort_unstable_by_key(|(intent, _)| *intent);
    ObjectFixture {
        bytes,
        patch_offsets,
    }
}

fn push_header(bytes: &mut Vec<u8>, command_bytes: u32) {
    push_u32(bytes, macho::MH_MAGIC_64);
    push_u32(bytes, macho::CPU_TYPE_ARM64);
    push_u32(bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(bytes, macho::MH_OBJECT);
    push_u32(bytes, 3);
    push_u32(bytes, command_bytes);
    push_u32(bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(bytes, 0);
}

fn push_segment(
    bytes: &mut Vec<u8>,
    segment_size: u32,
    text_offset: u32,
    stackmap_size: u32,
    registration_size: u32,
) {
    push_u32(bytes, macho::LC_SEGMENT_64);
    push_u32(bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(bytes, 0);
    push_u64(
        bytes,
        TEXT_SIZE + u64::from(stackmap_size) + u64::from(registration_size),
    );
    push_u64(bytes, u64::from(text_offset));
    push_u64(
        bytes,
        TEXT_SIZE + u64::from(stackmap_size) + u64::from(registration_size),
    );
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 3);
    push_u32(bytes, 0);
}

#[allow(clippy::too_many_arguments)]
fn push_section(
    bytes: &mut Vec<u8>,
    section: &[u8],
    segment: &[u8],
    address: u64,
    size: u32,
    offset: u32,
    alignment: u32,
    relocation_offset: u32,
    relocation_count: u32,
    flags: u32,
) {
    push_fixed_name(bytes, section);
    push_fixed_name(bytes, segment);
    push_u64(bytes, address);
    push_u64(bytes, u64::from(size));
    push_u32(bytes, offset);
    push_u32(bytes, alignment);
    push_u32(bytes, relocation_offset);
    push_u32(bytes, relocation_count);
    push_u32(bytes, flags);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
}

fn push_symbol_commands(
    bytes: &mut Vec<u8>,
    symbol_count: usize,
    symbol_offset: u32,
    string_offset: u32,
    string_size: u32,
) {
    let symbol_count = u32::try_from(symbol_count).unwrap();
    push_u32(bytes, macho::LC_SYMTAB);
    push_u32(bytes, 24);
    push_u32(bytes, symbol_offset);
    push_u32(bytes, symbol_count);
    push_u32(bytes, string_offset);
    push_u32(bytes, string_size);

    push_u32(bytes, macho::LC_DYSYMTAB);
    push_u32(bytes, 80);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, symbol_count);
    push_u32(bytes, symbol_count);
    push_u32(bytes, 0);
    bytes.extend_from_slice(&[0; 48]);
}

fn symbol_location(
    role: PlannedStrongObjectSymbolRoleV1,
    stackmap_size: usize,
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
) -> (u8, u64) {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::CallableBody,
            ..
        } => (1, 0),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            atom_role: DefinitionAtomRole::Primary,
            definition,
            ..
        } if !is_registration_definition(definition, registrations, callable_registrations) => {
            (1, 0)
        }
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            atom_role: DefinitionAtomRole::Primary,
            definition,
            ..
        } if !is_registration_definition(definition, registrations, callable_registrations) => {
            (1, TEXT_SIZE)
        }
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            atom_role: DefinitionAtomRole::Stackmap | DefinitionAtomRole::AddressTakenConstant,
            ..
        } => (2, TEXT_SIZE),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            atom_role: DefinitionAtomRole::Stackmap | DefinitionAtomRole::AddressTakenConstant,
            ..
        } => (2, TEXT_SIZE + u64::try_from(stackmap_size).unwrap()),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::SafepointRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_safepoint_registration_definition(definition, registrations) => (
            3,
            registration_start(definition, stackmap_size, registrations),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_safepoint_registration_definition(definition, registrations) => (
            3,
            registration_start(definition, stackmap_size, registrations)
                + SAFEPOINT_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::CallableRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_callable_registration_definition(definition, callable_registrations) => (
            3,
            callable_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_callable_registration_definition(definition, callable_registrations) => (
            3,
            callable_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
            ) + CALLABLE_REGISTRATION_SIZE,
        ),
        _ => unreachable!("fixture has only primary and stackmap atoms"),
    }
}

fn is_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
) -> bool {
    is_safepoint_registration_definition(definition, registrations)
        || is_callable_registration_definition(definition, callable_registrations)
}

fn is_safepoint_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongSafepointRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.definition_plan() == definition)
}

fn is_callable_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongCallableRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.definition_plan() == definition)
}

fn registration_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    registrations: &StrongSafepointRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.definition_plan() == definition)
        .unwrap();
    TEXT_SIZE
        + u64::try_from(stackmap_size).unwrap()
        + u64::try_from(index).unwrap() * SAFEPOINT_REGISTRATION_SIZE
}

fn callable_registration_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    registrations: &StrongCallableRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.definition_plan() == definition)
        .unwrap();
    TEXT_SIZE
        + u64::try_from(stackmap_size).unwrap()
        + u64::try_from(safepoints.registrations().len()).unwrap() * SAFEPOINT_REGISTRATION_SIZE
        + u64::try_from(index).unwrap() * CALLABLE_REGISTRATION_SIZE
}

fn push_registration(bytes: &mut Vec<u8>, registration: StrongSafepointRegistrationPlanV1) {
    push_u64(bytes, 0x5343_4f4f_5053_5054);
    push_u32(bytes, 1);
    push_u32(bytes, u32::try_from(SAFEPOINT_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.site().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, registration.safepoint().get());
    push_u32(bytes, registration.role().tag());
    push_u32(bytes, registration.root_pair_count());
    bytes.extend_from_slice(registration.owner().as_array());
    bytes.extend_from_slice(&[0; 32]);
}

fn push_callable_registration(bytes: &mut Vec<u8>, registration: StrongCallableRegistrationPlanV1) {
    push_u64(bytes, 0x5343_4f4f_5043_414c);
    push_u32(bytes, 1);
    push_u32(bytes, u32::try_from(CALLABLE_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.body().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, 0);
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
