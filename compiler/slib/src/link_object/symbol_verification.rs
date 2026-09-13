//! Exact strong-definition symbol and atom-range verification per object member.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU8;

use scoop_identity::{ObjectDefinitionAtomId, ObjectDefinitionPlanId};

use super::{
    DarwinArm64SymbolKindV1, ObservedMachOSymbolV1, PlannedMemberStrongObjectSymbolsV1,
    PlannedStrongObjectSymbolRoleV1, ValidatedBuiltinObjectSectionInventoryV1,
};
use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongDefinitionSymbolV1 {
    role: PlannedStrongObjectSymbolRoleV1,
    table_index: u32,
    section_ordinal: NonZeroU8,
    value: u64,
    no_dead_strip: bool,
}

impl VerifiedStrongDefinitionSymbolV1 {
    pub const fn role(self) -> PlannedStrongObjectSymbolRoleV1 {
        self.role
    }

    pub const fn table_index(self) -> u32 {
        self.table_index
    }

    pub const fn section_ordinal(self) -> NonZeroU8 {
        self.section_ordinal
    }

    pub const fn value(self) -> u64 {
        self.value
    }

    pub const fn no_dead_strip(self) -> bool {
        self.no_dead_strip
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedDefinitionAtomRangeV1 {
    atom: ObjectDefinitionAtomId,
    section_ordinal: NonZeroU8,
    start: u64,
    end: u64,
}

impl VerifiedDefinitionAtomRangeV1 {
    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn section_ordinal(self) -> NonZeroU8 {
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
pub struct VerifiedStrongObjectDefinitionV1 {
    definition: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    primary_symbol_table_index: u32,
    atoms: Vec<VerifiedDefinitionAtomRangeV1>,
}

impl VerifiedStrongObjectDefinitionV1 {
    pub const fn definition(&self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn primary_atom(&self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn primary_symbol_table_index(&self) -> u32 {
        self.primary_symbol_table_index
    }

    pub fn atoms(&self) -> &[VerifiedDefinitionAtomRangeV1] {
        &self.atoms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedMemberStrongObjectDefinitionIndexV1 {
    member: SlibMemberId,
    sections: ValidatedBuiltinObjectSectionInventoryV1,
    symbols: Vec<VerifiedStrongDefinitionSymbolV1>,
    definitions: Vec<VerifiedStrongObjectDefinitionV1>,
}

impl VerifiedMemberStrongObjectDefinitionIndexV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn sections(&self) -> &ValidatedBuiltinObjectSectionInventoryV1 {
        &self.sections
    }

    pub fn symbols(&self) -> &[VerifiedStrongDefinitionSymbolV1] {
        &self.symbols
    }

    pub fn definitions(&self) -> &[VerifiedStrongObjectDefinitionV1] {
        &self.definitions
    }

    pub fn definition(
        &self,
        definition: ObjectDefinitionPlanId,
    ) -> Option<&VerifiedStrongObjectDefinitionV1> {
        self.definitions
            .binary_search_by_key(&definition, |item| item.definition)
            .ok()
            .map(|index| &self.definitions[index])
    }

    pub fn strong_symbol_by_table_index(
        &self,
        table_index: u32,
    ) -> Option<VerifiedStrongDefinitionSymbolV1> {
        self.symbols
            .iter()
            .find(|symbol| symbol.table_index == table_index)
            .copied()
    }
}

pub fn verify_member_strong_object_definitions_v1(
    sections: ValidatedBuiltinObjectSectionInventoryV1,
    plan: &PlannedMemberStrongObjectSymbolsV1,
) -> Result<VerifiedMemberStrongObjectDefinitionIndexV1, StrongObjectDefinitionValidationError> {
    let expected = plan
        .symbols()
        .iter()
        .map(|symbol| (symbol.macho_name().to_vec(), symbol))
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::<Vec<u8>, &ObservedMachOSymbolV1>::new();
    for symbol in sections
        .envelope()
        .symbols()
        .iter()
        .filter(|symbol| symbol.kind() == DarwinArm64SymbolKindV1::ExternalStrongDefinition)
    {
        if actual.insert(symbol.name().to_vec(), symbol).is_some() {
            return Err(
                StrongObjectDefinitionValidationError::DuplicateExternalStrongDefinition {
                    name: symbol.name().to_vec(),
                },
            );
        }
    }
    if let Some(name) = actual.keys().find(|name| !expected.contains_key(*name)) {
        return Err(
            StrongObjectDefinitionValidationError::UnexpectedExternalStrongDefinition {
                name: name.clone(),
            },
        );
    }
    if let Some(name) = expected.keys().find(|name| !actual.contains_key(*name)) {
        return Err(
            StrongObjectDefinitionValidationError::MissingExternalStrongDefinition {
                name: name.clone(),
            },
        );
    }

    let mut symbols = Vec::with_capacity(expected.len());
    let mut definitions = BTreeMap::<ObjectDefinitionPlanId, DefinitionAccumulator>::new();
    for (name, planned) in expected {
        let observed = actual[&name];
        let section_ordinal = observed.section_ordinal().ok_or(
            StrongObjectDefinitionValidationError::ExternalDefinitionWithoutSection {
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
        symbols.push(VerifiedStrongDefinitionSymbolV1 {
            role,
            table_index: location.table_index,
            section_ordinal: location.section_ordinal,
            value: location.value,
            no_dead_strip: observed.no_dead_strip(),
        });
    }
    symbols.sort_unstable_by_key(|symbol| symbol.role);

    let definitions = definitions
        .into_iter()
        .map(finalize_definition)
        .collect::<Result<Vec<_>, _>>()?;
    validate_disjoint_atom_ranges(&definitions)?;

    Ok(VerifiedMemberStrongObjectDefinitionIndexV1 {
        member: plan.member(),
        sections,
        symbols,
        definitions,
    })
}

#[derive(Clone, Copy)]
struct SymbolLocation {
    table_index: u32,
    section_ordinal: NonZeroU8,
    value: u64,
}

#[derive(Default)]
struct DefinitionAccumulator {
    primary: Option<(ObjectDefinitionAtomId, SymbolLocation)>,
    atoms: BTreeMap<ObjectDefinitionAtomId, AtomAccumulator>,
}

#[derive(Default)]
struct AtomAccumulator {
    start: Option<SymbolLocation>,
    end: Option<SymbolLocation>,
}

fn record_definition_symbol(
    definitions: &mut BTreeMap<ObjectDefinitionPlanId, DefinitionAccumulator>,
    role: PlannedStrongObjectSymbolRoleV1,
    location: SymbolLocation,
) -> Result<(), StrongObjectDefinitionValidationError> {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition,
            primary_atom,
        } => {
            if definitions
                .entry(definition)
                .or_default()
                .primary
                .replace((primary_atom, location))
                .is_some()
            {
                return Err(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet);
            }
        }
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { definition, atom } => {
            if definitions
                .entry(definition)
                .or_default()
                .atoms
                .entry(atom)
                .or_default()
                .start
                .replace(location)
                .is_some()
            {
                return Err(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet);
            }
        }
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { definition, atom } => {
            if definitions
                .entry(definition)
                .or_default()
                .atoms
                .entry(atom)
                .or_default()
                .end
                .replace(location)
                .is_some()
            {
                return Err(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet);
            }
        }
    }
    Ok(())
}

fn finalize_definition(
    (definition, accumulator): (ObjectDefinitionPlanId, DefinitionAccumulator),
) -> Result<VerifiedStrongObjectDefinitionV1, StrongObjectDefinitionValidationError> {
    let (primary_atom, primary) = accumulator
        .primary
        .ok_or(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet)?;
    let atoms = accumulator
        .atoms
        .into_iter()
        .map(|(atom, accumulator)| {
            let start = accumulator
                .start
                .ok_or(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet)?;
            let end = accumulator
                .end
                .ok_or(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet)?;
            if start.section_ordinal != end.section_ordinal {
                return Err(
                    StrongObjectDefinitionValidationError::AtomBoundarySectionMismatch {
                        atom,
                        start: start.section_ordinal,
                        end: end.section_ordinal,
                    },
                );
            }
            if start.value >= end.value {
                return Err(StrongObjectDefinitionValidationError::InvalidAtomRange {
                    atom,
                    start: start.value,
                    end: end.value,
                });
            }
            Ok(VerifiedDefinitionAtomRangeV1 {
                atom,
                section_ordinal: start.section_ordinal,
                start: start.value,
                end: end.value,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let primary_range = atoms
        .iter()
        .find(|range| range.atom == primary_atom)
        .ok_or(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet)?;
    if primary.section_ordinal != primary_range.section_ordinal
        || primary.value != primary_range.start
    {
        return Err(
            StrongObjectDefinitionValidationError::PrimarySymbolLocationMismatch {
                definition,
                primary_atom,
            },
        );
    }
    Ok(VerifiedStrongObjectDefinitionV1 {
        definition,
        primary_atom,
        primary_symbol_table_index: primary.table_index,
        atoms,
    })
}

fn validate_disjoint_atom_ranges(
    definitions: &[VerifiedStrongObjectDefinitionV1],
) -> Result<(), StrongObjectDefinitionValidationError> {
    let mut ranges = definitions
        .iter()
        .flat_map(|definition| definition.atoms.iter().copied())
        .collect::<Vec<_>>();
    ranges
        .sort_unstable_by_key(|range| (range.section_ordinal, range.start, range.end, range.atom));
    for pair in ranges.windows(2) {
        if pair[0].section_ordinal == pair[1].section_ordinal && pair[0].end > pair[1].start {
            return Err(
                StrongObjectDefinitionValidationError::OverlappingAtomRanges {
                    first: pair[0].atom,
                    second: pair[1].atom,
                },
            );
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongObjectDefinitionValidationError {
    DuplicateExternalStrongDefinition {
        name: Vec<u8>,
    },
    UnexpectedExternalStrongDefinition {
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
        start: NonZeroU8,
        end: NonZeroU8,
    },
    InvalidAtomRange {
        atom: ObjectDefinitionAtomId,
        start: u64,
        end: u64,
    },
    PrimarySymbolLocationMismatch {
        definition: ObjectDefinitionPlanId,
        primary_atom: ObjectDefinitionAtomId,
    },
    OverlappingAtomRanges {
        first: ObjectDefinitionAtomId,
        second: ObjectDefinitionAtomId,
    },
}

impl fmt::Display for StrongObjectDefinitionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid member strong object definitions: {self:?}"
        )
    }
}

impl std::error::Error for StrongObjectDefinitionValidationError {}

#[cfg(test)]
mod tests;
