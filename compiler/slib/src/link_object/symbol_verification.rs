//! Exact strong-definition symbol and atom-range verification per object member.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    ObjectDefinitionPlanOwner,
};
use scoop_wire::sha256;

use super::{
    BuiltinObjectSectionRoleV1, ObjectSymbolKindV1, ObservedObjectSymbolV1,
    PlannedMemberStrongObjectSymbolsV1, PlannedStrongObjectSymbolRoleV1,
    ValidatedBuiltinObjectSectionInventoryV1,
};
use crate::SlibMemberId;

mod padding;
use padding::validate_and_assign_zero_padding;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongDefinitionSymbolV1 {
    role: PlannedStrongObjectSymbolRoleV1,
    definition_owner: ObjectDefinitionPlanOwner,
    macho_name: Vec<u8>,
    table_index: u32,
    section_ordinal: NonZeroU32,
    value: u64,
    no_dead_strip: bool,
    private_external: bool,
}

impl VerifiedStrongDefinitionSymbolV1 {
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
pub struct VerifiedDefinitionAtomRangeV1 {
    atom: ObjectDefinitionAtomId,
    atom_role: DefinitionAtomRole,
    section_ordinal: NonZeroU32,
    start: u64,
    end: u64,
    padding_end: u64,
}

impl VerifiedDefinitionAtomRangeV1 {
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

    pub const fn padding_end(self) -> u64 {
        self.padding_end
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
    producer: ConeIdentity,
    member: SlibMemberId,
    sections: ValidatedBuiltinObjectSectionInventoryV1,
    symbols: Vec<VerifiedStrongDefinitionSymbolV1>,
    definitions: Vec<VerifiedStrongObjectDefinitionV1>,
}

impl VerifiedMemberStrongObjectDefinitionIndexV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

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
            .cloned()
    }
}

pub fn verify_member_strong_object_definitions_v1(
    bytes: &[u8],
    sections: ValidatedBuiltinObjectSectionInventoryV1,
    plan: &PlannedMemberStrongObjectSymbolsV1,
) -> Result<VerifiedMemberStrongObjectDefinitionIndexV1, StrongObjectDefinitionValidationError> {
    if u64::try_from(bytes.len()).ok() != Some(sections.envelope().byte_length())
        || sha256(bytes) != sections.envelope().content_digest()
    {
        return Err(StrongObjectDefinitionValidationError::ObjectBytesMismatch);
    }
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
        let expected_weak = planned.request().linkage() == LinkageClass::OdrWeak;
        let actual_weak = observed.kind() == ObjectSymbolKindV1::ExternalWeakDefinition;
        if expected_weak != actual_weak {
            return Err(
                StrongObjectDefinitionValidationError::DefinitionLinkageMismatch {
                    name,
                    expected: planned.request().linkage(),
                    actual: observed.kind(),
                },
            );
        }
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

    let mut definitions = definitions
        .into_iter()
        .map(finalize_definition)
        .collect::<Result<Vec<_>, _>>()?;
    validate_disjoint_atom_ranges(&definitions)?;
    validate_and_assign_zero_padding(bytes, &sections, &mut definitions)?;

    Ok(VerifiedMemberStrongObjectDefinitionIndexV1 {
        producer: plan.producer(),
        member: plan.member(),
        sections,
        symbols,
        definitions,
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
) -> Result<(), StrongObjectDefinitionValidationError> {
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
                return Err(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet);
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
                return Err(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet);
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
                return Err(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet);
            }
        }
    }
    Ok(())
}

fn record_atom_role(
    atom: &mut AtomAccumulator,
    role: DefinitionAtomRole,
) -> Result<(), StrongObjectDefinitionValidationError> {
    if atom.role.is_some_and(|actual| actual != role) {
        return Err(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet);
    }
    atom.role = Some(role);
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
            let atom_role = accumulator
                .role
                .ok_or(StrongObjectDefinitionValidationError::InvalidPlannedSymbolSet)?;
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
                atom_role,
                section_ordinal: start.section_ordinal,
                start: start.value,
                end: end.value,
                padding_end: end.value,
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
    DefinitionLinkageMismatch {
        name: Vec<u8>,
        expected: LinkageClass,
        actual: ObjectSymbolKindV1,
    },
    ObjectBytesMismatch,
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
        start: NonZeroU32,
        end: NonZeroU32,
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
    UnsupportedSectionOrdinal {
        index: u32,
    },
    InvalidSectionByteRange {
        section: NonZeroU32,
    },
    UnownedSection {
        section: NonZeroU32,
        role: BuiltinObjectSectionRoleV1,
        segment_name: Vec<u8>,
        section_name: Vec<u8>,
        symbols: Vec<(Vec<u8>, u64)>,
    },
    UnownedSectionPrefix {
        section: NonZeroU32,
        section_start: u64,
        first_atom_start: u64,
    },
    NonzeroAtomPadding {
        atom: ObjectDefinitionAtomId,
        address: u64,
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
pub(super) mod tests;
