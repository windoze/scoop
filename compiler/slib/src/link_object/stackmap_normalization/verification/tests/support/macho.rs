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

const TEXT_SIZE: u64 = 16;
const STACK_SIZE: u64 = 64;
const SAFEPOINT_REGISTRATION_SIZE: u64 = 232;
const CALLABLE_REGISTRATION_SIZE: u64 = 208;
const TYPE_DESCRIPTOR_SIZE: u64 = 152;
const LAYOUT_SIZE: u64 = 8;
const TYPE_REGISTRATION_SIZE: u64 = 240;
const IMMORTAL_REGISTRATION_SIZE: u64 = 184;
const STATIC_LAYOUT_SIZE: u64 = 8;
const STATIC_STORAGE_REGISTRATION_SIZE: u64 = 296;
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
        | Corruption::StaticSentinelCollision
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

#[allow(clippy::too_many_arguments)]
fn macho_object(
    module: &scoop_lir::Module,
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    instructions: &[u32; 4],
    stackmap: &[u8],
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
    type_registrations: &StrongTypeRegistrationPlanSetV1,
    immortal_registrations: &StrongImmortalObjectRegistrationPlanSetV1,
    static_storage_registrations: &StrongStaticStorageRegistrationPlanSetV1,
    corruption: Corruption,
) -> ObjectFixture {
    let segment_size = 72_u32 + 4 * 80;
    let command_bytes = segment_size + 24 + 80;
    let text_offset = 32 + command_bytes;
    let stackmap_offset = text_offset + u32::try_from(TEXT_SIZE).unwrap();
    let stackmap_size = u32::try_from(stackmap.len()).unwrap();
    let registration_offset = stackmap_offset + stackmap_size;
    let registration_size = u32::try_from(
        registrations.registrations().len() * usize::try_from(SAFEPOINT_REGISTRATION_SIZE).unwrap()
            + callable_registrations.registrations().len()
                * usize::try_from(CALLABLE_REGISTRATION_SIZE).unwrap()
            + type_registrations.registrations().len()
                * usize::try_from(TYPE_DESCRIPTOR_SIZE + LAYOUT_SIZE + TYPE_REGISTRATION_SIZE)
                    .unwrap()
            + usize::try_from(type_diagnostic_region_size(type_registrations)).unwrap()
            + immortal_registrations
                .registrations()
                .iter()
                .map(|registration| {
                    usize::try_from(registration.object_size() + IMMORTAL_REGISTRATION_SIZE)
                        .unwrap()
                })
                .sum::<usize>()
            + usize::from(!static_storage_registrations.registrations().is_empty())
                * usize::try_from(STATIC_EMPTY_SENTINELS_SIZE).unwrap()
            + static_storage_registrations
                .registrations()
                .iter()
                .map(|registration| {
                    usize::try_from(static_readonly_group_size(registration)).unwrap()
                })
                .sum::<usize>(),
    )
    .unwrap();
    let writable_offset = registration_offset + registration_size;
    let writable_size = u32::try_from(
        static_storage_registrations
            .registrations()
            .iter()
            .map(|registration| {
                usize::try_from(registration.semantic().allocation_extent()).unwrap()
            })
            .sum::<usize>(),
    )
    .unwrap();
    let storage_is_zero_fill = if matches!(corruption, Corruption::StaticZeroedWritableSection) {
        false
    } else if matches!(corruption, Corruption::StaticEncodedZeroFillSection) {
        true
    } else {
        !static_storage_registrations.registrations().is_empty()
            && static_storage_registrations
                .registrations()
                .iter()
                .all(|registration| {
                    matches!(
                        registration.initial_artifacts(),
                        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
                    )
                })
    };
    let writable_file_size = if storage_is_zero_fill {
        0
    } else {
        writable_size
    };
    let relocation_offset = writable_offset + writable_file_size;
    let registration_relocation_count = u32::try_from(callable_registrations.registrations().len())
        .unwrap()
        + 2 * u32::try_from(type_registrations.registrations().len()).unwrap()
        + 3 * u32::try_from(immortal_registrations.registrations().len()).unwrap()
        + static_storage_registrations
            .registrations()
            .iter()
            .map(|registration| {
                4 + u32::try_from(
                    registration
                        .semantic()
                        .initial_state()
                        .immortal_relocations()
                        .len(),
                )
                .unwrap()
            })
            .sum::<u32>()
        + u32::from(matches!(corruption, Corruption::RelocatedRegistration));
    let writable_relocation_count = static_storage_registrations
        .registrations()
        .iter()
        .map(|registration| {
            u32::try_from(
                registration
                    .semantic()
                    .initial_state()
                    .immortal_relocations()
                    .len(),
            )
            .unwrap()
        })
        .sum::<u32>();
    let symbol_offset =
        relocation_offset + 8 * (2 + registration_relocation_count + writable_relocation_count);
    let external_type = immortal_registrations
        .registrations()
        .iter()
        .find(|registration| {
            matches!(
                registration.semantic().type_registration_ref(),
                scoop_lir::ImmortalObjectTypeRegistrationRefV1::DependencyExternal { .. }
            )
        })
        .map(|registration| registration.type_registration());
    let external_type_symbols = external_type.map(|exact_type| {
        let normalization = scoop_lir::LirTargetProfile::DARWIN_AARCH64
            .contract()
            .native_symbol_normalization();
        let descriptor = PersistentSymbolRequest::new(
            PersistentSymbolKey::TypeDescriptor(exact_type),
            LinkageClass::ConeStrong,
        )
        .unwrap();
        let registration = PersistentSymbolRequest::new(
            PersistentSymbolKey::TypeRegistration(exact_type),
            LinkageClass::ConeStrong,
        )
        .unwrap();
        [
            normalization
                .compiler_generated_object_symbol(descriptor.symbol().as_str())
                .into_bytes(),
            normalization
                .compiler_generated_object_symbol(registration.symbol().as_str())
                .into_bytes(),
        ]
    });
    let undefined_symbol_count = external_type_symbols
        .as_ref()
        .map_or(0, |symbols| symbols.len());
    let static_local_symbol_names = static_storage_registrations
        .registrations()
        .iter()
        .enumerate()
        .flat_map(|(index, _)| {
            [
                format!("static-template-{index}").into_bytes(),
                format!("static-relocations-{index}").into_bytes(),
            ]
        })
        .collect::<Vec<_>>();
    let static_local_symbol_count = static_local_symbol_names.len();
    let local_symbol_names = static_local_symbol_names
        .into_iter()
        .chain(
            type_registrations
                .registrations()
                .iter()
                .enumerate()
                .map(|(index, _)| format!("type-diagnostic-{index}").into_bytes()),
        )
        .collect::<Vec<_>>();
    let local_symbol_count = local_symbol_names.len();
    let symbol_bytes =
        u32::try_from((local_symbol_count + symbols.symbols().len() + undefined_symbol_count) * 16)
            .unwrap();
    let string_offset = symbol_offset + symbol_bytes;
    let mut strings = vec![0];
    let local_string_indexes = local_symbol_names
        .iter()
        .map(|symbol| {
            let index = u32::try_from(strings.len()).unwrap();
            strings.extend_from_slice(symbol);
            strings.push(0);
            index
        })
        .collect::<Vec<_>>();
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
    let external_string_indexes = external_type_symbols.as_ref().map(|symbols| {
        symbols.each_ref().map(|symbol| {
            let index = u32::try_from(strings.len()).unwrap();
            strings.extend_from_slice(symbol);
            strings.push(0);
            index
        })
    });
    let string_size = u32::try_from(strings.len()).unwrap();
    let mut bytes = Vec::with_capacity((string_offset + string_size) as usize);

    push_header(&mut bytes, command_bytes);
    push_segment(
        &mut bytes,
        segment_size,
        text_offset,
        stackmap_size,
        registration_size,
        writable_size,
        writable_file_size,
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
    let (storage_section, storage_segment, storage_offset, storage_flags) =
        if matches!(corruption, Corruption::WritableRegistrationSection) {
            (
                b"__const".as_slice(),
                b"__DATA_CONST".as_slice(),
                writable_offset,
                macho::S_REGULAR,
            )
        } else if storage_is_zero_fill {
            (
                b"__bss".as_slice(),
                b"__DATA".as_slice(),
                0,
                macho::S_ZEROFILL,
            )
        } else {
            (
                b"__data".as_slice(),
                b"__DATA".as_slice(),
                writable_offset,
                macho::S_REGULAR,
            )
        };
    push_section(
        &mut bytes,
        storage_section,
        storage_segment,
        TEXT_SIZE + u64::from(stackmap_size) + u64::from(registration_size),
        writable_size,
        storage_offset,
        3,
        if writable_relocation_count == 0 {
            0
        } else {
            relocation_offset + 16 + 8 * registration_relocation_count
        },
        writable_relocation_count,
        storage_flags,
    );
    push_symbol_commands(
        &mut bytes,
        local_symbol_count,
        symbols.symbols().len(),
        undefined_symbol_count,
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
    for registration in type_registrations.registrations() {
        push_type_descriptor(&mut bytes, registration);
    }
    if matches!(corruption, Corruption::TypeDescriptorScalar) {
        let descriptor_offset = usize::try_from(
            type_descriptor_base(stackmap.len(), registrations, callable_registrations)
                - TEXT_SIZE
                - u64::try_from(stackmap.len()).unwrap(),
        )
        .unwrap()
            + usize::try_from(registration_offset).unwrap();
        bytes[descriptor_offset + 16] ^= 1;
    }
    for registration in type_registrations.registrations() {
        bytes.extend_from_slice(registration.semantic().diagnostic_name().as_bytes());
    }
    if matches!(corruption, Corruption::TypeDescriptorDiagnosticBytes) {
        let diagnostic_offset = usize::try_from(
            type_diagnostic_base(
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
            ) - TEXT_SIZE
                - u64::try_from(stackmap.len()).unwrap(),
        )
        .unwrap()
            + usize::try_from(registration_offset).unwrap();
        bytes[diagnostic_offset] ^= 1;
    }
    align_zero(&mut bytes, 8);
    bytes.extend(std::iter::repeat_n(
        0,
        type_registrations.registrations().len() * usize::try_from(LAYOUT_SIZE).unwrap(),
    ));
    for registration in type_registrations.registrations() {
        push_type_registration(&mut bytes, registration);
    }
    let first_immortal_object_offset = bytes.len();
    for registration in immortal_registrations.registrations() {
        push_immortal_object(&mut bytes, module, *registration);
    }
    if matches!(corruption, Corruption::ImmortalObjectLength) {
        bytes[first_immortal_object_offset + 16..first_immortal_object_offset + 24]
            .copy_from_slice(&u64::MAX.to_le_bytes());
    }
    for registration in immortal_registrations.registrations() {
        push_immortal_registration(&mut bytes, *registration);
    }
    if !static_storage_registrations.registrations().is_empty() {
        bytes.extend_from_slice(&[0; STATIC_EMPTY_SENTINELS_SIZE as usize]);
    }
    for registration in static_storage_registrations.registrations() {
        bytes.extend_from_slice(&[0; STATIC_LAYOUT_SIZE as usize]);
        push_scan_program(&mut bytes, registration);
        if let StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table,
            ..
        } = registration.initial_artifacts()
        {
            bytes.extend_from_slice(registration.semantic().initial_state().initial_template());
            if matches!(
                relocation_table,
                scoop_lir::StaticStorageRelocationTableArtifactV1::Defined { .. }
            ) {
                for relocation in registration
                    .semantic()
                    .initial_state()
                    .immortal_relocations()
                {
                    push_u64(&mut bytes, relocation.pointer_offset());
                    push_u64(&mut bytes, 0);
                }
            }
        }
        push_static_storage_registration(&mut bytes, registration);
    }
    if !storage_is_zero_fill {
        for registration in static_storage_registrations.registrations() {
            let initial = registration.semantic().initial_state().initial_template();
            bytes.extend_from_slice(initial);
            bytes.resize(
                bytes.len() + usize::try_from(registration.semantic().allocation_extent()).unwrap()
                    - initial.len(),
                0,
            );
        }
    }
    assert_eq!(bytes.len(), usize::try_from(relocation_offset).unwrap());
    if matches!(corruption, Corruption::RegistrationMagic) {
        bytes[usize::try_from(registration_offset).unwrap()] ^= 1;
    }
    if matches!(corruption, Corruption::CallableRegistrationMagic) {
        let offset = u64::from(registration_offset)
            + u64::try_from(registrations.registrations().len()).unwrap()
                * SAFEPOINT_REGISTRATION_SIZE;
        bytes[usize::try_from(offset).unwrap()] ^= 1;
    }
    if matches!(corruption, Corruption::TypeRegistrationMagic) {
        let offset = type_registration_base(
            stackmap.len(),
            registrations,
            callable_registrations,
            type_registrations,
        );
        bytes[usize::try_from(
            u64::from(registration_offset) - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap()
                + offset,
        )
        .unwrap()] ^= 1;
    }
    if matches!(corruption, Corruption::ImmortalRegistrationMagic) {
        let offset = immortal_registration_base(
            stackmap.len(),
            registrations,
            callable_registrations,
            type_registrations,
            immortal_registrations,
        );
        bytes[usize::try_from(
            u64::from(registration_offset) - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap()
                + offset,
        )
        .unwrap()] ^= 1;
    }
    if matches!(
        corruption,
        Corruption::StaticRegistrationMagic | Corruption::StaticScanProgram
    ) {
        let registration = &static_storage_registrations.registrations()[0];
        let virtual_offset = if matches!(corruption, Corruption::StaticRegistrationMagic) {
            static_registration_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            )
        } else {
            static_scan_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            )
        };
        let file_offset = u64::from(registration_offset) + virtual_offset
            - TEXT_SIZE
            - u64::try_from(stackmap.len()).unwrap();
        bytes[usize::try_from(file_offset).unwrap()] ^= 1;
    }
    let planned_index = |index: usize| u32::try_from(local_symbol_count + index).unwrap();
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
        planned_index(target)
            | 1 << 24
            | 2 << 25
            | 1 << 27
            | u32::from(macho::ARM64_RELOC_BRANCH26) << 28,
    );
    push_u32(&mut bytes, 16);
    push_u32(&mut bytes, planned_index(target) | 3 << 25 | 1 << 27);
    if registration_relocation_count != 0 {
        if matches!(corruption, Corruption::RelocatedRegistration) {
            push_u32(&mut bytes, 56);
            push_u32(&mut bytes, planned_index(target) | 3 << 25 | 1 << 27);
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
                planned_index(relocation_target) | 3 << 25 | 1 << 27,
            );
        }
        for (index, registration) in type_registrations.registrations().iter().enumerate() {
            let descriptor_start = type_descriptor_start(
                registration.descriptor_definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
            );
            push_u32(
                &mut bytes,
                u32::try_from(
                    descriptor_start - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap() + 112,
                )
                .unwrap(),
            );
            let diagnostic_target = if matches!(
                corruption,
                Corruption::TypeDescriptorDiagnosticRelocationTarget
            ) && index == 0
            {
                symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::TypeDescriptor,
                                definition,
                                ..
                            } if definition == registration.descriptor_definition_plan()
                        )
                    })
                    .map(planned_index)
                    .unwrap()
            } else {
                u32::try_from(static_local_symbol_count + index).unwrap()
            };
            push_u32(&mut bytes, diagnostic_target | 3 << 25 | 1 << 27);
        }
        for (index, registration) in type_registrations.registrations().iter().enumerate() {
            let type_start = type_registration_start(
                registration.definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
            );
            push_u32(
                &mut bytes,
                u32::try_from(
                    type_start - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap() + 168,
                )
                .unwrap(),
            );
            let descriptor_target =
                if matches!(corruption, Corruption::TypeDescriptorRelocationTarget) && index == 0 {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::TypeRegistration,
                                    ..
                                }
                            )
                        })
                        .unwrap()
                } else {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::TypeDescriptor,
                                    definition,
                                    ..
                                } if definition == registration.descriptor_definition_plan()
                            )
                        })
                        .unwrap()
                };
            push_u32(
                &mut bytes,
                planned_index(descriptor_target) | 3 << 25 | 1 << 27,
            );
        }
        for (index, registration) in immortal_registrations.registrations().iter().enumerate() {
            let object_start = immortal_object_start(
                registration.object_definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            );
            let record_start = immortal_registration_start(
                registration.registration_definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            );
            let section_base = TEXT_SIZE + u64::try_from(stackmap.len()).unwrap();
            let descriptor_target = if matches!(
                corruption,
                Corruption::ImmortalObjectDescriptorRelocationTarget
            ) && index == 0
            {
                symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::ImmortalRegistration,
                                ..
                            }
                        )
                    })
                    .unwrap()
            } else if matches!(
                registration.semantic().type_registration_ref(),
                scoop_lir::ImmortalObjectTypeRegistrationRefV1::DependencyExternal { .. }
            ) {
                symbols.symbols().len()
            } else {
                symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::TypeDescriptor,
                                ..
                            }
                        )
                    })
                    .unwrap()
            };
            push_u32(
                &mut bytes,
                u32::try_from(object_start - section_base).unwrap(),
            );
            push_u32(
                &mut bytes,
                planned_index(descriptor_target) | 3 << 25 | 1 << 27,
            );

            let object_target =
                if matches!(corruption, Corruption::ImmortalObjectRelocationTarget) && index == 0 {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::ImmortalRegistration,
                                    ..
                                }
                            )
                        })
                        .unwrap()
                } else {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::ImmortalObject,
                                    definition,
                                    ..
                                } if definition == registration.object_definition_plan()
                            )
                        })
                        .unwrap()
                };
            push_u32(
                &mut bytes,
                u32::try_from(record_start - section_base + 152).unwrap(),
            );
            push_u32(&mut bytes, planned_index(object_target) | 3 << 25 | 1 << 27);

            let type_target = if matches!(
                registration.semantic().type_registration_ref(),
                scoop_lir::ImmortalObjectTypeRegistrationRefV1::DependencyExternal { .. }
            ) {
                symbols.symbols().len() + 1
            } else if matches!(
                corruption,
                Corruption::ImmortalTypeRegistrationRelocationTarget
            ) && index == 0
            {
                object_target
            } else {
                symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::TypeRegistration,
                                ..
                            }
                        )
                    })
                    .unwrap()
            };
            push_u32(
                &mut bytes,
                u32::try_from(record_start - section_base + 176).unwrap(),
            );
            push_u32(&mut bytes, planned_index(type_target) | 3 << 25 | 1 << 27);
        }
        for (index, registration) in static_storage_registrations
            .registrations()
            .iter()
            .enumerate()
        {
            let section_base = TEXT_SIZE + u64::try_from(stackmap.len()).unwrap();
            let record_start = static_registration_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            );
            let storage_target = symbols
                .symbols()
                .iter()
                .position(|symbol| {
                    if matches!(corruption, Corruption::StaticStorageRelocationTarget) && index == 0
                    {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition,
                                definition_role: StrongDefinitionRole::RootRegistration,
                                ..
                            } if definition == registration.registration_definition_plan()
                        )
                    } else {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition,
                                definition_role: StrongDefinitionRole::StaticStorage,
                                ..
                            } if definition == registration.storage_definition_plan()
                        )
                    }
                })
                .unwrap();
            let scan_target = symbols
                .symbols()
                .iter()
                .position(|symbol| {
                    matches!(
                        symbol.role(),
                        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                            definition,
                            definition_role: StrongDefinitionRole::ScanProgram,
                            ..
                        } if definition == registration.scan_definition_plan()
                    )
                })
                .unwrap();
            for (offset, target) in [
                (160, planned_index(storage_target)),
                (192, planned_index(scan_target)),
                (264, u32::try_from(index * 2).unwrap()),
                (
                    280,
                    u32::try_from(
                        if matches!(corruption, Corruption::StaticSentinelCollision) {
                            index * 2
                        } else {
                            index * 2 + 1
                        },
                    )
                    .unwrap(),
                ),
            ] {
                push_u32(
                    &mut bytes,
                    u32::try_from(record_start - section_base + offset).unwrap(),
                );
                push_u32(&mut bytes, target | 3 << 25 | 1 << 27);
            }
            let relocation_start = static_relocation_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            );
            for (relocation_index, relocation) in registration
                .semantic()
                .initial_state()
                .immortal_relocations()
                .iter()
                .enumerate()
            {
                let immortal_registration_definition = immortal_registrations
                    .registrations()
                    .iter()
                    .find(|immortal| immortal.object() == relocation.target())
                    .unwrap()
                    .registration_definition_plan();
                let immortal_target = symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        if matches!(corruption, Corruption::StaticInitialTableRelocationTarget)
                            && index == 0
                            && relocation_index == 0
                        {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::ImmortalObject,
                                    ..
                                }
                            )
                        } else {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition,
                                    definition_role: StrongDefinitionRole::ImmortalRegistration,
                                    ..
                                } if definition == immortal_registration_definition
                            )
                        }
                    })
                    .unwrap();
                push_u32(
                    &mut bytes,
                    u32::try_from(
                        relocation_start - section_base
                            + u64::try_from(relocation_index).unwrap() * 16
                            + 8,
                    )
                    .unwrap(),
                );
                push_u32(
                    &mut bytes,
                    planned_index(immortal_target) | 3 << 25 | 1 << 27,
                );
            }
        }
    }
    let mut storage_base = 0_u64;
    for (static_index, registration) in static_storage_registrations
        .registrations()
        .iter()
        .enumerate()
    {
        for relocation in registration
            .semantic()
            .initial_state()
            .immortal_relocations()
        {
            let immortal_object_definition = immortal_registrations
                .registrations()
                .iter()
                .find(|immortal| immortal.object() == relocation.target())
                .unwrap()
                .object_definition_plan();
            let immortal_target = symbols
                .symbols()
                .iter()
                .position(|symbol| {
                    if matches!(corruption, Corruption::StaticInitialStorageRelocationTarget)
                        && static_index == 0
                    {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::ImmortalRegistration,
                                ..
                            }
                        )
                    } else {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition,
                                definition_role: StrongDefinitionRole::ImmortalObject,
                                ..
                            } if definition == immortal_object_definition
                        )
                    }
                })
                .unwrap();
            push_u32(
                &mut bytes,
                u32::try_from(storage_base + relocation.pointer_offset()).unwrap(),
            );
            push_u32(
                &mut bytes,
                planned_index(immortal_target) | 3 << 25 | 1 << 27,
            );
        }
        storage_base += registration.semantic().allocation_extent();
    }
    let (static_local_string_indexes, diagnostic_local_string_indexes) =
        local_string_indexes.split_at(static_local_symbol_count);
    for ((string_index, registration), local_kind) in static_local_string_indexes
        .iter()
        .zip(
            static_storage_registrations
                .registrations()
                .iter()
                .flat_map(|registration| [registration, registration]),
        )
        .zip([0_u8, 1].into_iter().cycle())
    {
        let value = if local_kind == 0 {
            static_template_target_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            )
        } else {
            static_relocation_target_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            )
        };
        push_u32(&mut bytes, *string_index);
        bytes.push(macho::N_SECT);
        bytes.push(3);
        push_u16(&mut bytes, 0);
        push_u64(&mut bytes, value);
    }
    for (string_index, registration) in diagnostic_local_string_indexes
        .iter()
        .zip(type_registrations.registrations())
    {
        push_u32(&mut bytes, *string_index);
        bytes.push(macho::N_SECT);
        bytes.push(3);
        push_u16(&mut bytes, 0);
        push_u64(
            &mut bytes,
            type_diagnostic_start(
                registration.descriptor_definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
            ),
        );
    }
    for (symbol, string_index) in symbols.symbols().iter().zip(string_indexes) {
        let (section, value) = symbol_location(
            symbol.role(),
            stackmap.len(),
            registrations,
            callable_registrations,
            type_registrations,
            immortal_registrations,
            static_storage_registrations,
        );
        push_u32(&mut bytes, string_index);
        bytes.push(macho::N_SECT | macho::N_EXT);
        bytes.push(section);
        push_u16(&mut bytes, 0);
        push_u64(&mut bytes, value);
    }
    if let Some(string_indexes) = external_string_indexes {
        for string_index in string_indexes {
            push_u32(&mut bytes, string_index);
            bytes.push(macho::N_UNDF | macho::N_EXT);
            bytes.push(0);
            push_u16(&mut bytes, 0);
            push_u64(&mut bytes, 0);
        }
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
    let type_base =
        u64::from(registration_offset) - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap()
            + type_registration_base(
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
            );
    patch_offsets.extend(
        type_registrations
            .registrations()
            .iter()
            .enumerate()
            .flat_map(|(index, registration)| {
                let record = type_base + u64::try_from(index).unwrap() * TYPE_REGISTRATION_SIZE;
                [
                    (registration.registration_definition_patch(), record + 120),
                    (registration.descriptor_definition_patch(), record + 176),
                    (registration.layout_fingerprint_patch(), record + 208),
                ]
            }),
    );
    let immortal_base =
        u64::from(registration_offset) - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap()
            + immortal_registration_base(
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            );
    patch_offsets.extend(
        immortal_registrations
            .registrations()
            .iter()
            .enumerate()
            .map(|(index, registration)| {
                let record =
                    immortal_base + u64::try_from(index).unwrap() * IMMORTAL_REGISTRATION_SIZE;
                (registration.registration_definition_patch(), record + 120)
            }),
    );
    let readonly_section_base = TEXT_SIZE + u64::try_from(stackmap.len()).unwrap();
    patch_offsets.extend(
        static_storage_registrations
            .registrations()
            .iter()
            .flat_map(|registration| {
                let record = u64::from(registration_offset)
                    + static_registration_start(
                        registration,
                        stackmap.len(),
                        registrations,
                        callable_registrations,
                        type_registrations,
                        immortal_registrations,
                        static_storage_registrations,
                    )
                    - readonly_section_base;
                [
                    (registration.registration_definition_patch(), record + 120),
                    (registration.scan_fingerprint_patch(), record + 200),
                    (registration.layout_fingerprint_patch(), record + 232),
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
    writable_virtual_size: u32,
    writable_file_size: u32,
) {
    push_u32(bytes, macho::LC_SEGMENT_64);
    push_u32(bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(bytes, 0);
    push_u64(
        bytes,
        TEXT_SIZE
            + u64::from(stackmap_size)
            + u64::from(registration_size)
            + u64::from(writable_virtual_size),
    );
    push_u64(bytes, u64::from(text_offset));
    push_u64(
        bytes,
        TEXT_SIZE
            + u64::from(stackmap_size)
            + u64::from(registration_size)
            + u64::from(writable_file_size),
    );
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 4);
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
    local_symbol_count: usize,
    defined_symbol_count: usize,
    undefined_symbol_count: usize,
    symbol_offset: u32,
    string_offset: u32,
    string_size: u32,
) {
    let local_symbol_count = u32::try_from(local_symbol_count).unwrap();
    let defined_symbol_count = u32::try_from(defined_symbol_count).unwrap();
    let undefined_symbol_count = u32::try_from(undefined_symbol_count).unwrap();
    let symbol_count = local_symbol_count + defined_symbol_count + undefined_symbol_count;
    push_u32(bytes, macho::LC_SYMTAB);
    push_u32(bytes, 24);
    push_u32(bytes, symbol_offset);
    push_u32(bytes, symbol_count);
    push_u32(bytes, string_offset);
    push_u32(bytes, string_size);

    push_u32(bytes, macho::LC_DYSYMTAB);
    push_u32(bytes, 80);
    push_u32(bytes, 0);
    push_u32(bytes, local_symbol_count);
    push_u32(bytes, local_symbol_count);
    push_u32(bytes, defined_symbol_count);
    push_u32(bytes, local_symbol_count + defined_symbol_count);
    push_u32(bytes, undefined_symbol_count);
    bytes.extend_from_slice(&[0; 48]);
}

fn symbol_location(
    role: PlannedStrongObjectSymbolRoleV1,
    stackmap_size: usize,
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
    type_registrations: &StrongTypeRegistrationPlanSetV1,
    immortal_registrations: &StrongImmortalObjectRegistrationPlanSetV1,
    static_storage_registrations: &StrongStaticStorageRegistrationPlanSetV1,
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
        } if is_callable_body_definition(definition, callable_registrations) => (1, 0),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            atom_role: DefinitionAtomRole::Primary,
            definition,
            ..
        } if is_callable_body_definition(definition, callable_registrations) => (1, TEXT_SIZE),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Stackmap | DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_callable_body_definition(definition, callable_registrations) => (2, TEXT_SIZE),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Stackmap | DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_callable_body_definition(definition, callable_registrations) => {
            (2, TEXT_SIZE + u64::try_from(stackmap_size).unwrap())
        }
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
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::TypeDescriptor,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_type_descriptor_definition(definition, type_registrations) => (
            3,
            type_descriptor_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_type_descriptor_definition(definition, type_registrations) => (
            3,
            type_descriptor_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ) + TYPE_DESCRIPTOR_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_type_descriptor_definition(definition, type_registrations) => (
            3,
            type_diagnostic_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_type_descriptor_definition(definition, type_registrations) => (
            3,
            type_diagnostic_end(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::Layout,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_layout_definition(definition, type_registrations) => (
            3,
            layout_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_layout_definition(definition, type_registrations) => (
            3,
            layout_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ) + LAYOUT_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::TypeRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_type_registration_definition(definition, type_registrations) => (
            3,
            type_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_type_registration_definition(definition, type_registrations) => (
            3,
            type_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ) + TYPE_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::ImmortalObject,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_immortal_object_definition(definition, immortal_registrations) => (
            3,
            immortal_object_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_immortal_object_definition(definition, immortal_registrations) => (
            3,
            immortal_object_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            ) + immortal_registrations
                .registrations()
                .iter()
                .find(|registration| registration.object_definition_plan() == definition)
                .unwrap()
                .object_size(),
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::ImmortalRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_immortal_registration_definition(definition, immortal_registrations) => (
            3,
            immortal_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_immortal_registration_definition(definition, immortal_registrations) => (
            3,
            immortal_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            ) + IMMORTAL_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::StaticStorage,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            4,
            static_storage_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            4,
            static_storage_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + static_storage_registrations
                .registrations()
                .iter()
                .find(|registration| registration.storage_definition_plan() == definition)
                .unwrap()
                .semantic()
                .allocation_extent(),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            3,
            static_template_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            3,
            static_template_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + static_storage_registrations
                .registrations()
                .iter()
                .find(|registration| registration.storage_definition_plan() == definition)
                .map(static_template_size)
                .unwrap(),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::RuntimeRecord,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            3,
            static_relocation_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::RuntimeRecord,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            3,
            static_relocation_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + static_storage_registrations
                .registrations()
                .iter()
                .find(|registration| registration.storage_definition_plan() == definition)
                .map(static_relocation_size)
                .unwrap(),
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::RootRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_registration_definition(definition, static_storage_registrations) => (
            3,
            static_registration_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_registration_definition(definition, static_storage_registrations) => (
            3,
            static_registration_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + STATIC_STORAGE_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::Layout,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_layout_definition(definition, static_storage_registrations) => (
            3,
            static_layout_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_layout_definition(definition, static_storage_registrations) => (
            3,
            static_layout_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + STATIC_LAYOUT_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::ScanProgram,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_scan_definition(definition, static_storage_registrations) => (
            3,
            static_scan_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_scan_definition(definition, static_storage_registrations) => (
            3,
            static_scan_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + static_storage_registrations
                .registrations()
                .iter()
                .find(|registration| registration.scan_definition_plan() == definition)
                .map(static_scan_size)
                .unwrap(),
        ),
        _ => unreachable!("fixture has only primary and stackmap atoms"),
    }
}

fn is_callable_body_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongCallableRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.body_definition_plan() == definition)
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

fn is_type_descriptor_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.descriptor_definition_plan() == definition)
}

fn is_layout_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.layout_definition_plan() == definition)
}

fn is_type_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.definition_plan() == definition)
}

fn is_immortal_object_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.object_definition_plan() == definition)
}

fn is_immortal_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.registration_definition_plan() == definition)
}

fn is_static_storage_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.storage_definition_plan() == definition)
}

fn is_static_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.registration_definition_plan() == definition)
}

