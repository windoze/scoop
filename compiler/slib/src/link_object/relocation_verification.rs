//! Resolve actual relocation references against definition and symbol indices.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
};

use super::{
    BuiltinObjectSectionRoleV1, DarwinArm64RelocationShapeV1, DarwinArm64RelocationTargetV1,
    MemberObjectDefinitionIndexV1, ObjectSymbolKindV1, PlannedStrongObjectSymbolRoleV1,
};
use crate::SlibMemberId;

mod resolution;
use resolution::resolve_shape;
mod pointer;
mod semantics;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VerifiedBoundaryRoleV1 {
    Start,
    End,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedRelocationTargetV1 {
    StrongDefinition {
        definition: ObjectDefinitionPlanId,
    },
    LocalDefinition {
        table_index: u32,
        name: Vec<u8>,
        owner_atom: Option<ObjectDefinitionAtomId>,
        section_ordinal: NonZeroU32,
        value: u64,
    },
    ExternalUndefined {
        table_index: u32,
        name: Vec<u8>,
    },
    SectionBase {
        section_ordinal: NonZeroU32,
        section_role: BuiltinObjectSectionRoleV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedObjectRelocationShapeV1 {
    CoffAmd64 {
        kind: u16,
        width: u8,
        target: VerifiedRelocationTargetV1,
    },
    ElfX86_64Rela {
        kind: u32,
        addend: i64,
        width: u8,
        target: VerifiedRelocationTargetV1,
    },
    Unsigned64 {
        target: VerifiedRelocationTargetV1,
    },
    Subtractor64 {
        minuend: VerifiedRelocationTargetV1,
        subtrahend: VerifiedRelocationTargetV1,
    },
    Branch26 {
        target: VerifiedRelocationTargetV1,
    },
    Page21 {
        target: VerifiedRelocationTargetV1,
        explicit_addend: Option<i32>,
    },
    PageOffset12 {
        target: VerifiedRelocationTargetV1,
        explicit_addend: Option<i32>,
    },
    GotLoadPage21 {
        target: VerifiedRelocationTargetV1,
    },
    GotLoadPageOffset12 {
        target: VerifiedRelocationTargetV1,
    },
    PointerToGot32 {
        target: VerifiedRelocationTargetV1,
    },
    TlvpLoadPage21 {
        target: VerifiedRelocationTargetV1,
    },
    TlvpLoadPageOffset12 {
        target: VerifiedRelocationTargetV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VerifiedObjectRelocationFormV1 {
    CoffAmd64 { kind: u16, width: u8 },
    ElfX86_64Rela { kind: u32, addend: i64, width: u8 },
    Unsigned64,
    Subtractor64,
    Branch26,
    Page21 { explicit_addend: Option<i32> },
    PageOffset12 { explicit_addend: Option<i32> },
    GotLoadPage21,
    GotLoadPageOffset12,
    PointerToGot32,
    TlvpLoadPage21,
    TlvpLoadPageOffset12,
}

impl VerifiedObjectRelocationShapeV1 {
    pub const fn form(&self) -> VerifiedObjectRelocationFormV1 {
        match self {
            Self::CoffAmd64 { kind, width, .. } => VerifiedObjectRelocationFormV1::CoffAmd64 {
                kind: *kind,
                width: *width,
            },
            Self::ElfX86_64Rela {
                kind,
                addend,
                width,
                ..
            } => VerifiedObjectRelocationFormV1::ElfX86_64Rela {
                kind: *kind,
                addend: *addend,
                width: *width,
            },
            Self::Unsigned64 { .. } => VerifiedObjectRelocationFormV1::Unsigned64,
            Self::Subtractor64 { .. } => VerifiedObjectRelocationFormV1::Subtractor64,
            Self::Branch26 { .. } => VerifiedObjectRelocationFormV1::Branch26,
            Self::Page21 {
                explicit_addend, ..
            } => VerifiedObjectRelocationFormV1::Page21 {
                explicit_addend: *explicit_addend,
            },
            Self::PageOffset12 {
                explicit_addend, ..
            } => VerifiedObjectRelocationFormV1::PageOffset12 {
                explicit_addend: *explicit_addend,
            },
            Self::GotLoadPage21 { .. } => VerifiedObjectRelocationFormV1::GotLoadPage21,
            Self::GotLoadPageOffset12 { .. } => VerifiedObjectRelocationFormV1::GotLoadPageOffset12,
            Self::PointerToGot32 { .. } => VerifiedObjectRelocationFormV1::PointerToGot32,
            Self::TlvpLoadPage21 { .. } => VerifiedObjectRelocationFormV1::TlvpLoadPage21,
            Self::TlvpLoadPageOffset12 { .. } => {
                VerifiedObjectRelocationFormV1::TlvpLoadPageOffset12
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRelocationUseV1 {
    member: SlibMemberId,
    containing_atom: ObjectDefinitionAtomId,
    containing_atom_role: DefinitionAtomRole,
    section_role: BuiltinObjectSectionRoleV1,
    offset_within_atom: u64,
    width_bytes: u8,
    encoded_value: u64,
    shape: VerifiedObjectRelocationShapeV1,
}

impl VerifiedRelocationUseV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn containing_atom(&self) -> ObjectDefinitionAtomId {
        self.containing_atom
    }

    pub const fn containing_atom_role(&self) -> DefinitionAtomRole {
        self.containing_atom_role
    }

    pub const fn section_role(&self) -> BuiltinObjectSectionRoleV1 {
        self.section_role
    }

    pub const fn offset_within_atom(&self) -> u64 {
        self.offset_within_atom
    }

    pub const fn width_bytes(&self) -> u8 {
        self.width_bytes
    }

    pub const fn encoded_value(&self) -> u64 {
        self.encoded_value
    }

    pub const fn shape(&self) -> &VerifiedObjectRelocationShapeV1 {
        &self.shape
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedMemberObjectRelocationIndexV1 {
    definitions: MemberObjectDefinitionIndexV1,
    relocations: Vec<VerifiedRelocationUseV1>,
    relocations_by_atom: BTreeMap<ObjectDefinitionAtomId, Vec<usize>>,
}

impl VerifiedMemberObjectRelocationIndexV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.definitions.producer()
    }

    pub const fn member(&self) -> SlibMemberId {
        self.definitions.member()
    }

    pub const fn definitions(&self) -> &MemberObjectDefinitionIndexV1 {
        &self.definitions
    }

    pub fn relocations(&self) -> &[VerifiedRelocationUseV1] {
        &self.relocations
    }

    pub(in crate::link_object) fn relocations_for_atom(
        &self,
        atom: ObjectDefinitionAtomId,
    ) -> impl Iterator<Item = &VerifiedRelocationUseV1> {
        self.relocations_by_atom
            .get(&atom)
            .into_iter()
            .flatten()
            .map(|&index| &self.relocations[index])
    }
}

pub fn verify_member_object_relocations_v1(
    definitions: MemberObjectDefinitionIndexV1,
) -> Result<VerifiedMemberObjectRelocationIndexV1, ObjectRelocationValidationError> {
    let local_symbol_owners = local_symbol_owners(&definitions);
    let envelope = definitions.sections().envelope();
    let mut relocations = Vec::with_capacity(envelope.relocations().len());
    let mut relocations_by_atom: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for relocation in envelope.relocations() {
        let section_ordinal = relocation.containing_section_ordinal();
        let section_index = section_ordinal.get() as usize - 1;
        if definitions.sections().roles()[section_index]
            == BuiltinObjectSectionRoleV1::ObjectMetadata
        {
            continue;
        }
        let section = &envelope.sections()[section_index];
        let site_start = section
            .virtual_address()
            .checked_add(relocation.offset())
            .ok_or(ObjectRelocationValidationError::RelocationSiteOverflow)?;
        let width_bytes = relocation.shape().width_bytes();
        let site_end = site_start
            .checked_add(u64::from(width_bytes))
            .ok_or(ObjectRelocationValidationError::RelocationSiteOverflow)?;
        let containing_atoms = definitions
            .definitions()
            .iter()
            .flat_map(|definition| definition.atoms())
            .filter(|atom| {
                atom.section_ordinal() == section_ordinal
                    && atom.start() <= site_start
                    && site_end <= atom.end()
            })
            .collect::<Vec<_>>();
        if containing_atoms.is_empty() {
            return Err(ObjectRelocationValidationError::OrphanRelocation {
                section: relocation.containing_section_ordinal(),
                offset: relocation.offset(),
                width_bytes,
            });
        }
        let shape = resolve_shape(relocation.shape(), &definitions, &local_symbol_owners)?;
        // A complete parent table aliases a slice of its child's immutable
        // array. Each view consumes the relocation at its own relative offset.
        for containing_atom in containing_atoms {
            relocations_by_atom
                .entry(containing_atom.atom())
                .or_default()
                .push(relocations.len());
            relocations.push(VerifiedRelocationUseV1 {
                member: definitions.member(),
                containing_atom: containing_atom.atom(),
                containing_atom_role: containing_atom.atom_role(),
                section_role: definitions.sections().roles()[section_index],
                offset_within_atom: site_start - containing_atom.start(),
                width_bytes,
                encoded_value: relocation.encoded_value(),
                shape: shape.clone(),
            });
        }
    }
    Ok(VerifiedMemberObjectRelocationIndexV1 {
        definitions,
        relocations,
        relocations_by_atom,
    })
}

fn local_symbol_owners(
    definitions: &MemberObjectDefinitionIndexV1,
) -> BTreeMap<u32, ObjectDefinitionAtomId> {
    definitions
        .sections()
        .envelope()
        .symbols()
        .iter()
        .filter(|symbol| symbol.kind() == ObjectSymbolKindV1::LocalSectionDefinition)
        .filter_map(|symbol| {
            let section = symbol.section_ordinal()?;
            let owner = definitions
                .definitions()
                .iter()
                .flat_map(|definition| definition.atoms())
                .find(|atom| {
                    atom.section_ordinal() == section
                        && atom.start() <= symbol.value()
                        && symbol.value() < atom.end()
                })?;
            Some((symbol.table_index(), owner.atom()))
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectRelocationValidationError {
    UnsupportedContainingSectionOrdinal {
        section: NonZeroU32,
    },
    RelocationSiteOverflow,
    OrphanRelocation {
        section: NonZeroU32,
        offset: u64,
        width_bytes: u8,
    },
    InvalidSectionTarget {
        section: NonZeroU32,
    },
    InvalidSymbolTarget {
        table_index: u32,
    },
    UnplannedStrongDefinitionTarget {
        table_index: u32,
    },
    BoundaryRelocationTarget {
        atom: ObjectDefinitionAtomId,
        boundary: VerifiedBoundaryRoleV1,
    },
}

impl fmt::Display for ObjectRelocationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid member object relocations: {self:?}")
    }
}

impl std::error::Error for ObjectRelocationValidationError {}

#[cfg(test)]
pub(super) mod tests;
