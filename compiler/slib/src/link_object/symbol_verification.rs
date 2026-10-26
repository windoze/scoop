//! Read the definition symbols and atom ranges used by object production.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    ObjectDefinitionPlanOwner,
};

use super::{
    ObjectSymbolKindV1, ObservedObjectSymbolV1, PlannedMemberStrongObjectSymbolsV1,
    PlannedStrongObjectSymbolRoleV1, ValidatedBuiltinObjectSectionInventoryV1,
};
use crate::SlibMemberId;

use super::literal_pools::{LiteralPools, ObjectLiteral};
mod ranges;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectDefinitionSymbolV1 {
    role: PlannedStrongObjectSymbolRoleV1,
    definition_owner: ObjectDefinitionPlanOwner,
    macho_name: Vec<u8>,
    table_index: u32,
    section_ordinal: NonZeroU32,
    value: u64,
    no_dead_strip: bool,
    private_external: bool,
}

impl ObjectDefinitionSymbolV1 {
    pub const fn definition_owner(&self) -> ObjectDefinitionPlanOwner {
        self.definition_owner
    }

    pub const fn role(&self) -> PlannedStrongObjectSymbolRoleV1 {
        self.role
    }

    pub fn macho_name(&self) -> &[u8] {
        &self.macho_name
    }

    pub const fn table_index(&self) -> u32 {
        self.table_index
    }

    pub const fn section_ordinal(&self) -> NonZeroU32 {
        self.section_ordinal
    }

    pub const fn value(&self) -> u64 {
        self.value
    }

    pub const fn no_dead_strip(&self) -> bool {
        self.no_dead_strip
    }