fn is_static_layout_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.layout_definition_plan() == definition)
}

fn is_static_scan_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.scan_definition_plan() == definition)
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

fn type_descriptor_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
) -> u64 {
    TEXT_SIZE
        + u64::try_from(stackmap_size).unwrap()
        + u64::try_from(safepoints.registrations().len()).unwrap() * SAFEPOINT_REGISTRATION_SIZE
        + u64::try_from(callables.registrations().len()).unwrap() * CALLABLE_REGISTRATION_SIZE
}

fn type_descriptor_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.descriptor_definition_plan() == definition)
        .unwrap();
    type_descriptor_base(stackmap_size, safepoints, callables)
        + u64::try_from(index).unwrap() * TYPE_DESCRIPTOR_SIZE
}

fn layout_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    type_diagnostic_base(stackmap_size, safepoints, callables, registrations)
        + type_diagnostic_region_size(registrations)
}

fn type_diagnostic_region_size(registrations: &StrongTypeRegistrationPlanSetV1) -> u64 {
    let size = registrations
        .registrations()
        .iter()
        .map(|registration| u64::try_from(registration.semantic().diagnostic_name().len()).unwrap())
        .sum::<u64>();
    (size + 7) & !7
}

fn type_diagnostic_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    type_descriptor_base(stackmap_size, safepoints, callables)
        + u64::try_from(registrations.registrations().len()).unwrap() * TYPE_DESCRIPTOR_SIZE
}

