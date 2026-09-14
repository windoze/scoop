use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeKey, DigestSemanticFieldRole,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_lir::{ConeImagePlanV1, LinkageClass};

use super::physical::{atom_file_range, validate_objects, verified_member};
use super::record::{IMAGE_DESCRIPTOR_SIZE, expected_image_record};
use super::{
    ConeImageAtomRoleV1, ConeImagePatchFailureV1, ConeImageRelocationFailureV1,
    ConeImageValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    ScoopLirObjectCandidateV1, StrongDefinitionOwnerV1, StrongRelocationBindingV1,
    StrongRelocationResolutionV1, VerifiedDarwinArm64RelocationFormV1,
    VerifiedDarwinArm64RelocationShapeV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMaterializedPatchSiteV1, VerifiedMemberObjectRelocationIndexV1,
    VerifiedRelocationTargetV1, VerifiedRelocationUseV1, VerifiedScoopLirDigestPatchSiteSetV1,
};

const RUNTIME_IMAGE_FINGERPRINT_OFFSET: u64 = 96;
const DIGEST_WIDTH: u8 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedConeImageAtomV1 {
    atom: ObjectDefinitionAtomId,
    checked_offset: u64,
    byte_size: u64,
}

impl VerifiedConeImageAtomV1 {
    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn checked_offset(self) -> u64 {
        self.checked_offset
    }

