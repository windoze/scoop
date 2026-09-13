use object::macho;
use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, LinkageClass,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId, PersistentFunctionId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    RuntimeIdentityRecord, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalLirFoundation, LirTargetProfile, OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1,
    StrongProducerUnitPartitionV1,
};

use super::*;
use crate::{
    CanonicalScoopLirObjectUnitSetV1, DarwinBuildToolVersionV1, PlannedLinkObjectMemberSetV1,
    PlannedStrongObjectSymbolSetV1, validate_scoop_lir_llvm_22_1_object_envelope_v1,
};

#[test]
fn verifies_exact_external_symbols_and_primary_atom_range() {
    let fixture = fixture();
    let object = object_for_plan(&fixture.symbols, canonical_value);
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&object.bytes)
        .unwrap()
        .into_sections();

    let verified =
        verify_member_strong_object_definitions_v1(&object.bytes, sections, &fixture.symbols)
            .unwrap();

    assert_eq!(verified.member(), fixture.symbols.member());
    assert_eq!(verified.symbols().len(), 3);
    let definition = verified.definition(fixture.plan).unwrap();
    assert_eq!(definition.primary_atom(), fixture.atom);
    assert_eq!(definition.atoms().len(), 1);
    assert_eq!(definition.atoms()[0].atom(), fixture.atom);
    assert_eq!(definition.atoms()[0].section_ordinal().get(), 1);
    assert_eq!(definition.atoms()[0].start(), 0);
    assert_eq!(definition.atoms()[0].end(), 4);
    assert_eq!(definition.atoms()[0].padding_end(), 8);
    assert!(
        verified
            .strong_symbol_by_table_index(definition.primary_symbol_table_index())
            .is_some()
    );
}

#[test]
fn rejects_duplicate_and_unexpected_external_definitions() {
    let fixture = fixture();
    let mut duplicate = object_for_plan(&fixture.symbols, canonical_value);
    write_u32(
        &mut duplicate.bytes,
        duplicate.symbol_offset,
        duplicate.string_indexes[1],
    );
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&duplicate.bytes)
        .unwrap()
        .into_sections();
    assert!(matches!(
        verify_member_strong_object_definitions_v1(&duplicate.bytes, sections, &fixture.symbols),
        Err(StrongObjectDefinitionValidationError::DuplicateExternalStrongDefinition { .. })
    ));

    let mut unexpected = object_for_plan(&fixture.symbols, canonical_value);
    let first_name = unexpected.string_offset + unexpected.string_indexes[0] as usize;
    unexpected.bytes[first_name] = b'X';
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&unexpected.bytes)
        .unwrap()
        .into_sections();
    assert!(matches!(
        verify_member_strong_object_definitions_v1(&unexpected.bytes, sections, &fixture.symbols),
        Err(StrongObjectDefinitionValidationError::UnexpectedExternalStrongDefinition { .. })
    ));
}

#[test]
fn rejects_empty_ranges_and_displaced_primary_symbols() {
    let fixture = fixture();
    let empty = object_for_plan(&fixture.symbols, |role| match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 0,
    });
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&empty.bytes)
        .unwrap()
        .into_sections();
    assert_eq!(
        verify_member_strong_object_definitions_v1(&empty.bytes, sections, &fixture.symbols),
        Err(StrongObjectDefinitionValidationError::InvalidAtomRange {
            atom: fixture.atom,
            start: 0,
            end: 0,
        })
    );

    let displaced = object_for_plan(&fixture.symbols, |role| match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. } => 1,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 4,
    });
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&displaced.bytes)
        .unwrap()
        .into_sections();
    assert_eq!(
        verify_member_strong_object_definitions_v1(&displaced.bytes, sections, &fixture.symbols),
        Err(
            StrongObjectDefinitionValidationError::PrimarySymbolLocationMismatch {
                definition: fixture.plan,
                primary_atom: fixture.atom,
            }
        )
    );
}

#[test]
fn rejects_overlapping_atom_ranges_across_one_member() {
    let fixture = fixture_with_associated_atom();
    let associated_atom = fixture.associated_atom.unwrap();
    let overlapping = object_for_plan(&fixture.symbols, |role| match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { atom, .. } if atom == fixture.atom => {
            0
        }
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { atom, .. } if atom == fixture.atom => 4,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { atom, .. }
            if atom == associated_atom =>
        {
            2
        }
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { atom, .. }
            if atom == associated_atom =>
        {
            6
        }
        _ => unreachable!("fixture has exactly two planned atoms"),
    });
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&overlapping.bytes)
        .unwrap()
        .into_sections();
    assert_eq!(
        verify_member_strong_object_definitions_v1(&overlapping.bytes, sections, &fixture.symbols),
        Err(
            StrongObjectDefinitionValidationError::OverlappingAtomRanges {
                first: fixture.atom,
                second: associated_atom,
            }
        )
    );
}

