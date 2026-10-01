use std::collections::BTreeMap;

use object::macho;
use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, DigestPatchIntentId, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    StrongCallableRegistrationPlanSetV1, StrongInitializationCallableRefPlanV1,
    StrongInitializationRegistrationSchedulePlanV1, StrongInitializationUnitRegistrationPlanSetV1,
    StrongInitializationUnitRegistrationPlanV1,
};

use crate::link_object::{PlannedMemberStrongObjectSymbolsV1, PlannedStrongObjectSymbolRoleV1};

use super::Corruption;

const COMMAND_BYTES: u32 = 72 + 4 * 80 + 24 + 80;
const TEXT_FILE_OFFSET: u32 = 32 + COMMAND_BYTES;

pub(super) struct ObjectFixture {
    pub(super) bytes: Vec<u8>,
    pub(super) patch_offsets: Vec<(DigestPatchIntentId, u64)>,
}

#[derive(Clone, Copy)]
struct AtomLocation {
    section: u8,
    start: u64,
    end: u64,
}

pub(super) fn object_bytes(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    plans: &StrongInitializationUnitRegistrationPlanSetV1,
    callable_plans: &StrongCallableRegistrationPlanSetV1,
    corruption: Corruption,
) -> ObjectFixture {
    let plan = &plans.registrations()[0];
    let callables = callable_refs(plan);
    let text_size = u64::try_from(callables.len()).unwrap() * 8;
    let readonly_base = text_size;
    let root_registration_base = 0;
    let callable_registration_base = 16;
    let registration_relative = align_to(
        callable_registration_base
            + u64::try_from(callable_plans.registrations().len()).unwrap() * 192,
        8,
    );
    let readonly_size = registration_relative + 352;
    let cstring_base = readonly_base + readonly_size;
    let diagnostic = plan
        .semantic()
        .diagnostic_path()
        .as_bytes()
        .iter()
        .copied()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let cstring_size = align_to(u64::try_from(diagnostic.len()).unwrap(), 8);
    let writable_base = cstring_base + cstring_size;
    let writable_size = 32;

    let mut locations = BTreeMap::new();
    for (index, callable) in callables.iter().enumerate() {
        insert_location(
            &mut locations,
            callable.body_primary_atom(),
            1,
            u64::try_from(index).unwrap() * 8,
            8,
        );
        insert_location(
            &mut locations,
            callable.registration_primary_atom(),
            2,
            readonly_base + callable_registration_relative(callable_plans, callable.body()),
            192,
        );
    }
    for (index, storage) in [plan.storage(), plan.failure_root()].iter().enumerate() {
        insert_location(
            &mut locations,
            storage.registration_primary_atom(),
            2,
            readonly_base + root_registration_base + u64::try_from(index).unwrap() * 8,
            8,
        );
        let definition = static_storage_definition(plans.producer(), storage.storage());
        insert_location(
            &mut locations,
            primary_atom(definition),
            4,
            writable_base + u64::try_from(index).unwrap() * 8,
            8,
        );
    }
    insert_location(
        &mut locations,
        plan.diagnostic_atom(),
        3,
        cstring_base,
        u64::try_from(diagnostic.len()).unwrap(),
    );
    insert_location(
        &mut locations,
        plan.registration_primary_atom(),
        2,
        readonly_base + registration_relative,
        352,
    );
    insert_location(
        &mut locations,
        plan.cell_primary_atom(),
        4,
        writable_base + 16,
        16,
    );

    let mut text = vec![0; usize::try_from(text_size).unwrap()];
    for index in 0..callables.len() {
        let offset = index * 8;
        text[offset..offset + 4].copy_from_slice(&0xd503_201f_u32.to_le_bytes());
        text[offset + 4..offset + 8].copy_from_slice(&0xd65f_03c0_u32.to_le_bytes());
    }
    let mut readonly = vec![0; usize::try_from(readonly_size).unwrap()];
    for callable in callable_plans.registrations() {
        let start = callable_registration_relative(callable_plans, callable.body());
        readonly[usize::try_from(start).unwrap()..usize::try_from(start + 192).unwrap()]
            .copy_from_slice(
                &crate::link_object::callable_registrations::record::expected_record(*callable),
            );
    }
    readonly[usize::try_from(registration_relative).unwrap()
        ..usize::try_from(registration_relative + 352).unwrap()]
        .copy_from_slice(&super::super::super::record::expected_record(plan));
    let mut cstring = vec![0; usize::try_from(cstring_size).unwrap()];
    cstring[..diagnostic.len()].copy_from_slice(&diagnostic);
    let mut writable = vec![0; usize::try_from(writable_size).unwrap()];
    match corruption {
        Corruption::CellByte => writable[16] ^= 1,
        Corruption::OldAbiVersion => {
            readonly[usize::try_from(registration_relative + 8).unwrap()] = 2;
        }
        Corruption::RegistrationByte => {
            readonly[usize::try_from(registration_relative).unwrap()] ^= 1;
        }
        Corruption::DiagnosticByte => {
            cstring[0] ^= 1;
        }
        Corruption::None | Corruption::StorageRegistrationTarget | Corruption::GatewayTarget => {}
    }

    let readonly_file_offset = u64::from(TEXT_FILE_OFFSET) + text_size;
    let cstring_file_offset = readonly_file_offset + readonly_size;
    let writable_file_offset = cstring_file_offset + cstring_size;
    let relocation_file_offset = writable_file_offset + writable_size;
    let relocations = relocations(
        symbols,
        plans,
        callable_plans,
        plan,
        registration_relative,
        corruption,
    );
    let symbol_file_offset = relocation_file_offset + u64::try_from(relocations.len()).unwrap() * 8;
    let (strings, string_indexes) = strings(symbols);
    let symbol_count = 1 + symbols.symbols().len();
    let string_file_offset = symbol_file_offset + u64::try_from(symbol_count).unwrap() * 16;

    let mut bytes = Vec::new();
    push_header(&mut bytes);
    push_segment(
        &mut bytes,
        text_size + readonly_size + cstring_size + writable_size,
    );
    push_section(
        &mut bytes,
        b"__text",
        b"__TEXT",
        0,
        text_size,
        u64::from(TEXT_FILE_OFFSET),
        3,
        0,
        0,
        macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
    );
    push_section(
        &mut bytes,
        b"__const",
        b"__DATA_CONST",
        readonly_base,
        readonly_size,
        readonly_file_offset,
        3,
        relocation_file_offset,
        u32::try_from(relocations.len()).unwrap(),
        macho::S_REGULAR,
    );
    push_section(
        &mut bytes,
        b"__cstring",
        b"__TEXT",
        cstring_base,
        cstring_size,
        cstring_file_offset,
        0,
        0,
        0,
        macho::S_CSTRING_LITERALS,
    );
    push_section(
        &mut bytes,
        b"__data",
        b"__DATA",
        writable_base,
        writable_size,
        writable_file_offset,
        3,
        0,
        0,
        macho::S_REGULAR,
    );
    push_symbol_commands(
        &mut bytes,
        symbol_file_offset,
        u32::try_from(symbol_count).unwrap(),
        string_file_offset,
        u32::try_from(strings.len()).unwrap(),
        u32::try_from(symbols.symbols().len()).unwrap(),
    );
    assert_eq!(bytes.len(), usize::try_from(TEXT_FILE_OFFSET).unwrap());
    bytes.extend_from_slice(&text);
    bytes.extend_from_slice(&readonly);
    bytes.extend_from_slice(&cstring);
    bytes.extend_from_slice(&writable);
    for (offset, symbol) in relocations {
        push_u32(&mut bytes, offset);
        push_u32(&mut bytes, symbol | 3 << 25 | 1 << 27);
    }
    push_u32(&mut bytes, 1);
    bytes.push(macho::N_SECT);
    bytes.push(3);
    push_u16(&mut bytes, 0);
    push_u64(&mut bytes, cstring_base);
    for (symbol, string_index) in symbols.symbols().iter().zip(string_indexes) {
        let location = match symbol.role() {
            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { primary_atom, .. }
            | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
                atom: primary_atom, ..
            } => locations[&primary_atom],
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
                atom: primary_atom, ..
            } => {
                let location = locations[&primary_atom];
                AtomLocation {
                    start: location.end,
                    ..location
                }
            }
        };
        push_u32(&mut bytes, string_index);
        bytes.push(macho::N_SECT | macho::N_EXT);
        bytes.push(location.section);
        push_u16(&mut bytes, 0);
        push_u64(&mut bytes, location.start);
    }
    bytes.extend_from_slice(&strings);

    let registration_file = readonly_file_offset + registration_relative;
    let mut patch_offsets = vec![(
        plan.registration_definition_patch(),
        registration_file + 120,
    )];
    if let Some(intent) = plan.schedule().gateway_definition_patch() {
        patch_offsets.push((intent, registration_file + 312));
    }
    for callable in callable_plans.registrations() {
        let callable_file =
            readonly_file_offset + callable_registration_relative(callable_plans, callable.body());
        patch_offsets.push((
            callable.registration_definition_patch(),
            callable_file + 120,
        ));
        patch_offsets.push((callable.body_definition_patch(), callable_file + 152));
    }
    patch_offsets.sort_unstable_by_key(|(intent, _)| *intent);
    ObjectFixture {
        bytes,
        patch_offsets,
    }
}

