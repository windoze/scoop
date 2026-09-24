use std::collections::BTreeMap;

use object::macho;
use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeImageSupportRole, DefinitionAtomRole,
    DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentExactTypeId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CBridgeProductionSetV1, CanonicalLirFoundation, ConeImagePlanV1, EntryProductionPlanV1,
    GeneratedBridgePlanSetV1, LirTargetProfile, OdrFreeLirFoundation,
    StrongDigestFinalizationPlanV1, StrongObjectSymbolSurfaceV1, StrongProducerUnitPartitionV1,
    StrongRegistrationIdentitySurfaceV1,
};

use super::finalization::patch_image_bytes_for_test;
use super::record::{expected_final_image_record, expected_image_record};
use super::*;
use crate::link_object::c_bridge_production::tests::profile;
use crate::link_object::{
    CanonicalScoopLirObjectUnitSetV1, PlannedLinkObjectMemberSetV1,
    PlannedMemberStrongObjectSymbolsV1, PlannedStrongObjectSymbolRoleV1,
    PlannedStrongObjectSymbolSetV1, ProvisionalDigestPatchSiteV1, ScoopLirObjectCandidateV1,
    verify_builtin_object_strong_relocations_v1, verify_c_bridge_production_envelopes_v1,
    verify_scoop_lir_digest_patch_sites_v1,
};

const MINIMUM_OS: u32 = 0x000d_0100;
const SDK: u32 = 0x000e_0200;

#[derive(Clone, Copy)]
enum Corruption {
    None,
    CoordinateByte,
    CoordinateTarget,
    StaticSentinel,
    ArrayBoundsMessageByte,
    ArraySizeOverflowMessageByte,
}

#[test]
fn verifies_the_unique_image_record_support_atoms_and_relocations() {
    let fixture = fixture(Corruption::None, false);
    let verified = verify_cone_image_v1(
        fixture.patch_sites,
        fixture.plan.clone(),
        &[ScoopLirObjectCandidateV1::new(
            fixture.member,
            &fixture.bytes,
        )],
    )
    .unwrap();

    assert_eq!(verified.producer(), fixture.plan.cone().identity());
    assert_eq!(verified.member(), fixture.member);
    assert_eq!(verified.primary().byte_size(), 240);
    assert_eq!(verified.support().coordinate_group().byte_size(), 5);
    assert_eq!(verified.support().dependencies().byte_size(), 32);
    assert_eq!(verified.support().static_storages().byte_size(), 8);
    assert_eq!(verified.support().array_bounds_message().byte_size(), 26);
    assert_eq!(
        verified.support().array_size_overflow_message().byte_size(),
        20
    );
    assert_eq!(verified.support_relocations().len(), 10);
    assert!(verified.registration_relocations().is_empty());
    assert_eq!(verified.image_patch().offset_within_atom(), 96);
}

#[test]
fn rejects_changed_coordinate_bytes() {
    let fixture = fixture(Corruption::CoordinateByte, false);
    assert!(matches!(
        verify_cone_image_v1(
            fixture.patch_sites,
            fixture.plan,
            &[ScoopLirObjectCandidateV1::new(
                fixture.member,
                &fixture.bytes
            )],
        ),
        Err(ConeImageValidationError::AtomByteMismatch {
            role: ConeImageAtomRoleV1::CoordinateGroup,
            offset_within_atom: 0,
            ..
        })
    ));
}

#[test]
fn rejects_changed_array_trap_message_bytes() {
    for (corruption, role) in [
        (
            Corruption::ArrayBoundsMessageByte,
            ConeImageAtomRoleV1::ArrayBoundsMessage,
        ),
        (
            Corruption::ArraySizeOverflowMessageByte,
            ConeImageAtomRoleV1::ArraySizeOverflowMessage,
        ),
    ] {
        let fixture = fixture(corruption, false);
        assert!(matches!(
            verify_cone_image_v1(
                fixture.patch_sites,
                fixture.plan,
                &[ScoopLirObjectCandidateV1::new(
                    fixture.member,
                    &fixture.bytes
                )],
            ),
            Err(ConeImageValidationError::AtomByteMismatch {
                role: actual,
                offset_within_atom: 0,
                ..
            }) if actual == role
        ));
    }
}

