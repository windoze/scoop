use scoop_identity::{
    DefinitionAtomRole, DigestKind, DigestNodeId, ObjectDefinitionAtomId, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentStaticStorageId, StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::link_object::{
    CanonicalObjectDefinitionRequirementV1, CanonicalUndefinedRelocationUseV1,
    FinalUndefinedSymbolRequirementV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    StrongDefinitionOwnerV1, StrongRelocationResolutionV1,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedDarwinArm64RelocationFormV1,
    VerifiedDarwinArm64RelocationShapeV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1, VerifiedObjectDefinitionRequirementSetV1,
    VerifiedRelocationTargetV1,
};

const PRIMARY_ATOM_ROLE: u32 = 1;

mod runtime_encoding;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectDefinitionRelocationFailureV1 {
    BindingCount,
    UnsupportedLocalOrSectionTarget,
    UnsupportedObjectLocalOwner,
    MissingUndefinedRequirement,
    Range,
    EncodedValue,
}

pub(in crate::link_object) struct ObjectDefinitionFingerprintInputV1<'a> {
    pub(in crate::link_object) bytes: &'a [u8],
    pub(in crate::link_object) relocations: &'a [CanonicalObjectRelocationV1],
    pub(in crate::link_object) direct_inputs: &'a [CanonicalDigestInputV1],
}

pub(in crate::link_object) struct CanonicalAssociatedObjectAtomV1<'a> {
    pub(in crate::link_object) atom: ObjectDefinitionAtomId,
    pub(in crate::link_object) role: DefinitionAtomRole,
    pub(in crate::link_object) bytes: &'a [u8],
    pub(in crate::link_object) relocations: &'a [CanonicalObjectRelocationV1],
}

pub(in crate::link_object) struct ObjectDefinitionLeafWithAssociatedAtomsInputV1<'a> {
    pub(in crate::link_object) primary: ObjectDefinitionFingerprintInputV1<'a>,
    pub(in crate::link_object) associated_atoms: &'a [CanonicalAssociatedObjectAtomV1<'a>],
}

#[derive(Clone, Copy)]
pub(in crate::link_object) struct CanonicalDigestInputV1 {
    pub(in crate::link_object) kind: DigestKind,
    pub(in crate::link_object) node: DigestNodeId,
    pub(in crate::link_object) digest: [u8; 32],
}

#[derive(Clone)]
pub(in crate::link_object) struct CanonicalObjectRelocationV1 {
    offset_within_atom: u64,
    form: VerifiedDarwinArm64RelocationFormV1,
    encoded_value: u64,
    targets: Vec<CanonicalRelocationTargetV1>,
}

#[derive(Clone, Copy)]
pub(in crate::link_object) enum CanonicalStaticStorageTargetV1 {
    InitialTemplate(PersistentStaticStorageId),
    InitialRelocationTable(PersistentStaticStorageId),
    EmptyTemplateSentinel,
    EmptyRelocationTableSentinel,
}

impl CanonicalObjectRelocationV1 {
    pub(super) fn callable_entry(body: PersistentCallableBodyId) -> Self {
        let owner = StrongDefinitionOwnerV1::new(
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableBody,
        )
        .expect("callable bodies are valid strong definition owners");
        Self::unsigned64(
            184,
            FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
        )
    }

    pub(in crate::link_object) fn type_descriptor(exact_type: PersistentExactTypeId) -> Self {
        Self::intra_cone_type_descriptor(168, exact_type)
    }

    pub(in crate::link_object) fn intra_cone_type_descriptor(
        offset_within_atom: u64,
        exact_type: PersistentExactTypeId,
    ) -> Self {
        let owner = StrongDefinitionOwnerV1::new(
            StrongDefinitionEntity::exact_type(exact_type),
            StrongDefinitionRole::TypeDescriptor,
        )
        .expect("type descriptors are valid strong definition owners");
        Self::unsigned64(
            offset_within_atom,
            FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
        )
    }