    pub const fn byte_size(self) -> u64 {
        self.byte_size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedConeImageSupportAtomsV1 {
    coordinate_group: VerifiedConeImageAtomV1,
    coordinate_name: VerifiedConeImageAtomV1,
    coordinate_version: VerifiedConeImageAtomV1,
    dependencies: VerifiedConeImageAtomV1,
    static_storages: VerifiedConeImageAtomV1,
    immortal_objects: VerifiedConeImageAtomV1,
    initialization_units: VerifiedConeImageAtomV1,
    type_registrations: VerifiedConeImageAtomV1,
    safepoints: VerifiedConeImageAtomV1,
    callables: VerifiedConeImageAtomV1,
}

impl VerifiedConeImageSupportAtomsV1 {
    pub const fn coordinate_group(self) -> VerifiedConeImageAtomV1 {
        self.coordinate_group
    }

    pub const fn coordinate_name(self) -> VerifiedConeImageAtomV1 {
        self.coordinate_name
    }

    pub const fn coordinate_version(self) -> VerifiedConeImageAtomV1 {
        self.coordinate_version
    }

    pub const fn dependencies(self) -> VerifiedConeImageAtomV1 {
        self.dependencies
    }

    pub const fn static_storages(self) -> VerifiedConeImageAtomV1 {
        self.static_storages
    }

    pub const fn immortal_objects(self) -> VerifiedConeImageAtomV1 {
        self.immortal_objects
    }

    pub const fn initialization_units(self) -> VerifiedConeImageAtomV1 {
        self.initialization_units
    }

    pub const fn type_registrations(self) -> VerifiedConeImageAtomV1 {
        self.type_registrations
    }

    pub const fn safepoints(self) -> VerifiedConeImageAtomV1 {
        self.safepoints
    }

    pub const fn callables(self) -> VerifiedConeImageAtomV1 {
        self.callables
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedConeImageV1 {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: ConeImagePlanV1,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    primary: VerifiedConeImageAtomV1,
    support: VerifiedConeImageSupportAtomsV1,
    image_patch: VerifiedMaterializedPatchSiteV1,
    support_relocations: Vec<VerifiedRelocationUseV1>,
    registration_relocations: Vec<StrongRelocationBindingV1>,
}

impl VerifiedConeImageV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.plan.cone().identity()
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &ConeImagePlanV1 {
        &self.plan
    }

    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn primary_symbol_table_index(&self) -> u32 {
        self.primary_symbol_table_index
    }

    pub const fn primary(&self) -> VerifiedConeImageAtomV1 {
        self.primary
    }

    pub const fn support(&self) -> VerifiedConeImageSupportAtomsV1 {
        self.support
    }

    pub const fn image_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.image_patch
    }

    pub fn support_relocations(&self) -> &[VerifiedRelocationUseV1] {
        &self.support_relocations
    }

    pub fn registration_relocations(&self) -> &[StrongRelocationBindingV1] {
        &self.registration_relocations
    }
}

pub fn verify_cone_image_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: ConeImagePlanV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedConeImageV1, ConeImageValidationError> {
    if patch_sites.producer() != plan.cone().identity() {
        return Err(ConeImageValidationError::ProducerMismatch {
            image: plan.cone().identity(),
            patches: patch_sites.producer(),
        });
    }
    let objects = validate_objects(patch_sites.builtins(), scoop_objects)?;
    let member = require_unique_image(&patch_sites, &plan)?;
    let verified_member = verified_member(patch_sites.builtins(), member)?;
    let definition = verified_member
        .definitions()
        .definition(plan.definition_plan())
        .ok_or(ConeImageValidationError::MissingVerifiedDefinition(
            plan.definition_plan(),
        ))?;
    if definition.primary_atom() != plan.primary_atom() {
        return Err(ConeImageValidationError::PrimaryAtomMismatch {
            expected: plan.primary_atom(),
            actual: definition.primary_atom(),
        });
    }
    validate_exact_atom_set(definition.atoms(), &plan)?;

    let primary = require_atom(
        verified_member,
        definition.atoms(),
        plan.primary_atom(),
        DefinitionAtomRole::Primary,
        ConeImageAtomRoleV1::Primary,
        IMAGE_DESCRIPTOR_SIZE as u64,
        8,
    )?;
    validate_atom_bytes(
        objects[&member],
        primary,
        ConeImageAtomRoleV1::Primary,
        &expected_image_record(&plan),
    )?;

    let support_plan = plan.support_atoms();
    let coordinate = plan.cone().coordinate();
    let coordinate_group = require_support_bytes(
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.coordinate_group(),
        ConeImageAtomRoleV1::CoordinateGroup,
        coordinate.group().as_bytes(),
    )?;
    let coordinate_name = require_support_bytes(
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.coordinate_name(),
        ConeImageAtomRoleV1::CoordinateName,
        coordinate.name().as_bytes(),
    )?;
    let coordinate_version = require_support_bytes(
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.coordinate_version(),
        ConeImageAtomRoleV1::CoordinateVersion,
        coordinate.version().as_bytes(),
    )?;
    let dependency_bytes = if plan.dependencies().is_empty() {
        vec![0; 32]
    } else {
        plan.dependencies()
            .iter()
            .flat_map(|identity| identity.as_array().iter().copied())
            .collect()
    };
    let dependencies = require_table_bytes(
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.dependencies(),
        ConeImageAtomRoleV1::Dependencies,
        &dependency_bytes,
        1,
    )?;
    require_relocation_count(
        verified_member,
        support_plan.dependencies(),
        0,
        ConeImageAtomRoleV1::Dependencies,
    )?;

    let mut registration_relocations = Vec::new();
    let static_storages = verify_registration_table(
        &patch_sites,
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.static_storages(),
        ConeImageAtomRoleV1::StaticStorages,
        plan.tables().static_storages().iter().copied().map(|id| {
            registration_target(
                plan.cone().identity(),
                StrongDefinitionEntity::static_storage(id),
                StrongDefinitionRole::RootRegistration,
            )
        }),
        &mut registration_relocations,
    )?;
    let immortal_objects = verify_registration_table(
        &patch_sites,
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.immortal_objects(),
        ConeImageAtomRoleV1::ImmortalObjects,
        plan.tables().immortal_objects().iter().copied().map(|id| {
            registration_target(
                plan.cone().identity(),
                StrongDefinitionEntity::immortal_object(id),
                StrongDefinitionRole::ImmortalRegistration,
            )
        }),
        &mut registration_relocations,
    )?;
    let initialization_units = verify_registration_table(
        &patch_sites,
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.initialization_units(),
        ConeImageAtomRoleV1::InitializationUnits,
        plan.tables()
            .initialization_units()
            .iter()
            .copied()
            .map(|id| {
                registration_target(
                    plan.cone().identity(),
                    StrongDefinitionEntity::initialization_unit(id),
                    StrongDefinitionRole::InitializationRegistration,
                )
            }),
        &mut registration_relocations,
    )?;
    let type_registrations = verify_registration_table(
        &patch_sites,
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.type_registrations(),
        ConeImageAtomRoleV1::TypeRegistrations,
        plan.tables()
            .type_registrations()
            .iter()
            .copied()
            .map(|id| {
                registration_target(
                    plan.cone().identity(),
                    StrongDefinitionEntity::exact_type(id),
                    StrongDefinitionRole::TypeRegistration,
                )
            }),
        &mut registration_relocations,
    )?;
    let safepoints = verify_registration_table(
        &patch_sites,
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.safepoints(),
        ConeImageAtomRoleV1::Safepoints,
        plan.tables().safepoints().iter().copied().map(|id| {
            registration_target(
                plan.cone().identity(),
                StrongDefinitionEntity::safepoint_site(id),
                StrongDefinitionRole::SafepointRegistration,
            )
        }),
        &mut registration_relocations,
    )?;
    let callables = verify_registration_table(
        &patch_sites,
        verified_member,
        definition.atoms(),
        objects[&member],
        support_plan.callables(),
        ConeImageAtomRoleV1::Callables,
        plan.tables().callables().iter().copied().map(|id| {
            registration_target(
                plan.cone().identity(),
                StrongDefinitionEntity::callable_body(id),
                StrongDefinitionRole::CallableRegistration,
            )
        }),
        &mut registration_relocations,
    )?;

    let support = VerifiedConeImageSupportAtomsV1 {
        coordinate_group,
        coordinate_name,
        coordinate_version,
        dependencies,
        static_storages,
        immortal_objects,
        initialization_units,
        type_registrations,
        safepoints,
        callables,
    };
    let support_relocations = verify_primary_relocations(verified_member, &plan, support)?;
    let image_patch = verify_image_patch(&patch_sites, &plan, member, primary.checked_offset)?;
    validate_digest_node(&patch_sites, &plan)?;
    let primary_symbol_table_index = definition.primary_symbol_table_index();

    Ok(VerifiedConeImageV1 {
        patch_sites,
        plan,
        member,
        primary_symbol_table_index,
        primary,
        support,
        image_patch,
        support_relocations,
        registration_relocations,
    })
}

fn validate_exact_atom_set(
    actual: &[VerifiedDefinitionAtomRangeV1],
    plan: &ConeImagePlanV1,
) -> Result<(), ConeImageValidationError> {
    let support = plan.support_atoms();
    let mut expected = vec![
        (plan.primary_atom(), DefinitionAtomRole::Primary),
        (
            support.coordinate_group(),
            DefinitionAtomRole::AddressTakenConstant,
        ),
        (
            support.coordinate_name(),
            DefinitionAtomRole::AddressTakenConstant,
        ),
        (
            support.coordinate_version(),
            DefinitionAtomRole::AddressTakenConstant,
        ),
        (support.dependencies(), DefinitionAtomRole::RuntimeRecord),
        (support.static_storages(), DefinitionAtomRole::RuntimeRecord),
        (
            support.immortal_objects(),
            DefinitionAtomRole::RuntimeRecord,
        ),
        (
            support.initialization_units(),
            DefinitionAtomRole::RuntimeRecord,
        ),
        (
            support.type_registrations(),
            DefinitionAtomRole::RuntimeRecord,
        ),
        (support.safepoints(), DefinitionAtomRole::RuntimeRecord),
        (support.callables(), DefinitionAtomRole::RuntimeRecord),
    ];
    expected.sort_unstable();
    let mut actual = actual
        .iter()
        .map(|atom| (atom.atom(), atom.atom_role()))
        .collect::<Vec<_>>();
    actual.sort_unstable();
    if actual != expected {
        return Err(ConeImageValidationError::AtomSetMismatch { expected, actual });
    }
    Ok(())
}

fn require_unique_image(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &ConeImagePlanV1,
) -> Result<SlibMemberId, ConeImageValidationError> {
    let mut actual = Vec::new();
    for member in patch_sites.builtins().strong_relocations().members() {
        for symbol in member.definitions().symbols() {
            if let crate::link_object::PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                definition,
                owner,
                definition_role: StrongDefinitionRole::ImageDescriptor,
                ..
            } = symbol.role()
                && let StrongDefinitionEntityKind::ConeImage(cone) = owner.kind()
            {
                actual.push((member.member(), definition, cone));
            }
        }
    }
    let expected_member = patch_sites
        .builtins()
        .member_plan()
        .member_for_definition(plan.definition_plan())
        .ok_or(ConeImageValidationError::MissingDefinitionAssignment(
            plan.definition_plan(),
        ))?;
    if !patch_sites
        .builtins()
        .member_plan()
        .scoop_lir_members()
        .iter()
        .any(|member| member.member_id() == expected_member)
    {
        return Err(ConeImageValidationError::DefinitionAssignedToNonScoopMember(expected_member));
    }
    if actual.as_slice()
        != [(
            expected_member,
            plan.definition_plan(),
            plan.cone().identity(),
        )]
    {
        return Err(ConeImageValidationError::ImageDefinitionSet {
            expected: plan.definition_plan(),
            actual,
        });
    }
    Ok(expected_member)
}

#[allow(clippy::too_many_arguments)]
fn require_atom(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atoms: &[VerifiedDefinitionAtomRangeV1],
    expected_atom: ObjectDefinitionAtomId,
    expected_atom_role: DefinitionAtomRole,
    role: ConeImageAtomRoleV1,
    expected_size: u64,
    required_alignment: u64,
) -> Result<VerifiedConeImageAtomV1, ConeImageValidationError> {
    let atom = atoms
        .iter()
        .find(|atom| atom.atom() == expected_atom && atom.atom_role() == expected_atom_role)
        .copied()
        .ok_or(ConeImageValidationError::MissingAtom {
            role,
            atom: expected_atom,
        })?;
    let (section_role, start, end) = atom_file_range(member, atom).map_err(|kind| {
        ConeImageValidationError::InvalidAtomFileRange {
            role,
            atom: expected_atom,
            kind,
        }
    })?;
    if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(ConeImageValidationError::AtomSectionMismatch {
            role,
            actual: section_role,
        });
    }
    let actual_size = end - start;
    if actual_size != expected_size {
        return Err(ConeImageValidationError::AtomSizeMismatch {
            role,
            expected: expected_size,
            actual: actual_size,
        });
    }
    if atom.start() % required_alignment != 0 {
        return Err(ConeImageValidationError::AtomAlignmentMismatch {
            role,
            required: required_alignment,
            address: atom.start(),
        });
    }
    Ok(VerifiedConeImageAtomV1 {
        atom: expected_atom,
        checked_offset: start,
        byte_size: actual_size,
    })
}