#[test]
fn rejects_a_coordinate_pointer_to_another_support_atom() {
    let fixture = fixture(Corruption::CoordinateTarget, false);
    assert_eq!(
        verify_cone_image_v1(
            fixture.patch_sites,
            fixture.plan,
            &[ScoopLirObjectCandidateV1::new(
                fixture.member,
                &fixture.bytes
            )],
        ),
        Err(ConeImageValidationError::RelocationMismatch {
            role: ConeImageAtomRoleV1::CoordinateGroup,
            index: 0,
            kind: ConeImageRelocationFailureV1::TargetAtom,
        })
    );
}

#[test]
fn rejects_a_nonzero_empty_table_sentinel() {
    let fixture = fixture(Corruption::StaticSentinel, false);
    assert!(matches!(
        verify_cone_image_v1(
            fixture.patch_sites,
            fixture.plan,
            &[ScoopLirObjectCandidateV1::new(
                fixture.member,
                &fixture.bytes
            )],
        ),
        Err(ConeImageValidationError::AtomByteMismatch {
            role: ConeImageAtomRoleV1::StaticStorages,
            offset_within_atom: 0,
            ..
        })
    ));
}

#[test]
fn verifies_a_nonempty_registration_table_and_strong_target() {
    let fixture = fixture(Corruption::None, true);
    let verified = verify_cone_image_v1(
        fixture.patch_sites,
        fixture.plan,
        &[ScoopLirObjectCandidateV1::new(
            fixture.member,
            &fixture.bytes,
        )],
    )
    .unwrap();

    assert_eq!(verified.registration_relocations().len(), 1);
    assert_eq!(
        verified.registration_relocations()[0].containing_atom(),
        verified.support().type_registrations().atom()
    );
}

#[test]
fn patches_only_the_verified_runtime_image_slot_and_rechecks_the_record() {
    let mut fixture = fixture(Corruption::None, false);
    let verified = verify_cone_image_v1(
        fixture.patch_sites,
        fixture.plan.clone(),
        &[ScoopLirObjectCandidateV1::new(
            fixture.member,
            &fixture.bytes,
        )],
    )
    .unwrap();
    let fingerprint = [0xa5; 32];
    let expected = expected_final_image_record(&fixture.plan, &fingerprint);

    patch_image_bytes_for_test(
        &mut fixture.bytes,
        verified.primary().checked_offset(),
        verified.image_patch(),
        &fingerprint,
        &expected,
    )
    .unwrap();

    let start = usize::try_from(verified.image_patch().checked_offset()).unwrap();
    assert_eq!(&fixture.bytes[start..start + 32], &fingerprint);
    assert!(matches!(
        patch_image_bytes_for_test(
            &mut fixture.bytes,
            verified.primary().checked_offset(),
            verified.image_patch(),
            &fingerprint,
            &expected,
        ),
        Err(RuntimeImagePatchError::NonZeroPatchSlot { offset: 0 })
    ));
}

#[test]
fn library_entry_proof_accepts_the_complete_object_set_without_root_artifacts() {
    let fixture = fixture(Corruption::None, false);
    let verified = crate::link_object::verify_entry_production_v1(
        fixture.patch_sites,
        EntryProductionPlanV1::Library,
        &[ScoopLirObjectCandidateV1::new(
            fixture.member,
            &fixture.bytes,
        )],
    )
    .unwrap();

    assert!(matches!(
        verified.branch(),
        crate::link_object::VerifiedEntryProductionBranchV1::Library
    ));
}

struct Fixture {
    plan: ConeImagePlanV1,
    patch_sites: crate::link_object::VerifiedScoopLirDigestPatchSiteSetV1,
    member: crate::SlibMemberId,
    bytes: Vec<u8>,
}

