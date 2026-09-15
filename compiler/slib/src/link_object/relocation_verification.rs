//! Member-aware relocation ownership over a verified strong-definition index.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::{NonZeroU8, NonZeroU32};

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
};

use super::{
    BuiltinObjectSectionRoleV1, DarwinArm64RelocationShapeV1, DarwinArm64RelocationTargetV1,
    DarwinArm64SymbolKindV1, PlannedStrongObjectSymbolRoleV1,
    VerifiedMemberStrongObjectDefinitionIndexV1,
};
use crate::SlibMemberId;

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
        section_ordinal: NonZeroU8,
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
pub enum VerifiedDarwinArm64RelocationShapeV1 {
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
pub enum VerifiedDarwinArm64RelocationFormV1 {
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

impl VerifiedDarwinArm64RelocationShapeV1 {
    pub const fn form(&self) -> VerifiedDarwinArm64RelocationFormV1 {
        match self {
            Self::Unsigned64 { .. } => VerifiedDarwinArm64RelocationFormV1::Unsigned64,
            Self::Subtractor64 { .. } => VerifiedDarwinArm64RelocationFormV1::Subtractor64,
            Self::Branch26 { .. } => VerifiedDarwinArm64RelocationFormV1::Branch26,
            Self::Page21 {
                explicit_addend, ..
            } => VerifiedDarwinArm64RelocationFormV1::Page21 {
                explicit_addend: *explicit_addend,
            },
            Self::PageOffset12 {
                explicit_addend, ..
            } => VerifiedDarwinArm64RelocationFormV1::PageOffset12 {
                explicit_addend: *explicit_addend,
            },
            Self::GotLoadPage21 { .. } => VerifiedDarwinArm64RelocationFormV1::GotLoadPage21,
            Self::GotLoadPageOffset12 { .. } => {
                VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12
            }
            Self::PointerToGot32 { .. } => VerifiedDarwinArm64RelocationFormV1::PointerToGot32,
            Self::TlvpLoadPage21 { .. } => VerifiedDarwinArm64RelocationFormV1::TlvpLoadPage21,
            Self::TlvpLoadPageOffset12 { .. } => {
                VerifiedDarwinArm64RelocationFormV1::TlvpLoadPageOffset12
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
    shape: VerifiedDarwinArm64RelocationShapeV1,
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

    pub const fn shape(&self) -> &VerifiedDarwinArm64RelocationShapeV1 {
        &self.shape
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedMemberObjectRelocationIndexV1 {
    definitions: VerifiedMemberStrongObjectDefinitionIndexV1,
    relocations: Vec<VerifiedRelocationUseV1>,
}

impl VerifiedMemberObjectRelocationIndexV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.definitions.producer()
    }

    pub const fn member(&self) -> SlibMemberId {
        self.definitions.member()
    }

    pub const fn definitions(&self) -> &VerifiedMemberStrongObjectDefinitionIndexV1 {
        &self.definitions
    }

    pub fn relocations(&self) -> &[VerifiedRelocationUseV1] {
        &self.relocations
    }
}

pub fn verify_member_object_relocations_v1(
    definitions: VerifiedMemberStrongObjectDefinitionIndexV1,
) -> Result<VerifiedMemberObjectRelocationIndexV1, ObjectRelocationValidationError> {
    validate_unique_undefined_symbols(&definitions)?;
    let local_symbol_owners = local_symbol_owners(&definitions);
    let envelope = definitions.sections().envelope();
    let mut used_undefined_symbols = BTreeSet::new();
    let mut relocations = Vec::with_capacity(envelope.relocations().len());
    for relocation in envelope.relocations() {
        let section_ordinal = u8::try_from(relocation.containing_section_ordinal().get())
            .ok()
            .and_then(NonZeroU8::new)
            .ok_or(
                ObjectRelocationValidationError::UnsupportedContainingSectionOrdinal {
                    section: relocation.containing_section_ordinal(),
                },
            )?;
        let section_index = usize::from(section_ordinal.get()) - 1;
        let section = envelope.sections()[section_index];
        let site_start = section
            .virtual_address()
            .checked_add(u64::from(relocation.offset()))
            .ok_or(ObjectRelocationValidationError::RelocationSiteOverflow)?;
        let width_bytes = relocation.shape().width_bytes();
        let site_end = site_start
            .checked_add(u64::from(width_bytes))
            .ok_or(ObjectRelocationValidationError::RelocationSiteOverflow)?;
        let containing_atom = definitions
            .definitions()
            .iter()
            .flat_map(|definition| definition.atoms())
            .find(|atom| {
                atom.section_ordinal() == section_ordinal
                    && atom.start() <= site_start
                    && site_end <= atom.end()
            })
            .copied()
            .ok_or(ObjectRelocationValidationError::OrphanRelocation {
                section: relocation.containing_section_ordinal(),
                offset: relocation.offset(),
                width_bytes,
            })?;
        let shape = resolve_shape(
            relocation.shape(),
            &definitions,
            &local_symbol_owners,
            &mut used_undefined_symbols,
        )?;
        relocations.push(VerifiedRelocationUseV1 {
            member: definitions.member(),
            containing_atom: containing_atom.atom(),
            containing_atom_role: containing_atom.atom_role(),
            section_role: definitions.sections().roles()[section_index],
            offset_within_atom: site_start - containing_atom.start(),
            width_bytes,
            encoded_value: relocation.encoded_value(),
            shape,
        });
    }
    if let Some(symbol) = envelope.symbols().iter().find(|symbol| {
        symbol.kind() == DarwinArm64SymbolKindV1::ExternalUndefined
            && !used_undefined_symbols.contains(&symbol.table_index())
    }) {
        return Err(ObjectRelocationValidationError::UnusedExternalUndefined {
            table_index: symbol.table_index(),
            name: symbol.name().to_vec(),
        });
    }
    Ok(VerifiedMemberObjectRelocationIndexV1 {
        definitions,
        relocations,
    })
}

fn local_symbol_owners(
    definitions: &VerifiedMemberStrongObjectDefinitionIndexV1,
) -> BTreeMap<u32, ObjectDefinitionAtomId> {
    definitions
        .sections()
        .envelope()
        .symbols()
        .iter()
        .filter(|symbol| symbol.kind() == DarwinArm64SymbolKindV1::LocalSectionDefinition)
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

fn validate_unique_undefined_symbols(
    definitions: &VerifiedMemberStrongObjectDefinitionIndexV1,
) -> Result<(), ObjectRelocationValidationError> {
    let mut names = BTreeMap::<Vec<u8>, u32>::new();
    for symbol in definitions
        .sections()
        .envelope()
        .symbols()
        .iter()
        .filter(|symbol| symbol.kind() == DarwinArm64SymbolKindV1::ExternalUndefined)
    {
        if let Some(first) = names.insert(symbol.name().to_vec(), symbol.table_index()) {
            return Err(
                ObjectRelocationValidationError::DuplicateExternalUndefinedSymbol {
                    name: symbol.name().to_vec(),
                    first_table_index: first,
                    second_table_index: symbol.table_index(),
                },
            );
        }
    }
    Ok(())
}

fn resolve_shape(
    shape: DarwinArm64RelocationShapeV1,
    definitions: &VerifiedMemberStrongObjectDefinitionIndexV1,
    local_symbol_owners: &BTreeMap<u32, ObjectDefinitionAtomId>,
    used_undefined_symbols: &mut BTreeSet<u32>,
) -> Result<VerifiedDarwinArm64RelocationShapeV1, ObjectRelocationValidationError> {
    Ok(match shape {
        DarwinArm64RelocationShapeV1::Unsigned64 { target } => {
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                target: resolve_target(
                    target,
                    definitions,
                    local_symbol_owners,
                    used_undefined_symbols,
                )?,
            }
        }
        DarwinArm64RelocationShapeV1::Subtractor64 {
            minuend,
            subtrahend,
        } => VerifiedDarwinArm64RelocationShapeV1::Subtractor64 {
            minuend: resolve_target(
                minuend,
                definitions,
                local_symbol_owners,
                used_undefined_symbols,
            )?,
            subtrahend: resolve_target(
                subtrahend,
                definitions,
                local_symbol_owners,
                used_undefined_symbols,
            )?,
        },
        DarwinArm64RelocationShapeV1::Branch26 { target } => {
            VerifiedDarwinArm64RelocationShapeV1::Branch26 {
                target: resolve_target(
                    target,
                    definitions,
                    local_symbol_owners,
                    used_undefined_symbols,
                )?,
            }
        }
        DarwinArm64RelocationShapeV1::Page21 {
            target,
            explicit_addend,
        } => VerifiedDarwinArm64RelocationShapeV1::Page21 {
            target: resolve_target(
                target,
                definitions,
                local_symbol_owners,
                used_undefined_symbols,
            )?,
            explicit_addend,
        },
        DarwinArm64RelocationShapeV1::PageOffset12 {
            target,
            explicit_addend,
        } => VerifiedDarwinArm64RelocationShapeV1::PageOffset12 {
            target: resolve_target(
                target,
                definitions,
                local_symbol_owners,
                used_undefined_symbols,
            )?,
            explicit_addend,
        },
        DarwinArm64RelocationShapeV1::GotLoadPage21 { target } => {
            VerifiedDarwinArm64RelocationShapeV1::GotLoadPage21 {
                target: resolve_target(
                    target,
                    definitions,
                    local_symbol_owners,
                    used_undefined_symbols,
                )?,
            }
        }
        DarwinArm64RelocationShapeV1::GotLoadPageOffset12 { target } => {
            VerifiedDarwinArm64RelocationShapeV1::GotLoadPageOffset12 {
                target: resolve_target(
                    target,
                    definitions,
                    local_symbol_owners,
                    used_undefined_symbols,
                )?,
            }
        }
        DarwinArm64RelocationShapeV1::PointerToGot32 { target } => {
            VerifiedDarwinArm64RelocationShapeV1::PointerToGot32 {
                target: resolve_target(
                    target,
                    definitions,
                    local_symbol_owners,
                    used_undefined_symbols,
                )?,
            }
        }
        DarwinArm64RelocationShapeV1::TlvpLoadPage21 { target } => {
            VerifiedDarwinArm64RelocationShapeV1::TlvpLoadPage21 {
                target: resolve_target(
                    target,
                    definitions,
                    local_symbol_owners,
                    used_undefined_symbols,
                )?,
            }
        }
        DarwinArm64RelocationShapeV1::TlvpLoadPageOffset12 { target } => {
            VerifiedDarwinArm64RelocationShapeV1::TlvpLoadPageOffset12 {
                target: resolve_target(
                    target,
                    definitions,
                    local_symbol_owners,
                    used_undefined_symbols,
                )?,
            }
        }
    })
}

fn resolve_target(
    target: DarwinArm64RelocationTargetV1,
    definitions: &VerifiedMemberStrongObjectDefinitionIndexV1,
    local_symbol_owners: &BTreeMap<u32, ObjectDefinitionAtomId>,
    used_undefined_symbols: &mut BTreeSet<u32>,
) -> Result<VerifiedRelocationTargetV1, ObjectRelocationValidationError> {
    match target {
        DarwinArm64RelocationTargetV1::SectionOrdinal(section_ordinal) => {
            let index = usize::try_from(section_ordinal.get() - 1).map_err(|_| {
                ObjectRelocationValidationError::InvalidSectionTarget {
                    section: section_ordinal,
                }
            })?;
            let section_role = definitions.sections().roles().get(index).copied().ok_or(
                ObjectRelocationValidationError::InvalidSectionTarget {
                    section: section_ordinal,
                },
            )?;
            Ok(VerifiedRelocationTargetV1::SectionBase {
                section_ordinal,
                section_role,
            })
        }
        DarwinArm64RelocationTargetV1::SymbolTableIndex(table_index) => {
            if let Some(strong) = definitions.strong_symbol_by_table_index(table_index) {
                return match strong.role() {
                    PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { definition, .. } => {
                        Ok(VerifiedRelocationTargetV1::StrongDefinition { definition })
                    }
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { atom, .. } => {
                        let symbol = definitions
                            .sections()
                            .envelope()
                            .symbols()
                            .get(table_index as usize)
                            .ok_or(ObjectRelocationValidationError::InvalidSymbolTarget {
                                table_index,
                            })?;
                        let section_ordinal = symbol.section_ordinal().ok_or(
                            ObjectRelocationValidationError::InvalidSymbolTarget { table_index },
                        )?;
                        Ok(VerifiedRelocationTargetV1::LocalDefinition {
                            table_index,
                            name: symbol.name().to_vec(),
                            owner_atom: Some(atom),
                            section_ordinal,
                            value: symbol.value(),
                        })
                    }
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { atom, .. } => {
                        Err(ObjectRelocationValidationError::BoundaryRelocationTarget {
                            atom,
                            boundary: VerifiedBoundaryRoleV1::End,
                        })
                    }
                };
            }
            let symbol = definitions
                .sections()
                .envelope()
                .symbols()
                .get(table_index as usize)
                .ok_or(ObjectRelocationValidationError::InvalidSymbolTarget { table_index })?;
            match symbol.kind() {
                DarwinArm64SymbolKindV1::ExternalStrongDefinition => Err(
                    ObjectRelocationValidationError::UnplannedStrongDefinitionTarget {
                        table_index,
                    },
                ),
                DarwinArm64SymbolKindV1::LocalSectionDefinition => {
                    let section_ordinal = symbol.section_ordinal().ok_or(
                        ObjectRelocationValidationError::InvalidSymbolTarget { table_index },
                    )?;
                    Ok(VerifiedRelocationTargetV1::LocalDefinition {
                        table_index,
                        name: symbol.name().to_vec(),
                        owner_atom: local_symbol_owners.get(&table_index).copied(),
                        section_ordinal,
                        value: symbol.value(),
                    })
                }
                DarwinArm64SymbolKindV1::ExternalUndefined => {
                    used_undefined_symbols.insert(table_index);
                    Ok(VerifiedRelocationTargetV1::ExternalUndefined {
                        table_index,
                        name: symbol.name().to_vec(),
                    })
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectRelocationValidationError {
    DuplicateExternalUndefinedSymbol {
        name: Vec<u8>,
        first_table_index: u32,
        second_table_index: u32,
    },
    UnsupportedContainingSectionOrdinal {
        section: NonZeroU32,
    },
    RelocationSiteOverflow,
    OrphanRelocation {
        section: NonZeroU32,
        offset: u32,
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
    UnusedExternalUndefined {
        table_index: u32,
        name: Vec<u8>,
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