fn require_support_bytes(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atoms: &[VerifiedDefinitionAtomRangeV1],
    object: &[u8],
    atom: ObjectDefinitionAtomId,
    role: ConeImageAtomRoleV1,
    expected: &[u8],
) -> Result<VerifiedConeImageAtomV1, ConeImageValidationError> {
    let verified = require_atom(
        member,
        atoms,
        atom,
        DefinitionAtomRole::AddressTakenConstant,
        role,
        expected.len() as u64,
        1,
    )?;
    validate_atom_bytes(object, verified, role, expected)?;
    require_relocation_count(member, atom, 0, role)?;
    Ok(verified)
}

fn require_table_bytes(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atoms: &[VerifiedDefinitionAtomRangeV1],
    object: &[u8],
    atom: ObjectDefinitionAtomId,
    role: ConeImageAtomRoleV1,
    expected: &[u8],
    alignment: u64,
) -> Result<VerifiedConeImageAtomV1, ConeImageValidationError> {
    let verified = require_atom(
        member,
        atoms,
        atom,
        DefinitionAtomRole::RuntimeRecord,
        role,
        expected.len() as u64,
        alignment,
    )?;
    validate_atom_bytes(object, verified, role, expected)?;
    Ok(verified)
}

fn validate_atom_bytes(
    object: &[u8],
    atom: VerifiedConeImageAtomV1,
    role: ConeImageAtomRoleV1,
    expected: &[u8],
) -> Result<(), ConeImageValidationError> {
    let start = usize::try_from(atom.checked_offset)
        .map_err(|_| ConeImageValidationError::RecordRangeOverflow(role))?;
    let end = start
        .checked_add(expected.len())
        .ok_or(ConeImageValidationError::RecordRangeOverflow(role))?;
    let actual = object
        .get(start..end)
        .ok_or(ConeImageValidationError::RecordRangeOverflow(role))?;
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(ConeImageValidationError::AtomByteMismatch {
            role,
            offset_within_atom: offset as u64,
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}

fn verify_primary_relocations(
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &ConeImagePlanV1,
    support: VerifiedConeImageSupportAtomsV1,
) -> Result<Vec<VerifiedRelocationUseV1>, ConeImageValidationError> {
    let expected = [
        (
            16,
            support.coordinate_group,
            ConeImageAtomRoleV1::CoordinateGroup,
        ),
        (
            32,
            support.coordinate_name,
            ConeImageAtomRoleV1::CoordinateName,
        ),
        (
            48,
            support.coordinate_version,
            ConeImageAtomRoleV1::CoordinateVersion,
        ),
        (128, support.dependencies, ConeImageAtomRoleV1::Dependencies),
        (
            144,
            support.static_storages,
            ConeImageAtomRoleV1::StaticStorages,
        ),
        (
            160,
            support.immortal_objects,
            ConeImageAtomRoleV1::ImmortalObjects,
        ),
        (
            176,
            support.initialization_units,
            ConeImageAtomRoleV1::InitializationUnits,
        ),
        (
            192,
            support.type_registrations,
            ConeImageAtomRoleV1::TypeRegistrations,
        ),
        (208, support.safepoints, ConeImageAtomRoleV1::Safepoints),
        (224, support.callables, ConeImageAtomRoleV1::Callables),
    ];
    require_relocation_count(
        member,
        plan.primary_atom(),
        expected.len(),
        ConeImageAtomRoleV1::Primary,
    )?;
    expected
        .into_iter()
        .enumerate()
        .map(|(index, (offset, target, role))| {
            verify_local_relocation(member, plan.primary_atom(), offset, target, role, index)
        })
        .collect()
}

fn verify_local_relocation(
    member: &VerifiedMemberObjectRelocationIndexV1,
    source_atom: ObjectDefinitionAtomId,
    offset: u64,
    target_atom: VerifiedConeImageAtomV1,
    role: ConeImageAtomRoleV1,
    index: usize,
) -> Result<VerifiedRelocationUseV1, ConeImageValidationError> {
    let relocation = require_relocation(member, source_atom, offset, role, index)?;
    let kind = if relocation.containing_atom_role() != DefinitionAtomRole::Primary {
        Some(ConeImageRelocationFailureV1::ContainingAtomRole)
    } else if relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(ConeImageRelocationFailureV1::SectionRole)
    } else if relocation.width_bytes() != 8 {
        Some(ConeImageRelocationFailureV1::Width)
    } else if relocation.encoded_value() != 0 {
        Some(ConeImageRelocationFailureV1::EncodedValue)
    } else {
        match relocation.shape() {
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                target:
                    VerifiedRelocationTargetV1::LocalDefinition {
                        owner_atom,
                        section_ordinal,
                        value,
                        ..
                    },
            } => {
                if *owner_atom != Some(target_atom.atom) {
                    Some(ConeImageRelocationFailureV1::TargetAtom)
                } else {
                    let target_range = member
                        .definitions()
                        .definitions()
                        .iter()
                        .flat_map(|definition| definition.atoms())
                        .find(|atom| atom.atom() == target_atom.atom)
                        .expect("verified image support atom remains in its definition");
                    if *section_ordinal != target_range.section_ordinal() {
                        Some(ConeImageRelocationFailureV1::TargetSection)
                    } else if *value != target_range.start() {
                        Some(ConeImageRelocationFailureV1::TargetValue)
                    } else {
                        None
                    }
                }
            }
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { .. } => {
                Some(ConeImageRelocationFailureV1::TargetKind)
            }
            _ => Some(ConeImageRelocationFailureV1::Form),
        }
    };
    if let Some(kind) = kind {
        return relocation_error(role, index, kind);
    }
    Ok(relocation.clone())
}