fn fixture(corruption: Corruption, with_type_registration: bool) -> Fixture {
    let coordinate = ConeCoordinate::reserved_single_file();
    let producer = coordinate.identity().unwrap();
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::cone_image(producer),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let definition_id = definition.id();
    let mut definitions = vec![definition];
    let mut atoms = image_atoms(definition_id);
    let image_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::ImageDescriptor(producer),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut symbol_requests = vec![image_symbol];
    let type_registration = with_type_registration.then(|| {
        let exact = unit_exact_type();
        let definition = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                producer,
                StrongDefinitionEntity::exact_type(exact),
                StrongDefinitionRole::TypeRegistration,
            )
            .unwrap(),
        )
        .unwrap();
        let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            definition.id(),
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap();
        let symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::TypeRegistration(exact),
            LinkageClass::ConeStrong,
        )
        .unwrap();
        definitions.push(definition.clone());
        atoms.push(atom.clone());
        symbol_requests.push(symbol);
        (definition.id(), atom.id())
    });
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(definitions).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbol_requests).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();

    let image_key = DigestNodeKey::runtime_image(producer);
    let image_node_id = DigestNodeId::from_key(&image_key).unwrap();
    let image_patch = DigestPatchIntentKey::new(
        image_node_id,
        definition_id,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let registration_node = type_registration.map(|(definition, _)| {
        scoop_lir::DigestNodeV1::new(
            DigestNodeKey::strong_registration(definition),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    });
    let image_inputs = registration_node
        .iter()
        .map(scoop_lir::DigestInputRefV1::from_node)
        .collect();
    let image_node =
        scoop_lir::DigestNodeV1::new(image_key, image_inputs, vec![image_patch]).unwrap();
    let mut nodes = vec![image_node];
    nodes.extend(registration_node);
    let digest_plan = StrongDigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();
    let plan = ConeImagePlanV1::new(
        coordinate,
        &[scoop_identity::ConeIdentity::CORE],
        &foundation,
        &registrations,
        &digest_plan,
    )
    .unwrap();

    let bridge_plan = GeneratedBridgePlanSetV1::from_odr_free_foundation(&foundation).unwrap();
    let partition = StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let member_plan = PlannedLinkObjectMemberSetV1::new(
        &partition,
        vec![
            CanonicalScoopLirObjectUnitSetV1::new(
                std::iter::once(definition_id)
                    .chain(type_registration.map(|(definition, _)| definition))
                    .collect(),
            )
            .unwrap(),
        ],
        Vec::new(),
    )
    .unwrap();
    let surface = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
        LirTargetProfile::DARWIN_AARCH64,
        &surface,
        &member_plan,
    )
    .unwrap();
    let member = member_plan.scoop_lir_members()[0].member_id();
    let object = image_object(
        symbol_plan.member(member).unwrap(),
        &plan,
        type_registration,
        corruption,
    );
    let objects = [ScoopLirObjectCandidateV1::new(member, &object.bytes)];

    let c_profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &c_profile);
    let production = verify_c_bridge_production_envelopes_v1(
        bridge_plan.clone(),
        production,
        &c_profile,
        &member_plan,
        &[],
    )
    .unwrap();
    let builtins = verify_builtin_object_strong_relocations_v1(
        &member_plan,
        &symbol_plan,
        &objects,
        production,
        &[],
    )
    .unwrap();
    let provisional = [ProvisionalDigestPatchSiteV1::new(
        plan.fingerprint_patch(),
        member,
        u64::try_from(object.section_offset).unwrap() + 96,
        32,
    )];
    let patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        builtins,
        &foundation,
        digest_plan,
        &objects,
        &provisional,
    )
    .unwrap();
    Fixture {
        plan,
        patch_sites,
        member,
        bytes: object.bytes,
    }
}