fn relocations(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    plans: &StrongInitializationUnitRegistrationPlanSetV1,
    callable_plans: &StrongCallableRegistrationPlanSetV1,
    plan: &StrongInitializationUnitRegistrationPlanV1,
    registration: u64,
    corruption: Corruption,
) -> Vec<(u32, u32)> {
    let local_diagnostic = 0;
    let cell = primary_symbol_index(symbols, plan.cell_definition_plan());
    let storage = primary_symbol_index(
        symbols,
        static_storage_definition(plans.producer(), plan.storage().storage()),
    );
    let initializer = primary_symbol_index(symbols, plan.initializer().body_definition_plan());
    let ensure = primary_symbol_index(symbols, plan.ensure().body_definition_plan());
    let mut items = vec![
        (registration + 160, local_diagnostic),
        (registration + 176, cell),
        (
            registration + 184,
            if matches!(corruption, Corruption::StorageRegistrationTarget) {
                storage
            } else {
                primary_symbol_index(symbols, plan.storage().registration_definition_plan())
            },
        ),
        (
            registration + 192,
            primary_symbol_index(symbols, plan.failure_root().registration_definition_plan()),
        ),
        (registration + 264, initializer),
        (registration + 272, ensure),
    ];
    if let StrongInitializationRegistrationSchedulePlanV1::EagerStartup { gateway, .. } =
        plan.schedule()
    {
        items.push((
            registration + 344,
            if matches!(corruption, Corruption::GatewayTarget) {
                initializer
            } else {
                primary_symbol_index(symbols, gateway.body_definition_plan())
            },
        ));
    }
    for callable in callable_plans.registrations() {
        items.push((
            callable_registration_relative(callable_plans, callable.body()) + 184,
            primary_symbol_index(symbols, callable.body_definition_plan()),
        ));
    }
    items
        .into_iter()
        .map(|(offset, symbol)| (u32::try_from(offset).unwrap(), symbol))
        .collect()
}

