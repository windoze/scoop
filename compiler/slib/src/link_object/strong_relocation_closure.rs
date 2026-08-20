//! Artifact-wide resolution of current-Cone strong relocation targets.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    ObjectDefinitionPlanOwner, StrongDefinitionEntity, StrongDefinitionRole,
};

use super::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, PlannedStrongObjectSymbolRoleV1,
    VerifiedMemberObjectRelocationIndexV1, VerifiedObjectRelocationFormV1,
    VerifiedObjectRelocationShapeV1, VerifiedRelocationTargetV1, VerifiedRelocationUseV1,
};
use crate::SlibMemberId;
use scoop_lir::LirTargetProfile;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RelocationTargetSlotV1 {
    Single,
    Minuend,
    Subtrahend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongRelocationResolutionV1 {
    ObjectLocalStrong {
        target_member: SlibMemberId,
        definition: ObjectDefinitionPlanId,
        owner: LinkDefinitionOwnerV1,
    },
    CurrentConeUndefinedStrong {
        target_member: SlibMemberId,
        definition: ObjectDefinitionPlanId,
        owner: LinkDefinitionOwnerV1,
    },
    ExternalCandidate {
        object_symbol_table_index: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongRelocationBindingV1 {
    source_member: SlibMemberId,
    containing_atom: ObjectDefinitionAtomId,
    containing_atom_role: DefinitionAtomRole,
    section_role: BuiltinObjectSectionRoleV1,
    offset_within_atom: u64,
    width_bytes: u8,
    relocation_form: VerifiedObjectRelocationFormV1,
    encoded_value: u64,
    target_slot: RelocationTargetSlotV1,
    symbol: Vec<u8>,
    resolution: StrongRelocationResolutionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalUndefinedRelocationUseV1 {
    source_member: SlibMemberId,
    containing_atom: ObjectDefinitionAtomId,
    containing_atom_role: DefinitionAtomRole,
    section_role: BuiltinObjectSectionRoleV1,
    offset_within_atom: u64,
    width_bytes: u8,
    relocation_form: VerifiedObjectRelocationFormV1,
    encoded_value: u64,
    target_slot: RelocationTargetSlotV1,
    symbol: Vec<u8>,
}

impl CanonicalUndefinedRelocationUseV1 {
    pub const fn source_member(&self) -> SlibMemberId {
        self.source_member
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

    pub const fn relocation_form(&self) -> VerifiedObjectRelocationFormV1 {
        self.relocation_form
    }

    pub const fn encoded_value(&self) -> u64 {
        self.encoded_value
    }

    pub const fn target_slot(&self) -> RelocationTargetSlotV1 {
        self.target_slot
    }

    pub fn symbol(&self) -> &[u8] {
        &self.symbol
    }
}

impl From<&StrongRelocationBindingV1> for CanonicalUndefinedRelocationUseV1 {
    fn from(binding: &StrongRelocationBindingV1) -> Self {
        Self {
            source_member: binding.source_member,
            containing_atom: binding.containing_atom,
            containing_atom_role: binding.containing_atom_role,
            section_role: binding.section_role,
            offset_within_atom: binding.offset_within_atom,
            width_bytes: binding.width_bytes,
            relocation_form: binding.relocation_form,
            encoded_value: binding.encoded_value,
            target_slot: binding.target_slot,
            symbol: binding.symbol.clone(),
        }
    }
}

impl StrongRelocationBindingV1 {
    pub const fn source_member(&self) -> SlibMemberId {
        self.source_member
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

    pub const fn relocation_form(&self) -> VerifiedObjectRelocationFormV1 {
        self.relocation_form
    }

    pub const fn encoded_value(&self) -> u64 {
        self.encoded_value
    }

    pub const fn target_slot(&self) -> RelocationTargetSlotV1 {
        self.target_slot
    }

    pub fn symbol(&self) -> &[u8] {
        &self.symbol
    }

    pub const fn resolution(&self) -> StrongRelocationResolutionV1 {
        self.resolution
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCurrentConeStrongRelocationClosureV1 {
    target: LirTargetProfile,
    producer: ConeIdentity,
    members: Vec<VerifiedMemberObjectRelocationIndexV1>,
    bindings: Vec<StrongRelocationBindingV1>,
}

impl VerifiedCurrentConeStrongRelocationClosureV1 {
    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn members(&self) -> &[VerifiedMemberObjectRelocationIndexV1] {
        &self.members
    }

    pub fn bindings(&self) -> &[StrongRelocationBindingV1] {
        &self.bindings
    }

    pub(in crate::link_object) fn bindings_at(
        &self,
        member: SlibMemberId,
        atom: ObjectDefinitionAtomId,
        offset: u64,
    ) -> &[StrongRelocationBindingV1] {
        let key = (member, atom, offset);
        let site = |binding: &StrongRelocationBindingV1| {
            (
                binding.source_member,
                binding.containing_atom,
                binding.offset_within_atom,
            )
        };
        // binding_key orders every target slot of a relocation contiguously.
        let start = self.bindings.partition_point(|binding| site(binding) < key);
        let rest = &self.bindings[start..];
        let count = rest.partition_point(|binding| site(binding) == key);
        &rest[..count]
    }
}

pub fn verify_current_cone_strong_relocation_closure_v1(
    mut members: Vec<VerifiedMemberObjectRelocationIndexV1>,
) -> Result<VerifiedCurrentConeStrongRelocationClosureV1, StrongRelocationClosureValidationError> {
    if members.is_empty() {
        return Err(StrongRelocationClosureValidationError::NoLinkObjectMembers);
    }
    members.sort_unstable_by_key(VerifiedMemberObjectRelocationIndexV1::member);
    if let Some(pair) = members
        .windows(2)
        .find(|pair| pair[0].member() == pair[1].member())
    {
        return Err(StrongRelocationClosureValidationError::DuplicateMember(
            pair[0].member(),
        ));
    }
    let producer = members[0].producer();
    if let Some(member) = members.iter().find(|member| member.producer() != producer) {
        return Err(StrongRelocationClosureValidationError::MixedProducer {
            expected: producer,
            actual: member.producer(),
            member: member.member(),
        });
    }

    let target = LirTargetProfile::from_id(members[0].definitions().sections().envelope().target());
    if let Some(member) = members
        .iter()
        .find(|member| member.definitions().sections().envelope().target() != target.id())
    {
        return Err(StrongRelocationClosureValidationError::MixedTarget {
            expected: target,
            actual: LirTargetProfile::from_id(member.definitions().sections().envelope().target()),
            member: member.member(),
        });
    }

    let (symbols, primaries) = index_strong_definitions(&members)?;
    let mut bindings = Vec::new();
    for member in &members {
        for relocation in member.relocations() {
            collect_relocation_bindings(
                member.member(),
                relocation,
                &symbols,
                &primaries,
                &mut bindings,
            )?;
        }
    }
    bindings.sort_unstable_by_key(binding_key);
    Ok(VerifiedCurrentConeStrongRelocationClosureV1 {
        target,
        producer,
        members,
        bindings,
    })
}

#[derive(Clone, Copy)]
struct IndexedStrongSymbol<'a> {
    member: SlibMemberId,
    role: PlannedStrongObjectSymbolRoleV1,
    definition_owner: ObjectDefinitionPlanOwner,
    name: &'a [u8],
}

type StrongSymbolByName<'a> = BTreeMap<Vec<u8>, IndexedStrongSymbol<'a>>;
type StrongPrimaryByDefinition<'a> =
    BTreeMap<(SlibMemberId, ObjectDefinitionPlanId), IndexedStrongSymbol<'a>>;

fn index_strong_definitions(
    members: &[VerifiedMemberObjectRelocationIndexV1],
) -> Result<
    (StrongSymbolByName<'_>, StrongPrimaryByDefinition<'_>),
    StrongRelocationClosureValidationError,
> {
    let mut symbols = BTreeMap::new();
    let mut primaries = BTreeMap::new();
    for member in members {
        for symbol in member.definitions().symbols() {
            let indexed = IndexedStrongSymbol {
                member: member.member(),
                role: symbol.role(),
                definition_owner: symbol.definition_owner(),
                name: symbol.macho_name(),
            };
            if symbols
                .insert(symbol.macho_name().to_vec(), indexed)
                .is_some()
            {
                return Err(
                    StrongRelocationClosureValidationError::DuplicateStrongSymbol {
                        name: symbol.macho_name().to_vec(),
                    },
                );
            }
            if let PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { definition, .. } =
                symbol.role()
            {
                if primaries
                    .insert((member.member(), definition), indexed)
                    .is_some()
                {
                    return Err(
                        StrongRelocationClosureValidationError::DuplicateDefinitionPrimary {
                            member: member.member(),
                            definition,
                        },
                    );
                }
            }
        }
    }
    Ok((symbols, primaries))
}

fn collect_relocation_bindings(
    source_member: SlibMemberId,
    relocation: &VerifiedRelocationUseV1,
    symbols: &StrongSymbolByName<'_>,
    primaries: &StrongPrimaryByDefinition<'_>,
    bindings: &mut Vec<StrongRelocationBindingV1>,
) -> Result<(), StrongRelocationClosureValidationError> {
    match relocation.shape() {
        VerifiedObjectRelocationShapeV1::ElfRela { target, .. }
        | VerifiedObjectRelocationShapeV1::Unsigned64 { target }
        | VerifiedObjectRelocationShapeV1::Branch26 { target }
        | VerifiedObjectRelocationShapeV1::Page21 { target, .. }
        | VerifiedObjectRelocationShapeV1::PageOffset12 { target, .. }
        | VerifiedObjectRelocationShapeV1::GotLoadPage21 { target }
        | VerifiedObjectRelocationShapeV1::GotLoadPageOffset12 { target }
        | VerifiedObjectRelocationShapeV1::PointerToGot32 { target }
        | VerifiedObjectRelocationShapeV1::TlvpLoadPage21 { target }
        | VerifiedObjectRelocationShapeV1::TlvpLoadPageOffset12 { target } => {
            collect_target_binding(
                source_member,
                relocation,
                RelocationTargetSlotV1::Single,
                target,
                symbols,
                primaries,
                bindings,
            )?;
        }
        VerifiedObjectRelocationShapeV1::Subtractor64 {
            minuend,
            subtrahend,
        } => {
            collect_target_binding(
                source_member,
                relocation,
                RelocationTargetSlotV1::Minuend,
                minuend,
                symbols,
                primaries,
                bindings,
            )?;
            collect_target_binding(
                source_member,
                relocation,
                RelocationTargetSlotV1::Subtrahend,
                subtrahend,
                symbols,
                primaries,
                bindings,
            )?;
        }
    }
    Ok(())
}

fn collect_target_binding(
    source_member: SlibMemberId,
    relocation: &VerifiedRelocationUseV1,
    target_slot: RelocationTargetSlotV1,
    target: &VerifiedRelocationTargetV1,
    symbols: &StrongSymbolByName<'_>,
    primaries: &StrongPrimaryByDefinition<'_>,
    bindings: &mut Vec<StrongRelocationBindingV1>,
) -> Result<(), StrongRelocationClosureValidationError> {
    let (symbol, resolution) = match target {
        VerifiedRelocationTargetV1::StrongDefinition { definition } => {
            let indexed = primaries.get(&(source_member, *definition)).ok_or(
                StrongRelocationClosureValidationError::MissingLocalDefinitionPrimary {
                    member: source_member,
                    definition: *definition,
                },
            )?;
            let PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                owner,
                definition_role,
                ..
            } = indexed.role
            else {
                unreachable!("the primary-definition index contains only primary symbols")
            };
            let owner = LinkDefinitionOwnerV1::from_definition(
                indexed.definition_owner,
                owner,
                definition_role,
            )
            .map_err(|_| {
                StrongRelocationClosureValidationError::InvalidResolvedDefinitionOwner {
                    definition: *definition,
                    owner,
                    definition_role,
                }
            })?;
            (
                indexed.name.to_vec(),
                StrongRelocationResolutionV1::ObjectLocalStrong {
                    target_member: indexed.member,
                    definition: *definition,
                    owner,
                },
            )
        }
        VerifiedRelocationTargetV1::ExternalUndefined { table_index, name } => {
            match symbols.get(name) {
                Some(indexed) => {
                    match indexed.role {
                        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                            definition,
                            owner,
                            definition_role,
                            ..
                        } => {
                            if indexed.member == source_member {
                                return Err(
                                StrongRelocationClosureValidationError::RedundantLocalUndefined {
                                    member: source_member,
                                    definition,
                                    table_index: *table_index,
                                },
                            );
                            }
                            let link_owner =
                            LinkDefinitionOwnerV1::from_definition(indexed.definition_owner, owner, definition_role)
                                .map_err(|_| {
                                StrongRelocationClosureValidationError::InvalidResolvedDefinitionOwner {
                                    definition,
                                    owner,
                                    definition_role,
                                }
                            })?;
                            (
                                name.clone(),
                                StrongRelocationResolutionV1::CurrentConeUndefinedStrong {
                                    target_member: indexed.member,
                                    definition,
                                    owner: link_owner,
                                },
                            )
                        }
                        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { atom, .. } => {
                            return Err(
                                StrongRelocationClosureValidationError::ExternalBoundaryTarget {
                                    atom,
                                    boundary: super::VerifiedBoundaryRoleV1::Start,
                                },
                            );
                        }
                        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { atom, .. } => {
                            return Err(
                                StrongRelocationClosureValidationError::ExternalBoundaryTarget {
                                    atom,
                                    boundary: super::VerifiedBoundaryRoleV1::End,
                                },
                            );
                        }
                    }
                }
                None => (
                    name.clone(),
                    StrongRelocationResolutionV1::ExternalCandidate {
                        object_symbol_table_index: *table_index,
                    },
                ),
            }
        }
        VerifiedRelocationTargetV1::LocalDefinition { .. }
        | VerifiedRelocationTargetV1::SectionBase { .. } => return Ok(()),
    };
    bindings.push(StrongRelocationBindingV1 {
        source_member,
        containing_atom: relocation.containing_atom(),
        containing_atom_role: relocation.containing_atom_role(),
        section_role: relocation.section_role(),
        offset_within_atom: relocation.offset_within_atom(),
        width_bytes: relocation.width_bytes(),
        relocation_form: relocation.shape().form(),
        encoded_value: relocation.encoded_value(),
        target_slot,
        symbol,
        resolution,
    });
    Ok(())
}

fn binding_key(
    binding: &StrongRelocationBindingV1,
) -> (
    SlibMemberId,
    ObjectDefinitionAtomId,
    u64,
    RelocationTargetSlotV1,
) {
    (
        binding.source_member,
        binding.containing_atom,
        binding.offset_within_atom,
        binding.target_slot,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongRelocationClosureValidationError {
    NoLinkObjectMembers,
    MixedProducer {
        expected: ConeIdentity,
        actual: ConeIdentity,
        member: SlibMemberId,
    },
    MixedTarget {
        expected: LirTargetProfile,
        actual: LirTargetProfile,
        member: SlibMemberId,
    },
    DuplicateMember(SlibMemberId),
    DuplicateStrongSymbol {
        name: Vec<u8>,
    },
    DuplicateDefinitionPrimary {
        member: SlibMemberId,
        definition: ObjectDefinitionPlanId,
    },
    MissingLocalDefinitionPrimary {
        member: SlibMemberId,
        definition: ObjectDefinitionPlanId,
    },
    RedundantLocalUndefined {
        member: SlibMemberId,
        definition: ObjectDefinitionPlanId,
        table_index: u32,
    },
    ExternalBoundaryTarget {
        atom: ObjectDefinitionAtomId,
        boundary: super::VerifiedBoundaryRoleV1,
    },
    InvalidResolvedDefinitionOwner {
        definition: ObjectDefinitionPlanId,
        owner: StrongDefinitionEntity,
        definition_role: StrongDefinitionRole,
    },
}

impl fmt::Display for StrongRelocationClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid current-Cone strong relocation closure: {self:?}"
        )
    }
}

impl std::error::Error for StrongRelocationClosureValidationError {}

#[cfg(test)]
pub(super) mod tests;