fn type_diagnostic_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.descriptor_definition_plan() == definition)
        .unwrap();
    type_diagnostic_base(stackmap_size, safepoints, callables, registrations)
        + registrations.registrations()[..index]
            .iter()
            .map(|registration| {
                u64::try_from(registration.semantic().diagnostic_name().len()).unwrap()
            })
            .sum::<u64>()
}

fn type_diagnostic_end(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let registration = registrations
        .registrations()
        .iter()
        .find(|registration| registration.descriptor_definition_plan() == definition)
        .unwrap();
    type_diagnostic_start(
        definition,
        stackmap_size,
        safepoints,
        callables,
        registrations,
    ) + u64::try_from(registration.semantic().diagnostic_name().len()).unwrap()
}

fn layout_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.layout_definition_plan() == definition)
        .unwrap();
    layout_base(stackmap_size, safepoints, callables, registrations)
        + u64::try_from(index).unwrap() * LAYOUT_SIZE
}

fn type_registration_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    layout_base(stackmap_size, safepoints, callables, registrations)
        + u64::try_from(registrations.registrations().len()).unwrap() * LAYOUT_SIZE
}

fn type_registration_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.definition_plan() == definition)
        .unwrap();
    type_registration_base(stackmap_size, safepoints, callables, registrations)
        + u64::try_from(index).unwrap() * TYPE_REGISTRATION_SIZE
}