    pub(in crate::link_object) fn dependency_type_descriptor(
        offset_within_atom: u64,
        provider: scoop_identity::ConeIdentity,
        exact_type: PersistentExactTypeId,
    ) -> Self {
        Self {
            offset_within_atom,
            form: VerifiedDarwinArm64RelocationFormV1::Unsigned64,
            encoded_value: 0,
            targets: vec![CanonicalRelocationTargetV1 {
                slot: RelocationTargetSlotV1::Single,
                target: CanonicalRelocationTargetKindV1::Requirement(
                    CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong {
                        provider,
                        subject: scoop_lir::ExternalStrongShapeSubjectV1::TypeDescriptor(
                            exact_type,
                        ),
                    },
                ),
            }],
        }
    }

    pub(in crate::link_object) fn dispatch_table(
        offset_within_atom: u64,
        table: scoop_identity::PersistentDispatchTableId,
    ) -> Self {
        let owner = StrongDefinitionOwnerV1::new(
            StrongDefinitionEntity::dispatch_table(table),
            StrongDefinitionRole::DispatchTable,
        )
        .expect("dispatch tables are valid strong definition owners");
        Self::unsigned64(
            offset_within_atom,
            FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
        )
    }

    pub(in crate::link_object) fn unsigned64(
        offset_within_atom: u64,
        requirement: FinalUndefinedSymbolRequirementV1,
    ) -> Self {
        Self {
            offset_within_atom,
            form: VerifiedDarwinArm64RelocationFormV1::Unsigned64,
            encoded_value: 0,
            targets: vec![CanonicalRelocationTargetV1 {
                slot: RelocationTargetSlotV1::Single,
                target: CanonicalRelocationTargetKindV1::Requirement(
                    CanonicalObjectDefinitionRequirementV1::Legacy(requirement),
                ),
            }],
        }
    }

    pub(in crate::link_object) fn static_storage_target(
        offset_within_atom: u64,
        target: CanonicalStaticStorageTargetV1,
    ) -> Self {
        Self {
            offset_within_atom,
            form: VerifiedDarwinArm64RelocationFormV1::Unsigned64,
            encoded_value: 0,
            targets: vec![CanonicalRelocationTargetV1 {
                slot: RelocationTargetSlotV1::Single,
                target: CanonicalRelocationTargetKindV1::StaticStorage(target),
            }],
        }
    }

    pub(in crate::link_object) fn owning_associated_atom_offset(
        offset_within_atom: u64,
        atom: ObjectDefinitionAtomId,
        role: DefinitionAtomRole,
        target_offset_within_atom: u64,
    ) -> Self {
        Self {
            offset_within_atom,
            form: VerifiedDarwinArm64RelocationFormV1::Unsigned64,
            encoded_value: 0,
            targets: vec![CanonicalRelocationTargetV1 {
                slot: RelocationTargetSlotV1::Single,
                target: CanonicalRelocationTargetKindV1::OwningAssociatedAtomOffset {
                    atom,
                    role,
                    offset_within_atom: target_offset_within_atom,
                },
            }],
        }
    }

    pub(in crate::link_object) fn normalize_bytes(
        &self,
        bytes: &mut [u8],
    ) -> Result<(), ObjectDefinitionRelocationFailureV1> {
        let start = usize::try_from(self.offset_within_atom)
            .map_err(|_| ObjectDefinitionRelocationFailureV1::Range)?;
        match self.form {
            VerifiedDarwinArm64RelocationFormV1::Unsigned64
            | VerifiedDarwinArm64RelocationFormV1::Subtractor64 => {
                let slot = bytes
                    .get_mut(start..start + 8)
                    .ok_or(ObjectDefinitionRelocationFailureV1::Range)?;
                let actual = u64::from_le_bytes(slot.try_into().expect("eight-byte slot"));
                if actual != self.encoded_value {
                    return Err(ObjectDefinitionRelocationFailureV1::EncodedValue);
                }
                slot.fill(0);
            }
            VerifiedDarwinArm64RelocationFormV1::Branch26 => {
                normalize_u32(bytes, start, self.encoded_value, 0xfc00_0000)?;
            }
            VerifiedDarwinArm64RelocationFormV1::Page21 { .. }
            | VerifiedDarwinArm64RelocationFormV1::GotLoadPage21
            | VerifiedDarwinArm64RelocationFormV1::TlvpLoadPage21 => {
                normalize_u32(bytes, start, self.encoded_value, 0x9f00_001f)?;
            }
            VerifiedDarwinArm64RelocationFormV1::PageOffset12 { .. }
            | VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12
            | VerifiedDarwinArm64RelocationFormV1::TlvpLoadPageOffset12 => {
                normalize_u32(bytes, start, self.encoded_value, 0xffc0_03ff)?;
            }
            VerifiedDarwinArm64RelocationFormV1::PointerToGot32 => {
                normalize_u32(bytes, start, self.encoded_value, 0)?;
            }
        }
        Ok(())
    }