#[test]
fn binds_the_validation_to_exact_bytes_and_requires_zero_padding() {
    let fixture = fixture();
    let mut changed = object_for_plan(&fixture.symbols, canonical_value);
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&changed.bytes)
        .unwrap()
        .into_sections();
    changed.bytes[changed.section_offset] ^= 1;
    assert_eq!(
        verify_member_strong_object_definitions_v1(&changed.bytes, sections, &fixture.symbols),
        Err(StrongObjectDefinitionValidationError::ObjectBytesMismatch)
    );

    let mut nonzero_padding = object_for_plan(&fixture.symbols, canonical_value);
    nonzero_padding.bytes[nonzero_padding.section_offset + 4] = 1;
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&nonzero_padding.bytes)
        .unwrap()
        .into_sections();
    assert_eq!(
        verify_member_strong_object_definitions_v1(
            &nonzero_padding.bytes,
            sections,
            &fixture.symbols,
        ),
        Err(StrongObjectDefinitionValidationError::NonzeroAtomPadding {
            atom: fixture.atom,
            address: 4,
        })
    );
}

fn canonical_value(role: PlannedStrongObjectSymbolRoleV1) -> u64 {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 4,
    }
}

pub(in crate::link_object) struct Fixture {
    pub(in crate::link_object) plan: ObjectDefinitionPlanId,
    pub(in crate::link_object) atom: ObjectDefinitionAtomId,
    associated_atom: Option<ObjectDefinitionAtomId>,
    pub(in crate::link_object) symbols: PlannedMemberStrongObjectSymbolsV1,
}

pub(in crate::link_object) fn fixture() -> Fixture {
    build_fixture(ConeIdentity::CORE, "entry", false)
}

fn fixture_with_associated_atom() -> Fixture {
    build_fixture(ConeIdentity::CORE, "entry", true)
}

pub(in crate::link_object) fn fixture_named(name: &str) -> Fixture {
    build_fixture(ConeIdentity::CORE, name, false)
}

pub(in crate::link_object) fn fixture_for_producer(producer: ConeIdentity, name: &str) -> Fixture {
    build_fixture(producer, name, false)
}

pub(in crate::link_object) fn fixture_for_type_descriptor(
    producer: ConeIdentity,
    target: PersistentExactTypeId,
) -> Fixture {
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::exact_type(target),
            StrongDefinitionRole::TypeDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeDescriptor(target),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![plan.clone()]).unwrap();
    canonical.set_definition_atoms(vec![atom.clone()]).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();
    let surface = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    let partition = StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let members = PlannedLinkObjectMemberSetV1::new(
        producer,
        &partition,
        vec![CanonicalScoopLirObjectUnitSetV1::new(vec![plan.id()]).unwrap()],
        Vec::new(),
    )
    .unwrap();
    let symbols =
        PlannedStrongObjectSymbolSetV1::new(LirTargetProfile::DARWIN_AARCH64, &surface, &members)
            .unwrap()
            .members()[0]
            .clone();
    Fixture {
        plan: plan.id(),
        atom: atom.id(),
        associated_atom: None,
        symbols,
    }
}

fn build_fixture(producer: ConeIdentity, name: &str, include_associated_atom: bool) -> Fixture {
    let function =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                producer,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(function.id()),
    ))
    .unwrap();
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let associated_atom = include_associated_atom.then(|| {
        CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            plan.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap()
    });
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body.id()),
        LinkageClass::ConeStrong,
    )
    .unwrap();

    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_callable_bodies(vec![body]).unwrap();
    canonical.set_definition_plans(vec![plan.clone()]).unwrap();
    let mut atoms = vec![atom.clone()];
    atoms.extend(associated_atom.iter().cloned());
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();
    let surface = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    let partition = StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let members = PlannedLinkObjectMemberSetV1::new(
        producer,
        &partition,
        vec![CanonicalScoopLirObjectUnitSetV1::new(vec![plan.id()]).unwrap()],
        Vec::new(),
    )
    .unwrap();
    let symbols =
        PlannedStrongObjectSymbolSetV1::new(LirTargetProfile::DARWIN_AARCH64, &surface, &members)
            .unwrap()
            .members()[0]
            .clone();
    Fixture {
        plan: plan.id(),
        atom: atom.id(),
        associated_atom: associated_atom.map(|atom| atom.id()),
        symbols,
    }
}

pub(in crate::link_object) struct ObjectFixture {
    pub(in crate::link_object) bytes: Vec<u8>,
    section_offset: usize,
    symbol_offset: usize,
    string_offset: usize,
    string_indexes: Vec<u32>,
}

fn object_for_plan(
    plan: &PlannedMemberStrongObjectSymbolsV1,
    value: impl Fn(PlannedStrongObjectSymbolRoleV1) -> u64,
) -> ObjectFixture {
    object_for_plan_with_branch_relocation(plan, value, None)
}

pub(in crate::link_object) fn object_for_plan_with_branch_relocation(
    plan: &PlannedMemberStrongObjectSymbolsV1,
    value: impl Fn(PlannedStrongObjectSymbolRoleV1) -> u64,
    relocation: Option<(u32, PlannedStrongObjectSymbolRoleV1)>,
) -> ObjectFixture {
    match relocation {
        Some(relocation) => object_for_plan_with_deployment(plan, value, &[relocation], None),
        None => object_for_plan_with_deployment(plan, value, &[], None),
    }
}