fn immortal_object_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    type_registration_base(stackmap_size, safepoints, callables, types)
        + u64::try_from(types.registrations().len()).unwrap() * TYPE_REGISTRATION_SIZE
}

fn immortal_object_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.object_definition_plan() == definition)
        .unwrap();
    immortal_object_base(stackmap_size, safepoints, callables, types)
        + registrations.registrations()[..index]
            .iter()
            .map(|registration| registration.object_size())
            .sum::<u64>()
}

fn immortal_registration_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> u64 {
    immortal_object_base(stackmap_size, safepoints, callables, types)
        + registrations
            .registrations()
            .iter()
            .map(|registration| registration.object_size())
            .sum::<u64>()
}

fn immortal_registration_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.registration_definition_plan() == definition)
        .unwrap();
    immortal_registration_base(stackmap_size, safepoints, callables, types, registrations)
        + u64::try_from(index).unwrap() * IMMORTAL_REGISTRATION_SIZE
}

fn static_sentinel_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
) -> u64 {
    immortal_registration_base(stackmap_size, safepoints, callables, types, immortals)
        + u64::try_from(immortals.registrations().len()).unwrap() * IMMORTAL_REGISTRATION_SIZE
}

fn static_readonly_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_sentinel_base(stackmap_size, safepoints, callables, types, immortals)
        + u64::from(!statics.registrations().is_empty()) * STATIC_EMPTY_SENTINELS_SIZE
}