    pub(in crate::link_object) fn is_owning_associated_unsigned64(
        &self,
        offset_within_atom: u64,
        atom: ObjectDefinitionAtomId,
        role: DefinitionAtomRole,
        target_offset_within_atom: u64,
    ) -> bool {
        self.offset_within_atom == offset_within_atom
            && self.form == VerifiedDarwinArm64RelocationFormV1::Unsigned64
            && self.encoded_value == 0
            && matches!(
                self.targets.as_slice(),
                [CanonicalRelocationTargetV1 {
                    slot: RelocationTargetSlotV1::Single,
                    target: CanonicalRelocationTargetKindV1::OwningAssociatedAtomOffset {
                        atom: actual_atom,
                        role: actual_role,
                        offset_within_atom: actual_offset,
                    },
                }] if *actual_atom == atom
                    && *actual_role == role
                    && *actual_offset == target_offset_within_atom
            )
    }
}

#[derive(Clone)]
struct CanonicalRelocationTargetV1 {
    slot: RelocationTargetSlotV1,
    target: CanonicalRelocationTargetKindV1,
}

#[derive(Clone, Copy)]
enum CanonicalRelocationTargetKindV1 {
    Requirement(CanonicalObjectDefinitionRequirementV1),
    StaticStorage(CanonicalStaticStorageTargetV1),
    OwningAssociatedAtomOffset {
        atom: ObjectDefinitionAtomId,
        role: DefinitionAtomRole,
        offset_within_atom: u64,
    },
}

pub(in crate::link_object) fn canonicalize_relocations(
    bytes: &[u8],
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: scoop_identity::ObjectDefinitionAtomId,
    closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
) -> Result<Vec<CanonicalObjectRelocationV1>, ObjectDefinitionRelocationFailureV1> {
    canonicalize_relocations_with_associated_atoms(bytes, member, atom, closure, requirements, &[])
}

pub(in crate::link_object) fn canonicalize_relocations_with_associated_atoms(
    bytes: &[u8],
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: scoop_identity::ObjectDefinitionAtomId,
    closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    associated_atoms: &[VerifiedDefinitionAtomRangeV1],
) -> Result<Vec<CanonicalObjectRelocationV1>, ObjectDefinitionRelocationFailureV1> {
    let mut output = Vec::new();
    for relocation in member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == atom)
    {
        let bindings = closure
            .bindings()
            .iter()
            .filter(|binding| {
                binding.source_member() == member.member()
                    && binding.containing_atom() == atom
                    && binding.offset_within_atom() == relocation.offset_within_atom()
            })
            .collect::<Vec<_>>();
        let mut binding_count = 0;
        let mut targets = Vec::new();
        for (slot, target) in relocation_targets(relocation.shape()) {
            let target = match target {
                VerifiedRelocationTargetV1::LocalDefinition {
                    owner_atom: Some(target_atom),
                    section_ordinal,
                    value,
                    ..
                } => {
                    let range = associated_atoms
                        .iter()
                        .find(|range| range.atom() == *target_atom)
                        .ok_or(
                            ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget,
                        )?;
                    if range.section_ordinal() != *section_ordinal
                        || *value < range.start()
                        || *value >= range.end()
                    {
                        return Err(
                            ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget,
                        );
                    }
                    CanonicalRelocationTargetKindV1::OwningAssociatedAtomOffset {
                        atom: *target_atom,
                        role: range.atom_role(),
                        offset_within_atom: *value - range.start(),
                    }
                }
                VerifiedRelocationTargetV1::LocalDefinition { .. }
                | VerifiedRelocationTargetV1::SectionBase { .. } => {
                    return Err(
                        ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget,
                    );
                }
                VerifiedRelocationTargetV1::StrongDefinition { .. }
                | VerifiedRelocationTargetV1::ExternalUndefined { .. } => {
                    let binding = bindings
                        .iter()
                        .find(|binding| binding.target_slot() == slot)
                        .ok_or(ObjectDefinitionRelocationFailureV1::BindingCount)?;
                    binding_count += 1;
                    CanonicalRelocationTargetKindV1::Requirement(canonical_requirement(
                        binding,
                        requirements,
                    )?)
                }
            };
            targets.push(CanonicalRelocationTargetV1 { slot, target });
        }
        if bindings.len() != binding_count {
            return Err(ObjectDefinitionRelocationFailureV1::BindingCount);
        }
        targets.sort_unstable_by_key(|target| target.slot);
        output.push(CanonicalObjectRelocationV1 {
            offset_within_atom: relocation.offset_within_atom(),
            form: relocation.shape().form(),
            encoded_value: relocation.encoded_value(),
            targets,
        });
    }
    output.sort_unstable_by_key(|relocation| relocation.offset_within_atom);
    for relocation in &output {
        let start = usize::try_from(relocation.offset_within_atom)
            .map_err(|_| ObjectDefinitionRelocationFailureV1::Range)?;
        let width = match relocation.form {
            VerifiedDarwinArm64RelocationFormV1::Unsigned64
            | VerifiedDarwinArm64RelocationFormV1::Subtractor64 => 8,
            _ => 4,
        };
        if bytes.get(start..start + width).is_none() {
            return Err(ObjectDefinitionRelocationFailureV1::Range);
        }
    }
    Ok(output)
}