fn image_atoms(
    plan: ObjectDefinitionPlanId,
) -> Vec<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>> {
    let mut keys = vec![ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    )];
    keys.extend(
        support_roles().map(|(atom_role, support)| image_support_key(plan, atom_role, support)),
    );
    keys.into_iter()
        .map(|key| CborIdentityRecord::from_key(key).unwrap())
        .collect()
}

fn support_roles() -> impl Iterator<Item = (DefinitionAtomRole, ConeImageSupportRole)> {
    [
        (
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::CoordinateGroup,
        ),
        (
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::CoordinateName,
        ),
        (
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::CoordinateVersion,
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::Dependencies,
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::StaticStorages,
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::ImmortalObjects,
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::InitializationUnits,
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::TypeRegistrations,
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::Safepoints,
        ),
        (
            DefinitionAtomRole::RuntimeRecord,
            ConeImageSupportRole::Callables,
        ),
        (
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::ArrayBoundsMessage,
        ),
        (
            DefinitionAtomRole::AddressTakenConstant,
            ConeImageSupportRole::ArraySizeOverflowMessage,
        ),
    ]
    .into_iter()
}

fn image_support_key(
    plan: ObjectDefinitionPlanId,
    atom_role: DefinitionAtomRole,
    support: ConeImageSupportRole,
) -> ObjectDefinitionAtomKey {
    ObjectDefinitionAtomKey::new(
        plan,
        atom_role,
        DefinitionAtomSubkey::ConeImageSupport(support),
    )
}

struct ObjectFixture {
    bytes: Vec<u8>,
    section_offset: usize,
}