fn static_scan_size(registration: &StrongStaticStorageRegistrationPlanV1) -> u64 {
    match registration.semantic().scan_program() {
        scoop_lir::RefScan::None => 8,
        scoop_lir::RefScan::References(offsets) => (u64::try_from(offsets.len()).unwrap() + 1) * 8,
        scoop_lir::RefScan::Sequence(_) | scoop_lir::RefScan::Array { .. } => {
            panic!("fixture cannot contain non-value scans")
        }
    }
}

fn static_template_size(registration: &StrongStaticStorageRegistrationPlanV1) -> u64 {
    match registration.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => 0,
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue { .. } => u64::try_from(
            registration
                .semantic()
                .initial_state()
                .initial_template()
                .len(),
        )
        .unwrap(),
    }
}

fn static_relocation_size(registration: &StrongStaticStorageRegistrationPlanV1) -> u64 {
    match registration.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: scoop_lir::StaticStorageRelocationTableArtifactV1::Defined { .. },
            ..
        } => {
            u64::try_from(
                registration
                    .semantic()
                    .initial_state()
                    .immortal_relocations()
                    .len(),
            )
            .unwrap()
                * 16
        }
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
        | StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: scoop_lir::StaticStorageRelocationTableArtifactV1::SharedEmptySentinel,
            ..
        } => 0,
    }
}

