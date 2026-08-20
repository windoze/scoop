use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn macho_object(
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
    push_sections(
        &mut bytes,
        module,
        stackmap,
        registrations,
        callable_registrations,
        type_registrations,
        immortal_registrations,
        static_storage_registrations,
        registration_offset,
        relocation_offset,
        storage_is_zero_fill,
        corruption,
    );
    push_relocations(
        &mut bytes,
        symbols,
        stackmap,
        registrations,
        callable_registrations,
        type_registrations,
        immortal_registrations,
        static_storage_registrations,
        local_symbol_count,
        static_local_symbol_count,
        registration_relocation_count,
        corruption,
    );
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
            [(registration.normalized_stackmap_patch(), record + 168)]
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
                [(registration.body_definition_patch(), record + 120)]
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
                    (registration.descriptor_definition_patch(), record + 144),
                    (registration.layout_fingerprint_patch(), record + 176),
                ]
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
                    (registration.scan_fingerprint_patch(), record + 168),
                    (registration.layout_fingerprint_patch(), record + 200),
                ]
            }),
    );
    patch_offsets.sort_unstable_by_key(|(intent, _)| *intent);
    ObjectFixture {
        bytes,
        patch_offsets,
    }
}
