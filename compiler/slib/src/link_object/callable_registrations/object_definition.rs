use scoop_identity::{
    DefinitionAtomRole, DigestKind, DigestNodeId, ObjectDefinitionAtomId, PersistentExactTypeId,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::link_object::{
    CanonicalObjectDefinitionRequirementV1, CanonicalUndefinedRelocationUseV1,
    FinalUndefinedSymbolRequirementV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    StrongDefinitionOwnerV1, StrongRelocationResolutionV1,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1, VerifiedObjectDefinitionRequirementSetV1,
    VerifiedObjectRelocationFormV1, VerifiedObjectRelocationShapeV1, VerifiedRelocationTargetV1,
};

const PRIMARY_ATOM_ROLE: u32 = 1;

mod relocations;
mod runtime_encoding;

pub(in crate::link_object) use relocations::canonicalize_relocations_with_associated_atoms;

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
    form: VerifiedObjectRelocationFormV1,
    encoded_value: u64,
    canonical_value: u64,
    targets: Vec<CanonicalRelocationTargetV1>,
}

impl CanonicalObjectRelocationV1 {
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

    pub(in crate::link_object) fn dependency_target(
        offset_within_atom: u64,
        provider: scoop_identity::ConeIdentity,
        subject: scoop_lir::ExternalStrongShapeSubjectV1,
    ) -> Self {
        Self {
            offset_within_atom,
            form: VerifiedObjectRelocationFormV1::Unsigned64,
            encoded_value: 0,
            canonical_value: 0,
            targets: vec![CanonicalRelocationTargetV1 {
                slot: RelocationTargetSlotV1::Single,
                target: CanonicalRelocationTargetKindV1::Requirement(
                    CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong {
                        provider,
                        subject,
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
            form: VerifiedObjectRelocationFormV1::Unsigned64,
            encoded_value: 0,
            canonical_value: 0,
            targets: vec![CanonicalRelocationTargetV1 {
                slot: RelocationTargetSlotV1::Single,
                target: CanonicalRelocationTargetKindV1::Requirement(
                    CanonicalObjectDefinitionRequirementV1::Legacy(requirement),
                ),
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
            form: VerifiedObjectRelocationFormV1::Unsigned64,
            encoded_value: 0,
            canonical_value: 0,
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
            VerifiedObjectRelocationFormV1::ElfRela { width, .. } => {
                let slot = bytes
                    .get_mut(start..start + usize::from(width))
                    .ok_or(ObjectDefinitionRelocationFailureV1::Range)?;
                if slot != &self.encoded_value.to_le_bytes()[..usize::from(width)] {
                    return Err(ObjectDefinitionRelocationFailureV1::EncodedValue);
                }
                slot.fill(0);
            }
            VerifiedObjectRelocationFormV1::Unsigned64
            | VerifiedObjectRelocationFormV1::Subtractor64 => {
                let slot = bytes
                    .get_mut(start..start + 8)
                    .ok_or(ObjectDefinitionRelocationFailureV1::Range)?;
                let actual = u64::from_le_bytes(slot.try_into().expect("eight-byte slot"));
                if actual != self.encoded_value {
                    return Err(ObjectDefinitionRelocationFailureV1::EncodedValue);
                }
                slot.fill(0);
            }
            VerifiedObjectRelocationFormV1::Branch26 => {
                normalize_u32(bytes, start, self.encoded_value, 0xfc00_0000)?;
            }
            VerifiedObjectRelocationFormV1::Page21 { .. }
            | VerifiedObjectRelocationFormV1::GotLoadPage21
            | VerifiedObjectRelocationFormV1::TlvpLoadPage21 => {
                normalize_u32(bytes, start, self.encoded_value, 0x9f00_001f)?;
            }
            VerifiedObjectRelocationFormV1::PageOffset12 { .. }
            | VerifiedObjectRelocationFormV1::GotLoadPageOffset12
            | VerifiedObjectRelocationFormV1::TlvpLoadPageOffset12 => {
                normalize_u32(bytes, start, self.encoded_value, 0xffc0_03ff)?;
            }
            VerifiedObjectRelocationFormV1::PointerToGot32 => {
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
            && self.form == VerifiedObjectRelocationFormV1::Unsigned64
            && self.canonical_value == 0
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
    Literal(crate::link_object::literal_pools::ObjectLiteral),
    Requirement(CanonicalObjectDefinitionRequirementV1),
    OwningAssociatedAtomOffset {
        atom: ObjectDefinitionAtomId,
        role: DefinitionAtomRole,
        offset_within_atom: u64,
    },
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