fn static_readonly_group_size(registration: &StrongStaticStorageRegistrationPlanV1) -> u64 {
    STATIC_LAYOUT_SIZE
        + static_scan_size(registration)
        + static_template_size(registration)
        + static_relocation_size(registration)
        + STATIC_STORAGE_REGISTRATION_SIZE
}

fn static_readonly_group_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    let index = statics
        .registrations()
        .iter()
        .position(|candidate| candidate.semantic().storage() == registration.semantic().storage())
        .unwrap();
    static_readonly_base(
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + statics.registrations()[..index]
        .iter()
        .map(static_readonly_group_size)
        .sum::<u64>()
}

fn static_layout_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_readonly_group_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    )
}

fn static_scan_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_layout_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + STATIC_LAYOUT_SIZE
}

fn static_template_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_scan_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + static_scan_size(registration)
}

fn static_relocation_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_template_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + static_template_size(registration)
}

fn static_registration_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_relocation_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + static_relocation_size(registration)
}

fn static_template_target_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    match registration.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => {
            static_sentinel_base(stackmap_size, safepoints, callables, types, immortals)
        }
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue { .. } => {
            static_template_start(
                registration,
                stackmap_size,
                safepoints,
                callables,
                types,
                immortals,
                statics,
            )
        }
    }
}