fn relocation_targets(
    shape: &VerifiedDarwinArm64RelocationShapeV1,
) -> Vec<(RelocationTargetSlotV1, &VerifiedRelocationTargetV1)> {
    match shape {
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { target }
        | VerifiedDarwinArm64RelocationShapeV1::Branch26 { target }
        | VerifiedDarwinArm64RelocationShapeV1::Page21 { target, .. }
        | VerifiedDarwinArm64RelocationShapeV1::PageOffset12 { target, .. }
        | VerifiedDarwinArm64RelocationShapeV1::GotLoadPage21 { target }
        | VerifiedDarwinArm64RelocationShapeV1::GotLoadPageOffset12 { target }
        | VerifiedDarwinArm64RelocationShapeV1::PointerToGot32 { target }
        | VerifiedDarwinArm64RelocationShapeV1::TlvpLoadPage21 { target }
        | VerifiedDarwinArm64RelocationShapeV1::TlvpLoadPageOffset12 { target } => {
            vec![(RelocationTargetSlotV1::Single, target)]
        }
        VerifiedDarwinArm64RelocationShapeV1::Subtractor64 {
            minuend,
            subtrahend,
        } => vec![
            (RelocationTargetSlotV1::Minuend, minuend),
            (RelocationTargetSlotV1::Subtrahend, subtrahend),
        ],
    }
}

fn canonical_requirement(
    binding: &crate::link_object::StrongRelocationBindingV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
) -> Result<CanonicalObjectDefinitionRequirementV1, ObjectDefinitionRelocationFailureV1> {
    match binding.resolution() {
        StrongRelocationResolutionV1::ObjectLocalStrong { owner, .. } => match owner {
            LinkDefinitionOwnerV1::StrongDefinition(owner) => {
                Ok(CanonicalObjectDefinitionRequirementV1::Legacy(
                    FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
                ))
            }
            _ => Err(ObjectDefinitionRelocationFailureV1::UnsupportedObjectLocalOwner),
        },
        StrongRelocationResolutionV1::CurrentConeUndefinedStrong { .. }
        | StrongRelocationResolutionV1::ExternalCandidate { .. } => {
            let use_site = CanonicalUndefinedRelocationUseV1::from(binding);
            requirements
                .requirement_for(&use_site)
                .ok_or(ObjectDefinitionRelocationFailureV1::MissingUndefinedRequirement)
        }
    }
}

fn normalize_u32(
    bytes: &mut [u8],
    start: usize,
    expected: u64,
    keep_mask: u32,
) -> Result<(), ObjectDefinitionRelocationFailureV1> {
    let slot = bytes
        .get_mut(start..start + 4)
        .ok_or(ObjectDefinitionRelocationFailureV1::Range)?;
    let actual = u32::from_le_bytes(slot.try_into().expect("four-byte slot"));
    if u64::from(actual) != expected {
        return Err(ObjectDefinitionRelocationFailureV1::EncodedValue);
    }
    slot.copy_from_slice(&(actual & keep_mask).to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests;