fn image_object(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    plan: &ConeImagePlanV1,
    type_registration: Option<(ObjectDefinitionPlanId, ObjectDefinitionAtomId)>,
    corruption: Corruption,
) -> ObjectFixture {
    let support = plan.support_atoms();
    let locations = image_locations(plan, type_registration.map(|(_, atom)| atom));
    let section_size = locations.values().map(|(_, end)| *end).max().unwrap();
    let mut section = vec![0; usize::try_from(section_size).unwrap()];
    section[..240].copy_from_slice(&expected_image_record(plan));
    copy_atom_bytes(
        &mut section,
        locations[&support.coordinate_group()].0,
        plan.cone().coordinate().group().as_bytes(),
    );
    copy_atom_bytes(
        &mut section,
        locations[&support.coordinate_name()].0,
        plan.cone().coordinate().name().as_bytes(),
    );
    copy_atom_bytes(
        &mut section,
        locations[&support.coordinate_version()].0,
        plan.cone().coordinate().version().as_bytes(),
    );
    let dependency_bytes = if plan.dependencies().is_empty() {
        vec![0; 32]
    } else {
        plan.dependencies()
            .iter()
            .flat_map(|identity| identity.as_array().iter().copied())
            .collect()
    };
    copy_atom_bytes(
        &mut section,
        locations[&support.dependencies()].0,
        &dependency_bytes,
    );
    copy_atom_bytes(
        &mut section,
        locations[&support.array_bounds_message()].0,
        b"array index out of bounds\0",
    );
    copy_atom_bytes(
        &mut section,
        locations[&support.array_size_overflow_message()].0,
        b"array size overflow\0",
    );
    match corruption {
        Corruption::CoordinateByte => {
            section[usize::try_from(locations[&support.coordinate_group()].0).unwrap()] ^= 1;
        }
        Corruption::StaticSentinel => {
            section[usize::try_from(locations[&support.static_storages()].0).unwrap()] = 1;
        }
        Corruption::ArrayBoundsMessageByte => {
            section[usize::try_from(locations[&support.array_bounds_message()].0).unwrap()] ^= 1;
        }
        Corruption::ArraySizeOverflowMessageByte => {
            section
                [usize::try_from(locations[&support.array_size_overflow_message()].0).unwrap()] ^=
                1;
        }
        Corruption::None | Corruption::CoordinateTarget => {}
    }

    let local_atoms = [
        support.coordinate_group(),
        support.coordinate_name(),
        support.coordinate_version(),
        support.dependencies(),
        support.static_storages(),
        support.immortal_objects(),
        support.initialization_units(),
        support.type_registrations(),
        support.safepoints(),
        support.callables(),
        support.array_bounds_message(),
        support.array_size_overflow_message(),
    ];
    let mut relocation_targets = vec![0_u32, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    if matches!(corruption, Corruption::CoordinateTarget) {
        relocation_targets[0] = 1;
    }
    let mut relocation_offsets = vec![16_u32, 32, 48, 128, 144, 160, 176, 192, 208, 224];
    if let Some((definition, _)) = type_registration {
        relocation_offsets.push(u32::try_from(locations[&support.type_registrations()].0).unwrap());
        let symbol_index = symbols
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
            .unwrap();
        relocation_targets
            .push(u32::try_from(local_atoms.len()).unwrap() + u32::try_from(symbol_index).unwrap());
    }
    build_macho(
        symbols,
        &locations,
        &local_atoms,
        &relocation_offsets,
        &relocation_targets,
        section,
    )
}

pub(crate) fn empty_image_object_for_link_decode_test(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    plan: &ConeImagePlanV1,
) -> (Vec<u8>, u64) {
    let object = image_object(symbols, plan, None, Corruption::None);
    (
        object.bytes,
        u64::try_from(object.section_offset).unwrap() + 96,
    )
}

fn image_locations(
    plan: &ConeImagePlanV1,
    type_registration: Option<ObjectDefinitionAtomId>,
) -> BTreeMap<ObjectDefinitionAtomId, (u64, u64)> {
    let support = plan.support_atoms();
    let mut locations = BTreeMap::new();
    locations.insert(plan.primary_atom(), (0, 240));
    let mut cursor = 240;
    for (atom, size, alignment) in [
        (
            support.coordinate_group(),
            plan.cone().coordinate().group().len() as u64,
            1,
        ),
        (
            support.coordinate_name(),
            plan.cone().coordinate().name().len() as u64,
            1,
        ),
        (
            support.coordinate_version(),
            plan.cone().coordinate().version().len() as u64,
            1,
        ),
        (support.dependencies(), 32, 1),
        (support.static_storages(), 8, 8),
        (support.immortal_objects(), 8, 8),
        (support.initialization_units(), 8, 8),
        (support.type_registrations(), 8, 8),
        (support.safepoints(), 8, 8),
        (support.callables(), 8, 8),
        (support.array_bounds_message(), 26, 1),
        (support.array_size_overflow_message(), 20, 1),
    ] {
        cursor = align_to(cursor, alignment);
        locations.insert(atom, (cursor, cursor + size));
        cursor += size;
    }
    if let Some(atom) = type_registration {
        cursor = align_to(cursor, 8);
        locations.insert(atom, (cursor, cursor + 8));
    }
    locations
}

fn build_macho(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    locations: &BTreeMap<ObjectDefinitionAtomId, (u64, u64)>,
    local_atoms: &[ObjectDefinitionAtomId],
    relocation_offsets: &[u32],
    relocation_targets: &[u32],
    section: Vec<u8>,
) -> ObjectFixture {
    const SEGMENT_SIZE: u32 = 152;
    const SYMTAB_SIZE: u32 = 24;
    const DYSYMTAB_SIZE: u32 = 80;
    let command_bytes = SEGMENT_SIZE + SYMTAB_SIZE + DYSYMTAB_SIZE;
    let section_offset = 32 + command_bytes;
    let section_size = u32::try_from(section.len()).unwrap();
    let relocation_offset = section_offset + section_size;
    let relocation_count = u32::try_from(relocation_offsets.len()).unwrap();
    let symbol_offset = relocation_offset + relocation_count * 8;
    let local_count = u32::try_from(local_atoms.len()).unwrap();
    let external_count = u32::try_from(symbols.symbols().len()).unwrap();
    let symbol_count = local_count + external_count;
    let string_offset = symbol_offset + symbol_count * 16;

    let mut strings = vec![0];
    let mut local_string_indexes = Vec::new();
    for index in 0..local_atoms.len() {
        local_string_indexes.push(u32::try_from(strings.len()).unwrap());
        strings.extend_from_slice(format!("image.local.{index}").as_bytes());
        strings.push(0);
    }
    let external_string_indexes = symbols
        .symbols()
        .iter()
        .map(|symbol| {
            let index = u32::try_from(strings.len()).unwrap();
            strings.extend_from_slice(symbol.macho_name());
            strings.push(0);
            index
        })
        .collect::<Vec<_>>();

    let mut bytes = Vec::new();
    push_u32(&mut bytes, macho::MH_MAGIC_64);
    push_u32(&mut bytes, macho::CPU_TYPE_ARM64);
    push_u32(&mut bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(&mut bytes, macho::MH_OBJECT);
    push_u32(&mut bytes, 3);
    push_u32(&mut bytes, command_bytes);
    push_u32(&mut bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SEGMENT_64);
    push_u32(&mut bytes, SEGMENT_SIZE);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, u64::from(section_size));
    push_u64(&mut bytes, u64::from(section_offset));
    push_u64(&mut bytes, u64::from(section_size));
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);
    push_fixed_name(&mut bytes, b"__const");
    push_fixed_name(&mut bytes, b"__DATA_CONST");
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, u64::from(section_size));
    push_u32(&mut bytes, section_offset);
    push_u32(&mut bytes, 3);
    push_u32(&mut bytes, relocation_offset);
    push_u32(&mut bytes, relocation_count);
    push_u32(&mut bytes, macho::S_REGULAR);
    bytes.extend_from_slice(&[0; 12]);

    push_u32(&mut bytes, macho::LC_SYMTAB);
    push_u32(&mut bytes, SYMTAB_SIZE);
    push_u32(&mut bytes, symbol_offset);
    push_u32(&mut bytes, symbol_count);
    push_u32(&mut bytes, string_offset);
    push_u32(&mut bytes, u32::try_from(strings.len()).unwrap());

    push_u32(&mut bytes, macho::LC_DYSYMTAB);
    push_u32(&mut bytes, DYSYMTAB_SIZE);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, local_count);
    push_u32(&mut bytes, local_count);
    push_u32(&mut bytes, external_count);
    push_u32(&mut bytes, symbol_count);
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(&[0; 48]);
    assert_eq!(bytes.len(), usize::try_from(section_offset).unwrap());
    bytes.extend_from_slice(&section);

    for (offset, target) in relocation_offsets.iter().zip(relocation_targets) {
        push_u32(&mut bytes, *offset);
        push_u32(&mut bytes, *target | 3 << 25 | 1 << 27);
    }
    for (atom, string_index) in local_atoms.iter().zip(local_string_indexes) {
        push_u32(&mut bytes, string_index);
        bytes.push(macho::N_SECT);
        bytes.push(1);
        push_u16(&mut bytes, 0);
        push_u64(&mut bytes, locations[atom].0);
    }
    for (symbol, string_index) in symbols.symbols().iter().zip(external_string_indexes) {
        let value = match symbol.role() {
            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { primary_atom, .. } => {
                locations[&primary_atom].0
            }
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { atom, .. } => locations[&atom].0,
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { atom, .. } => locations[&atom].1,
        };
        push_u32(&mut bytes, string_index);
        bytes.push(macho::N_SECT | macho::N_EXT);
        bytes.push(1);
        push_u16(&mut bytes, 0);
        push_u64(&mut bytes, value);
    }
    bytes.extend_from_slice(&strings);
    ObjectFixture {
        bytes,
        section_offset: usize::try_from(section_offset).unwrap(),
    }
}

fn copy_atom_bytes(section: &mut [u8], start: u64, value: &[u8]) {
    let start = usize::try_from(start).unwrap();
    section[start..start + value.len()].copy_from_slice(value);
}

fn unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap()
}

const fn align_to(value: u64, alignment: u64) -> u64 {
    (value + alignment - 1) & !(alignment - 1)
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