fn callable_registration_relative(
    plans: &StrongCallableRegistrationPlanSetV1,
    body: scoop_identity::PersistentCallableBodyId,
) -> u64 {
    let index = plans
        .registrations()
        .iter()
        .position(|plan| plan.body() == body)
        .expect("initialization callable has a callable registration");
    16 + u64::try_from(index).unwrap() * 192
}

fn callable_refs(
    plan: &StrongInitializationUnitRegistrationPlanV1,
) -> Vec<StrongInitializationCallableRefPlanV1> {
    let mut callables = vec![plan.initializer(), plan.ensure()];
    if let Some(gateway) = plan.schedule().gateway() {
        callables.push(*gateway);
    }
    callables
}

fn static_storage_definition(
    producer: scoop_identity::ConeIdentity,
    storage: scoop_identity::PersistentStaticStorageId,
) -> ObjectDefinitionPlanId {
    ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::static_storage(storage),
            StrongDefinitionRole::StaticStorage,
        )
        .unwrap(),
    )
    .unwrap()
}

fn primary_atom(definition: ObjectDefinitionPlanId) -> ObjectDefinitionAtomId {
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}

fn insert_location(
    locations: &mut BTreeMap<ObjectDefinitionAtomId, AtomLocation>,
    atom: ObjectDefinitionAtomId,
    section: u8,
    start: u64,
    size: u64,
) {
    assert!(
        locations
            .insert(
                atom,
                AtomLocation {
                    section,
                    start,
                    end: start + size,
                },
            )
            .is_none()
    );
}