    pub const fn private_external(&self) -> bool {
        self.private_external
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionAtomRangeV1 {
    atom: ObjectDefinitionAtomId,
    atom_role: DefinitionAtomRole,
    section_ordinal: NonZeroU32,
    start: u64,
    end: u64,
}

impl DefinitionAtomRangeV1 {
    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn atom_role(self) -> DefinitionAtomRole {
        self.atom_role
    }

    pub const fn section_ordinal(self) -> NonZeroU32 {
        self.section_ordinal
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectDefinitionV1 {
    definition: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    primary_symbol_table_index: u32,
    atoms: Vec<DefinitionAtomRangeV1>,
}

impl ObjectDefinitionV1 {
    pub const fn definition(&self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn primary_atom(&self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn primary_symbol_table_index(&self) -> u32 {
        self.primary_symbol_table_index
    }

    pub fn atoms(&self) -> &[DefinitionAtomRangeV1] {
        &self.atoms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberObjectDefinitionIndexV1 {
    producer: ConeIdentity,
    member: SlibMemberId,
    sections: ValidatedBuiltinObjectSectionInventoryV1,
    symbols: Vec<ObjectDefinitionSymbolV1>,
    symbol_indices: BTreeMap<u32, usize>,
    definitions: Vec<ObjectDefinitionV1>,
    literal_pools: LiteralPools,
}

impl MemberObjectDefinitionIndexV1 {
    pub(super) fn literal_at(&self, section: NonZeroU32, address: u64) -> Option<ObjectLiteral> {
        self.literal_pools.at(section, address)
    }
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn sections(&self) -> &ValidatedBuiltinObjectSectionInventoryV1 {
        &self.sections
    }

    pub fn symbols(&self) -> &[ObjectDefinitionSymbolV1] {
        &self.symbols
    }

    pub fn definitions(&self) -> &[ObjectDefinitionV1] {
        &self.definitions
    }

    pub fn definition(&self, definition: ObjectDefinitionPlanId) -> Option<&ObjectDefinitionV1> {
        self.definitions
            .binary_search_by_key(&definition, |item| item.definition)
            .ok()
            .map(|index| &self.definitions[index])
    }

    pub fn strong_symbol_by_table_index(
        &self,
        table_index: u32,
    ) -> Option<ObjectDefinitionSymbolV1> {
        self.symbol_indices
            .get(&table_index)
            .map(|&index| self.symbols[index].clone())
    }
}

pub fn read_member_object_definitions_v1(
    bytes: &[u8],
    sections: ValidatedBuiltinObjectSectionInventoryV1,
    plan: &PlannedMemberStrongObjectSymbolsV1,
) -> Result<MemberObjectDefinitionIndexV1, ObjectDefinitionReadError> {
    let expected = plan
        .symbols()
        .iter()
        .map(|symbol| (symbol.macho_name().to_vec(), symbol))
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::<Vec<u8>, &ObservedObjectSymbolV1>::new();
    for symbol in sections.envelope().symbols().iter().filter(|symbol| {
        matches!(
            symbol.kind(),
            ObjectSymbolKindV1::ExternalStrongDefinition
                | ObjectSymbolKindV1::ExternalWeakDefinition
        )
    }) {
        if actual.insert(symbol.name().to_vec(), symbol).is_some() {
            return Err(
                ObjectDefinitionReadError::DuplicateExternalStrongDefinition {
                    name: symbol.name().to_vec(),
                },
            );
        }
    }
    let mut symbols = Vec::with_capacity(expected.len());
    let mut definitions = BTreeMap::<ObjectDefinitionPlanId, DefinitionAccumulator>::new();
    for (name, planned) in expected {
        let observed = actual.get(&name).ok_or_else(|| {
            ObjectDefinitionReadError::MissingExternalStrongDefinition { name: name.clone() }
        })?;
        let section_ordinal = observed.section_ordinal().ok_or(
            ObjectDefinitionReadError::ExternalDefinitionWithoutSection {
                table_index: observed.table_index(),
            },
        )?;
        let location = SymbolLocation {
            table_index: observed.table_index(),
            section_ordinal,
            value: observed.value(),
        };
        let role = planned.role();
        record_definition_symbol(&mut definitions, role, location)?;
        symbols.push(ObjectDefinitionSymbolV1 {
            role,
            definition_owner: planned.definition_owner(),
            macho_name: name,
            table_index: location.table_index,
            section_ordinal: location.section_ordinal,
            value: location.value,
            no_dead_strip: observed.no_dead_strip(),
            private_external: observed.private_external(),
        });
    }
    symbols.sort_unstable_by_key(|symbol| symbol.role);
    let symbol_indices = symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| (symbol.table_index, index))
        .collect();

    let definitions = definitions
        .into_iter()
        .map(finalize_definition)
        .collect::<Result<Vec<_>, _>>()?;
    ranges::validate(&definitions, &symbols, &sections)?;
    let literal_pools = LiteralPools::read(bytes, &sections, &definitions)?;

    Ok(MemberObjectDefinitionIndexV1 {
        producer: plan.producer(),
        member: plan.member(),
        sections,
        symbols,
        symbol_indices,
        definitions,
        literal_pools,
    })
}

#[derive(Clone, Copy)]
struct SymbolLocation {
    table_index: u32,
    section_ordinal: NonZeroU32,
    value: u64,
}

#[derive(Default)]
struct DefinitionAccumulator {
    primary: Option<(ObjectDefinitionAtomId, SymbolLocation)>,
    atoms: BTreeMap<ObjectDefinitionAtomId, AtomAccumulator>,
}

#[derive(Default)]
struct AtomAccumulator {
    role: Option<DefinitionAtomRole>,
    start: Option<SymbolLocation>,
    end: Option<SymbolLocation>,
}

fn record_definition_symbol(
    definitions: &mut BTreeMap<ObjectDefinitionPlanId, DefinitionAccumulator>,
    role: PlannedStrongObjectSymbolRoleV1,
    location: SymbolLocation,
) -> Result<(), ObjectDefinitionReadError> {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition,
            primary_atom,
            ..
        } => {
            if definitions
                .entry(definition)
                .or_default()
                .primary
                .replace((primary_atom, location))
                .is_some()
            {
                return Err(ObjectDefinitionReadError::InvalidPlannedSymbolSet);
            }
        }
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom,
            atom_role,
        } => {
            let atom = definitions
                .entry(definition)
                .or_default()
                .atoms
                .entry(atom)
                .or_default();
            record_atom_role(atom, atom_role)?;
            if atom.start.replace(location).is_some() {
                return Err(ObjectDefinitionReadError::InvalidPlannedSymbolSet);
            }
        }
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom,
            atom_role,
        } => {
            let atom = definitions
                .entry(definition)
                .or_default()
                .atoms
                .entry(atom)
                .or_default();
            record_atom_role(atom, atom_role)?;
            if atom.end.replace(location).is_some() {
                return Err(ObjectDefinitionReadError::InvalidPlannedSymbolSet);
            }
        }
    }
    Ok(())
}

fn record_atom_role(
    atom: &mut AtomAccumulator,
    role: DefinitionAtomRole,
) -> Result<(), ObjectDefinitionReadError> {
    if atom.role.is_some_and(|actual| actual != role) {
        return Err(ObjectDefinitionReadError::InvalidPlannedSymbolSet);
    }
    atom.role = Some(role);
    Ok(())
}

fn finalize_definition(
    (definition, accumulator): (ObjectDefinitionPlanId, DefinitionAccumulator),
) -> Result<ObjectDefinitionV1, ObjectDefinitionReadError> {
    let (primary_atom, primary) = accumulator
        .primary
        .ok_or(ObjectDefinitionReadError::InvalidPlannedSymbolSet)?;
    let atoms = accumulator
        .atoms
        .into_iter()
        .map(|(atom, accumulator)| {
            let atom_role = accumulator
                .role
                .ok_or(ObjectDefinitionReadError::InvalidPlannedSymbolSet)?;
            let start = accumulator
                .start
                .ok_or(ObjectDefinitionReadError::InvalidPlannedSymbolSet)?;
            let end = accumulator
                .end
                .ok_or(ObjectDefinitionReadError::InvalidPlannedSymbolSet)?;
            if start.section_ordinal != end.section_ordinal {
                return Err(ObjectDefinitionReadError::AtomBoundarySectionMismatch {
                    atom,
                    start: start.section_ordinal,
                    end: end.section_ordinal,
                });
            }
            if start.value >= end.value {
                return Err(ObjectDefinitionReadError::InvalidAtomRange {
                    atom,
                    start: start.value,
                    end: end.value,
                });
            }
            Ok(DefinitionAtomRangeV1 {
                atom,
                atom_role,
                section_ordinal: start.section_ordinal,
                start: start.value,
                end: end.value,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ObjectDefinitionV1 {
        definition,
        primary_atom,
        primary_symbol_table_index: primary.table_index,
        atoms,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectDefinitionReadError {
    DuplicateExternalStrongDefinition {
        name: Vec<u8>,
    },
    MissingExternalStrongDefinition {
        name: Vec<u8>,
    },
    ExternalDefinitionWithoutSection {
        table_index: u32,
    },
    InvalidPlannedSymbolSet,
    AtomBoundarySectionMismatch {
        atom: ObjectDefinitionAtomId,
        start: NonZeroU32,
        end: NonZeroU32,
    },
    InvalidAtomRange {
        atom: ObjectDefinitionAtomId,
        start: u64,
        end: u64,
    },
    OverlappingAtomRanges {
        first: ObjectDefinitionAtomId,
        second: ObjectDefinitionAtomId,
    },
    UnsupportedSectionOrdinal {
        index: u32,
    },
    InvalidSectionByteRange {
        section: NonZeroU32,
    },
}

impl fmt::Display for ObjectDefinitionReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid member strong object definitions: {self:?}"
        )
    }
}

impl std::error::Error for ObjectDefinitionReadError {}

#[cfg(test)]
pub(super) mod tests;