fn static_relocation_target_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    match registration.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: scoop_lir::StaticStorageRelocationTableArtifactV1::Defined { .. },
            ..
        } => static_relocation_start(
            registration,
            stackmap_size,
            safepoints,
            callables,
            types,
            immortals,
            statics,
        ),
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
        | StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: scoop_lir::StaticStorageRelocationTableArtifactV1::SharedEmptySentinel,
            ..
        } => static_sentinel_base(stackmap_size, safepoints, callables, types, immortals) + 8,
    }
}

fn static_registration_for_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
    select: impl Fn(&StrongStaticStorageRegistrationPlanV1) -> ObjectDefinitionPlanId,
) -> &StrongStaticStorageRegistrationPlanV1 {
    registrations
        .registrations()
        .iter()
        .find(|registration| select(registration) == definition)
        .unwrap()
}

macro_rules! static_start_for_definition {
    ($name:ident, $select:ident, $start:ident) => {
        fn $name(
            definition: ObjectDefinitionPlanId,
            stackmap_size: usize,
            safepoints: &StrongSafepointRegistrationPlanSetV1,
            callables: &StrongCallableRegistrationPlanSetV1,
            types: &StrongTypeRegistrationPlanSetV1,
            immortals: &StrongImmortalObjectRegistrationPlanSetV1,
            statics: &StrongStaticStorageRegistrationPlanSetV1,
        ) -> u64 {
            let registration =
                static_registration_for_definition(definition, statics, |registration| {
                    registration.$select()
                });
            $start(
                registration,
                stackmap_size,
                safepoints,
                callables,
                types,
                immortals,
                statics,
            )
        }
    };
}