pub(in crate::link_object) fn object_for_plan_with_deployment(
    plan: &PlannedMemberStrongObjectSymbolsV1,
    value: impl Fn(PlannedStrongObjectSymbolRoleV1) -> u64,
    relocations: &[(u32, PlannedStrongObjectSymbolRoleV1)],
    deployment: Option<(u32, u32, &[DarwinBuildToolVersionV1])>,
) -> ObjectFixture {
    let segment_size = 152_u32;
    let symtab_size = 24_u32;
    let dysymtab_size = 80_u32;
    let deployment_size = deployment
        .map(|(_, _, tools)| 24 + u32::try_from(tools.len()).unwrap() * 8)
        .unwrap_or(0);
    let command_bytes = segment_size + symtab_size + dysymtab_size + deployment_size;
    let section_offset = 32 + command_bytes;
    let section_size = 8_u32;
    let relocation_count = u32::try_from(relocations.len()).unwrap();
    let relocation_offset = section_offset + section_size;
    let symbol_offset = relocation_offset + relocation_count * 8;
    let symbol_bytes = u32::try_from(plan.symbols().len() * 16).unwrap();
    let string_offset = symbol_offset + symbol_bytes;
    let mut strings = vec![0];
    let mut string_indexes = Vec::with_capacity(plan.symbols().len());
    for symbol in plan.symbols() {
        string_indexes.push(u32::try_from(strings.len()).unwrap());
        strings.extend_from_slice(symbol.macho_name());
        strings.push(0);
    }
    let string_size = u32::try_from(strings.len()).unwrap();
    let mut bytes = Vec::with_capacity((string_offset + string_size) as usize);

    push_u32(&mut bytes, macho::MH_MAGIC_64);
    push_u32(&mut bytes, macho::CPU_TYPE_ARM64);
    push_u32(&mut bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(&mut bytes, macho::MH_OBJECT);
    push_u32(&mut bytes, 3 + u32::from(deployment.is_some()));
    push_u32(&mut bytes, command_bytes);
    push_u32(&mut bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SEGMENT_64);
    push_u32(&mut bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, u64::from(section_size));
    push_u64(&mut bytes, u64::from(section_offset));
    push_u64(&mut bytes, u64::from(section_size));
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);

    push_fixed_name(&mut bytes, b"__text");
    push_fixed_name(&mut bytes, b"__TEXT");
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, u64::from(section_size));
    push_u32(&mut bytes, section_offset);
    push_u32(&mut bytes, 3);
    push_u32(
        &mut bytes,
        if !relocations.is_empty() {
            relocation_offset
        } else {
            0
        },
    );
    push_u32(&mut bytes, relocation_count);
    push_u32(
        &mut bytes,
        macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
    );
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SYMTAB);
    push_u32(&mut bytes, symtab_size);
    push_u32(&mut bytes, symbol_offset);
    push_u32(&mut bytes, plan.symbols().len() as u32);
    push_u32(&mut bytes, string_offset);
    push_u32(&mut bytes, string_size);

    push_u32(&mut bytes, macho::LC_DYSYMTAB);
    push_u32(&mut bytes, dysymtab_size);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, plan.symbols().len() as u32);
    push_u32(&mut bytes, plan.symbols().len() as u32);
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(&[0; 48]);

    if let Some((minimum_os, sdk, tools)) = deployment {
        push_u32(&mut bytes, macho::LC_BUILD_VERSION);
        push_u32(&mut bytes, deployment_size);
        push_u32(&mut bytes, macho::PLATFORM_MACOS);
        push_u32(&mut bytes, minimum_os);
        push_u32(&mut bytes, sdk);
        push_u32(&mut bytes, u32::try_from(tools.len()).unwrap());
        for tool in tools {
            push_u32(&mut bytes, tool.tool());
            push_u32(&mut bytes, tool.version());
        }
    }

    bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0]);
    for (offset, target_role) in relocations {
        let target = plan
            .symbols()
            .iter()
            .position(|symbol| symbol.role() == *target_role)
            .unwrap() as u32;
        push_u32(&mut bytes, *offset);
        push_u32(
            &mut bytes,
            target | 1 << 24 | 2 << 25 | 1 << 27 | u32::from(macho::ARM64_RELOC_BRANCH26) << 28,
        );
    }
    for (symbol, string_index) in plan.symbols().iter().zip(&string_indexes) {
        push_u32(&mut bytes, *string_index);
        bytes.push(macho::N_SECT | macho::N_EXT);
        bytes.push(1);
        push_u16(&mut bytes, 0);
        push_u64(&mut bytes, value(symbol.role()));
    }
    bytes.extend_from_slice(&strings);
    ObjectFixture {
        bytes,
        section_offset: section_offset as usize,
        symbol_offset: symbol_offset as usize,
        string_offset: string_offset as usize,
        string_indexes,
    }
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

fn push_fixed_name(bytes: &mut Vec<u8>, name: &[u8]) {
    let mut fixed = [0; 16];
    fixed[..name.len()].copy_from_slice(name);
    bytes.extend_from_slice(&fixed);
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