#[derive(Clone, Copy)]
struct ExpectedRegistrationTargetV1 {
    definition: ObjectDefinitionPlanId,
    owner: StrongDefinitionOwnerV1,
    symbol: PersistentSymbolRequest,
}

fn registration_target(
    producer: ConeIdentity,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> ExpectedRegistrationTargetV1 {
    let key = scoop_identity::ObjectDefinitionPlanKey::strong(producer, entity, role)
        .expect("registration entity and role are statically paired");
    ExpectedRegistrationTargetV1 {
        definition: ObjectDefinitionPlanId::from_key(&key)
            .expect("registration definition identity hashes"),
        owner: StrongDefinitionOwnerV1::new(entity, role)
            .expect("registration owner is a non-reserved strong kind"),
        symbol: PersistentSymbolRequest::new(
            key.primary_symbol_key()
                .expect("registration definition has one primary symbol"),
            LinkageClass::ConeStrong,
        )
        .expect("registration symbol accepts strong Cone linkage"),
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_registration_table(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    member: &VerifiedMemberObjectRelocationIndexV1,
    atoms: &[VerifiedDefinitionAtomRangeV1],
    object: &[u8],
    atom: ObjectDefinitionAtomId,
    role: ConeImageAtomRoleV1,
    targets: impl IntoIterator<Item = ExpectedRegistrationTargetV1>,
    all_bindings: &mut Vec<StrongRelocationBindingV1>,
) -> Result<VerifiedConeImageAtomV1, ConeImageValidationError> {
    let targets = targets.into_iter().collect::<Vec<_>>();
    let expected_bytes = vec![0; targets.len().max(1) * 8];
    let verified = require_table_bytes(member, atoms, object, atom, role, &expected_bytes, 8)?;
    require_relocation_count(member, atom, targets.len(), role)?;
    for (index, target) in targets.into_iter().enumerate() {
        let offset = u64::try_from(index).expect("table index fits u64") * 8;
        let relocation = require_relocation(member, atom, offset, role, index)?;
        let binding = patch_sites
            .builtins()
            .strong_relocations()
            .bindings()
            .iter()
            .find(|binding| {
                binding.source_member() == member.member()
                    && binding.containing_atom() == atom
                    && binding.offset_within_atom() == offset
            })
            .ok_or(ConeImageValidationError::RelocationMismatch {
                role,
                index,
                kind: ConeImageRelocationFailureV1::TargetKind,
            })?;
        let kind = if relocation.containing_atom_role() != DefinitionAtomRole::RuntimeRecord
            || binding.containing_atom_role() != DefinitionAtomRole::RuntimeRecord
        {
            Some(ConeImageRelocationFailureV1::ContainingAtomRole)
        } else if relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
            || binding.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
        {
            Some(ConeImageRelocationFailureV1::SectionRole)
        } else if relocation.width_bytes() != 8 || binding.width_bytes() != 8 {
            Some(ConeImageRelocationFailureV1::Width)
        } else if relocation.shape().form() != VerifiedDarwinArm64RelocationFormV1::Unsigned64
            || binding.relocation_form() != VerifiedDarwinArm64RelocationFormV1::Unsigned64
        {
            Some(ConeImageRelocationFailureV1::Form)
        } else if relocation.encoded_value() != 0 || binding.encoded_value() != 0 {
            Some(ConeImageRelocationFailureV1::EncodedValue)
        } else if binding.target_slot() != RelocationTargetSlotV1::Single {
            Some(ConeImageRelocationFailureV1::TargetSlot)
        } else if binding.symbol() != expected_macho_name(target.symbol).as_slice() {
            Some(ConeImageRelocationFailureV1::TargetSymbol)
        } else {
            match binding.resolution() {
                StrongRelocationResolutionV1::ObjectLocalStrong {
                    definition,
                    owner: LinkDefinitionOwnerV1::StrongDefinition(owner),
                    ..
                }
                | StrongRelocationResolutionV1::CurrentConeUndefinedStrong {
                    definition,
                    owner: LinkDefinitionOwnerV1::StrongDefinition(owner),
                    ..
                } => {
                    if definition != target.definition {
                        Some(ConeImageRelocationFailureV1::TargetDefinition)
                    } else if owner != target.owner {
                        Some(ConeImageRelocationFailureV1::TargetOwner)
                    } else {
                        None
                    }
                }
                _ => Some(ConeImageRelocationFailureV1::TargetKind),
            }
        };
        if let Some(kind) = kind {
            return relocation_error(role, index, kind);
        }
        all_bindings.push(binding.clone());
    }
    Ok(verified)
}

fn expected_macho_name(request: PersistentSymbolRequest) -> Vec<u8> {
    let mut name = Vec::with_capacity(request.symbol().as_str().len() + 1);
    name.push(b'_');
    name.extend_from_slice(request.symbol().as_str().as_bytes());
    name
}

fn require_relocation_count(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: ObjectDefinitionAtomId,
    expected: usize,
    role: ConeImageAtomRoleV1,
) -> Result<(), ConeImageValidationError> {
    let actual = member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == atom)
        .count();
    if actual != expected {
        return relocation_error(role, actual, ConeImageRelocationFailureV1::Count);
    }
    Ok(())
}

fn require_relocation(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: ObjectDefinitionAtomId,
    offset: u64,
    role: ConeImageAtomRoleV1,
    index: usize,
) -> Result<&VerifiedRelocationUseV1, ConeImageValidationError> {
    let matches = member
        .relocations()
        .iter()
        .filter(|relocation| {
            relocation.containing_atom() == atom && relocation.offset_within_atom() == offset
        })
        .collect::<Vec<_>>();
    let [relocation] = matches.as_slice() else {
        return relocation_error(role, index, ConeImageRelocationFailureV1::MissingOffset);
    };
    Ok(*relocation)
}

fn verify_image_patch(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &ConeImagePlanV1,
    member: SlibMemberId,
    primary_file_start: u64,
) -> Result<VerifiedMaterializedPatchSiteV1, ConeImageValidationError> {
    let patch = patch_sites
        .sites()
        .iter()
        .find(|site| site.intent() == plan.fingerprint_patch())
        .copied()
        .ok_or(ConeImageValidationError::MissingPatch(
            plan.fingerprint_patch(),
        ))?;
    let expected_source = scoop_identity::DigestNodeId::from_key(&DigestNodeKey::runtime_image(
        plan.cone().identity(),
    ))
    .expect("runtime image digest node identity hashes");
    let expected_checked_offset = primary_file_start
        .checked_add(RUNTIME_IMAGE_FINGERPRINT_OFFSET)
        .ok_or(ConeImageValidationError::RecordRangeOverflow(
            ConeImageAtomRoleV1::Primary,
        ))?;
    let kind = if patch.source() != expected_source {
        Some(ConeImagePatchFailureV1::Source)
    } else if patch.semantic_field_role() != DigestSemanticFieldRole::RuntimeImage {
        Some(ConeImagePatchFailureV1::SemanticFieldRole)
    } else if patch.member() != member {
        Some(ConeImagePatchFailureV1::Member)
    } else if patch.definition() != plan.definition_plan() {
        Some(ConeImagePatchFailureV1::Definition)
    } else if patch.atom() != plan.primary_atom() {
        Some(ConeImagePatchFailureV1::Atom)
    } else if patch.atom_role() != DefinitionAtomRole::Primary {
        Some(ConeImagePatchFailureV1::AtomRole)
    } else if patch.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(ConeImagePatchFailureV1::SectionRole)
    } else if patch.offset_within_atom() != RUNTIME_IMAGE_FINGERPRINT_OFFSET {
        Some(ConeImagePatchFailureV1::OffsetWithinAtom)
    } else if patch.checked_offset() != expected_checked_offset {
        Some(ConeImagePatchFailureV1::CheckedOffset)
    } else if patch.width_bytes() != DIGEST_WIDTH {
        Some(ConeImagePatchFailureV1::Width)
    } else {
        None
    };
    if let Some(kind) = kind {
        return Err(ConeImageValidationError::PatchMismatch {
            intent: plan.fingerprint_patch(),
            kind,
        });
    }
    for site in patch_sites
        .sites()
        .iter()
        .filter(|site| site.definition() == plan.definition_plan())
    {
        if site.intent() != plan.fingerprint_patch() || site.atom() != plan.primary_atom() {
            return Err(ConeImageValidationError::UnexpectedPatch {
                atom: site.atom(),
                intent: site.intent(),
            });
        }
    }
    Ok(patch)
}

fn validate_digest_node(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &ConeImagePlanV1,
) -> Result<(), ConeImageValidationError> {
    let node = patch_sites
        .digest_plan()
        .nodes()
        .iter()
        .find(|node| node.key() == &DigestNodeKey::runtime_image(plan.cone().identity()))
        .ok_or(ConeImageValidationError::DigestNodeMismatch)?;
    if node.patch_intents().len() != 1 || node.patch_intents()[0].id() != plan.fingerprint_patch() {
        return Err(ConeImageValidationError::DigestNodeMismatch);
    }
    Ok(())
}

fn relocation_error<T>(
    role: ConeImageAtomRoleV1,
    index: usize,
    kind: ConeImageRelocationFailureV1,
) -> Result<T, ConeImageValidationError> {
    Err(ConeImageValidationError::RelocationMismatch { role, index, kind })
}