static_start_for_definition!(
    static_template_start_for_definition,
    storage_definition_plan,
    static_template_start
);
static_start_for_definition!(
    static_relocation_start_for_definition,
    storage_definition_plan,
    static_relocation_start
);
static_start_for_definition!(
    static_registration_start_for_definition,
    registration_definition_plan,
    static_registration_start
);
static_start_for_definition!(
    static_layout_start_for_definition,
    layout_definition_plan,
    static_layout_start
);
static_start_for_definition!(
    static_scan_start_for_definition,
    scan_definition_plan,
    static_scan_start
);

fn static_storage_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    let index = statics
        .registrations()
        .iter()
        .position(|registration| registration.storage_definition_plan() == definition)
        .unwrap();
    static_readonly_base(
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + statics
        .registrations()
        .iter()
        .map(static_readonly_group_size)
        .sum::<u64>()
        + statics.registrations()[..index]
            .iter()
            .map(|registration| registration.semantic().allocation_extent())
            .sum::<u64>()
}

fn push_registration(bytes: &mut Vec<u8>, registration: StrongSafepointRegistrationPlanV1) {
    push_u64(bytes, 0x5343_4f4f_5053_5054);
    push_u32(bytes, 4);
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
    push_u32(bytes, 4);
    push_u32(bytes, u32::try_from(CALLABLE_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.body().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, registration.context_key_count());
}

fn push_type_registration(bytes: &mut Vec<u8>, registration: &StrongTypeRegistrationPlanV1) {
    push_u64(bytes, 0x5343_4f4f_5054_5950);
    push_u32(bytes, 4);
    push_u32(bytes, u32::try_from(TYPE_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.exact_type().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, registration.runtime_type().get());
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
}

fn push_type_descriptor(bytes: &mut Vec<u8>, registration: &StrongTypeRegistrationPlanV1) {
    let shape = registration.semantic().instance_shape();
    push_u64(bytes, registration.runtime_type().get());
    push_u32(bytes, shape.instance_kind().tag());
    push_u32(bytes, shape.inline_storage_kind().tag());
    push_u64(bytes, shape.minimum_size());
    push_u64(bytes, shape.instance_alignment());
    push_u64(bytes, shape.inline_offset());
    push_u64(bytes, shape.inline_size());
    push_u64(bytes, shape.inline_stride());
    push_u64(bytes, shape.inline_alignment());
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(
        bytes,
        u64::try_from(registration.semantic().itables().len()).unwrap(),
    );
    push_u64(bytes, 0);
    push_u64(
        bytes,
        u64::try_from(registration.semantic().diagnostic_name().len()).unwrap(),
    );
    assert_eq!(registration.semantic().relations().runtime_kind(), 0);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
}

fn push_immortal_registration(
    bytes: &mut Vec<u8>,
    registration: StrongImmortalObjectRegistrationPlanV1,
) {
    push_u64(bytes, 0x5343_4f4f_5049_4d4d);
    push_u32(bytes, 4);
    push_u32(bytes, u32::try_from(IMMORTAL_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.object().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, 0);
    push_u64(bytes, registration.object_size());
    push_u64(bytes, registration.required_alignment());
    push_u64(bytes, 0);
}

fn push_static_storage_registration(
    bytes: &mut Vec<u8>,
    registration: &StrongStaticStorageRegistrationPlanV1,
) {
    let semantic = registration.semantic();
    push_u64(bytes, 0x5343_4f4f_5053_544f);
    push_u32(bytes, 4);
    push_u32(
        bytes,
        u32::try_from(STATIC_STORAGE_REGISTRATION_SIZE).unwrap(),
    );
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(semantic.storage().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u32(bytes, semantic.scan_kind().tag());
    push_u32(bytes, semantic.initial_state().tag());
    push_u64(bytes, 0);
    push_u64(bytes, semantic.byte_size());
    push_u64(bytes, semantic.allocation_extent());
    push_u64(bytes, semantic.required_alignment());
    push_u64(bytes, 0);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, 0);
    push_u64(
        bytes,
        u64::try_from(semantic.initial_state().initial_template().len()).unwrap(),
    );
    push_u64(bytes, 0);
    push_u64(
        bytes,
        u64::try_from(semantic.initial_state().immortal_relocations().len()).unwrap(),
    );
}

fn push_scan_program(bytes: &mut Vec<u8>, registration: &StrongStaticStorageRegistrationPlanV1) {
    match registration.semantic().scan_program() {
        scoop_lir::RefScan::None => push_u64(bytes, 0),
        scoop_lir::RefScan::References(offsets) => {
            push_u64(bytes, u64::try_from(offsets.len()).unwrap());
            for offset in offsets {
                push_u64(bytes, *offset);
            }
        }
        scoop_lir::RefScan::Sequence(_) | scoop_lir::RefScan::Array { .. } => {
            panic!("fixture cannot contain non-value scans")
        }
    }
}

fn push_immortal_object(
    bytes: &mut Vec<u8>,
    module: &scoop_lir::Module,
    registration: StrongImmortalObjectRegistrationPlanV1,
) {
    let (identity, value) = module
        .globals
        .iter()
        .find_map(|(_, global)| match &global.init {
            scoop_lir::GlobalInit::StringConst { identity, value }
                if identity.identity_record().id() == registration.object() =>
            {
                Some((identity, value))
            }
            scoop_lir::GlobalInit::StringConst { .. }
            | scoop_lir::GlobalInit::Storage { .. }
            | scoop_lir::GlobalInit::CString { .. }
            | scoop_lir::GlobalInit::RawStorage { .. }
            | scoop_lir::GlobalInit::ImportedStorage { .. } => None,
        })
        .expect("immortal registration must resolve to a StringConst global");
    assert_eq!(identity.identity_record().id(), registration.object());
    let start = bytes.len();
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, u64::try_from(value.len()).unwrap());
    bytes.extend_from_slice(value.as_bytes());
    bytes.resize(
        start + usize::try_from(registration.object_size()).unwrap(),
        0,
    );
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