fn primary_symbol_index(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    definition: ObjectDefinitionPlanId,
) -> u32 {
    1 + u32::try_from(
        symbols
            .symbols()
            .iter()
            .position(|symbol| {
                matches!(
                    symbol.role(),
                    PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                        definition: actual,
                        ..
                    } if actual == definition
                )
            })
            .unwrap(),
    )
    .unwrap()
}

fn strings(symbols: &PlannedMemberStrongObjectSymbolsV1) -> (Vec<u8>, Vec<u32>) {
    let mut strings = vec![0];
    strings.extend_from_slice(b"diagnostic\0");
    let indexes = symbols
        .symbols()
        .iter()
        .map(|symbol| {
            let index = u32::try_from(strings.len()).unwrap();
            strings.extend_from_slice(symbol.macho_name());
            strings.push(0);
            index
        })
        .collect();
    (strings, indexes)
}

fn push_header(bytes: &mut Vec<u8>) {
    push_u32(bytes, macho::MH_MAGIC_64);
    push_u32(bytes, macho::CPU_TYPE_ARM64);
    push_u32(bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(bytes, macho::MH_OBJECT);
    push_u32(bytes, 3);
    push_u32(bytes, COMMAND_BYTES);
    push_u32(bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(bytes, 0);
}

fn push_segment(bytes: &mut Vec<u8>, byte_size: u64) {
    push_u32(bytes, macho::LC_SEGMENT_64);
    push_u32(bytes, 72 + 4 * 80);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(bytes, 0);
    push_u64(bytes, byte_size);
    push_u64(bytes, u64::from(TEXT_FILE_OFFSET));
    push_u64(bytes, byte_size);
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
    size: u64,
    offset: u64,
    alignment: u32,
    relocation_offset: u64,
    relocation_count: u32,
    flags: u32,
) {
    push_fixed_name(bytes, section);
    push_fixed_name(bytes, segment);
    push_u64(bytes, address);
    push_u64(bytes, size);
    push_u32(bytes, u32::try_from(offset).unwrap());
    push_u32(bytes, alignment);
    push_u32(bytes, u32::try_from(relocation_offset).unwrap());
    push_u32(bytes, relocation_count);
    push_u32(bytes, flags);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
}

fn push_symbol_commands(
    bytes: &mut Vec<u8>,
    symbol_offset: u64,
    symbol_count: u32,
    string_offset: u64,
    string_size: u32,
    defined_count: u32,
) {
    push_u32(bytes, macho::LC_SYMTAB);
    push_u32(bytes, 24);
    push_u32(bytes, u32::try_from(symbol_offset).unwrap());
    push_u32(bytes, symbol_count);
    push_u32(bytes, u32::try_from(string_offset).unwrap());
    push_u32(bytes, string_size);
    push_u32(bytes, macho::LC_DYSYMTAB);
    push_u32(bytes, 80);
    push_u32(bytes, 0);
    push_u32(bytes, 1);
    push_u32(bytes, 1);
    push_u32(bytes, defined_count);
    push_u32(bytes, 1 + defined_count);
    push_u32(bytes, 0);
    bytes.extend_from_slice(&[0; 48]);
}

fn push_fixed_name(bytes: &mut Vec<u8>, name: &[u8]) {
    bytes.extend_from_slice(name);
    bytes.extend(std::iter::repeat_n(0, 16 - name.len()));
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

const fn align_to(value: u64, alignment: u64) -> u64 {
    (value + alignment - 1) & !(alignment - 1)
}
