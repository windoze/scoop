//! Canonical member-aware Link closure section for the strong-only profile.

use scoop_identity::{DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId};
use scoop_wire::{Encoder, WireEncode};

use super::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalGeneratedBridgeObjectUnitSetV1,
    CanonicalScoopLirObjectUnitSetV1, CanonicalUndefinedSymbolRequirementSetV1,
    CodeLinkObjectMemberSetV1, PlannedLinkObjectMemberSetV1, VerifiedCodeFingerprintV1,
    VerifiedCodeFingerprintV2, VerifiedCodeLinkObjectMemberSetV1, VerifiedEntryProductionBranchV1,
};
use crate::SlibMemberId;

mod wire;
pub use wire::{
    DecodedLinkIdentityClosureSectionV1, DigestPatchInputCheckedLinkIdentityClosureSectionV1,
    LinkDigestPatchInputValidationError, LinkIdentityClosureSectionValidationError,
    LinkObjectMaterializationValidationError, LinkObjectProjectionValidationError,
    LinkSymbolProjectionValidationError, MaterializationCheckedLinkIdentityClosureSectionV1,
    ObjectProjectionCheckedLinkIdentityClosureSectionV1,
    SymbolProjectionCheckedLinkIdentityClosureSectionV1,
};

#[cfg(test)]
pub(crate) fn encoded_link_identity_closure_for_test() -> Vec<u8> {
    wire::tests::encoded_link_identity_closure_for_test()
}

#[cfg(test)]
pub(crate) fn encoded_link_identity_closure_for_patch_test(
    plan: &PlannedLinkObjectMemberSetV1,
    builtins: &super::VerifiedBuiltinObjectStrongRelocationSetV1,
    intent: scoop_identity::DigestPatchIntentId,
    member: SlibMemberId,
    checked_offset: u64,
) -> Vec<u8> {
    wire::tests::encoded_link_identity_closure_for_patch_test(
        plan,
        Some(builtins),
        intent,
        member,
        checked_offset,
    )
}

#[cfg(test)]
pub(crate) fn encoded_link_identity_closure_without_object_projection_for_test(
    plan: &PlannedLinkObjectMemberSetV1,
    intent: scoop_identity::DigestPatchIntentId,
    member: SlibMemberId,
    checked_offset: u64,
) -> Vec<u8> {
    wire::tests::encoded_link_identity_closure_for_patch_test(
        plan,
        None,
        intent,
        member,
        checked_offset,
    )
}

#[cfg(test)]
pub(crate) fn encoded_link_identity_closure_without_symbol_projection_for_test(
    plan: &PlannedLinkObjectMemberSetV1,
    builtins: &super::VerifiedBuiltinObjectStrongRelocationSetV1,
    intent: scoop_identity::DigestPatchIntentId,
    member: SlibMemberId,
    checked_offset: u64,
) -> Vec<u8> {
    wire::tests::encoded_link_identity_closure_without_symbol_projection_for_test(
        plan,
        builtins,
        intent,
        member,
        checked_offset,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkObjectMaterializationV1 {
    ScoopLir {
        member: SlibMemberId,
        units: CanonicalScoopLirObjectUnitSetV1,
    },
    GeneratedCBridge {
        member: SlibMemberId,
        units: CanonicalGeneratedBridgeObjectUnitSetV1,
    },
}

impl LinkObjectMaterializationV1 {
    pub const fn member(&self) -> SlibMemberId {
        match self {
            Self::ScoopLir { member, .. } | Self::GeneratedCBridge { member, .. } => *member,
        }
    }

    pub const fn scoop_lir_units(&self) -> Option<&CanonicalScoopLirObjectUnitSetV1> {
        match self {
            Self::ScoopLir { units, .. } => Some(units),
            Self::GeneratedCBridge { .. } => None,
        }
    }

    pub const fn generated_bridge_units(&self) -> Option<&CanonicalGeneratedBridgeObjectUnitSetV1> {
        match self {
            Self::ScoopLir { .. } => None,
            Self::GeneratedCBridge { units, .. } => Some(units),
        }
    }
}

impl WireEncode for LinkObjectMaterializationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::ScoopLir { .. } => 1,
            Self::GeneratedCBridge { .. } => 2,
        })?;
        encoder.field(1)?;
        self.member().encode(encoder)?;
        encoder.field(2)?;
        match self {
            Self::ScoopLir { units, .. } => encode_array(encoder, units.units()),
            Self::GeneratedCBridge { units, .. } => encode_array(encoder, units.units()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedDefinitionAtomRangeProjectionV1 {
    atom: ObjectDefinitionAtomId,
    atom_role: DefinitionAtomRole,
    section_ordinal: u8,
    start: u64,
    end: u64,
    padding_end: u64,
}

impl VerifiedDefinitionAtomRangeProjectionV1 {
    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn atom_role(self) -> DefinitionAtomRole {
        self.atom_role
    }

    pub const fn section_ordinal(self) -> u8 {
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

impl WireEncode for VerifiedDefinitionAtomRangeProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.atom.encode(encoder)?;
        encoder.field(2)?;
        self.atom_role.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(u64::from(self.section_ordinal))?;
        encoder.field(4)?;
        encoder.unsigned(self.start)?;
        encoder.field(5)?;
        encoder.unsigned(self.end)?;
        encoder.field(6)?;
        encoder.unsigned(self.padding_end)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedObjectDefinitionIndexV1 {
    member: SlibMemberId,
    definition: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    primary_symbol_table_index: u32,
    atoms: Vec<VerifiedDefinitionAtomRangeProjectionV1>,
}

impl VerifiedObjectDefinitionIndexV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn definition(&self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn primary_atom(&self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn primary_symbol_table_index(&self) -> u32 {
        self.primary_symbol_table_index
    }

    pub fn atoms(&self) -> &[VerifiedDefinitionAtomRangeProjectionV1] {
        &self.atoms
    }
}

impl WireEncode for VerifiedObjectDefinitionIndexV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(4)?;
        encoder.unsigned(u64::from(self.primary_symbol_table_index))?;
        encoder.field(5)?;
        encode_array(encoder, &self.atoms)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedImageOwnerProjectionV1 {
    member: SlibMemberId,
    definition: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    byte_size: u64,
}

impl VerifiedImageOwnerProjectionV1 {
    pub const fn member(self) -> SlibMemberId {
        self.member
    }
}

impl WireEncode for VerifiedImageOwnerProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(4)?;
        encoder.unsigned(u64::from(self.primary_symbol_table_index))?;
        encoder.field(5)?;
        encoder.unsigned(self.checked_offset)?;
        encoder.field(6)?;
        encoder.unsigned(self.byte_size)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedEntryOwnerProjectionV1 {
    member: SlibMemberId,
    definition: ObjectDefinitionPlanId,
    checked_offset: u64,
}

impl VerifiedEntryOwnerProjectionV1 {
    pub const fn member(self) -> SlibMemberId {
        self.member
    }
}

impl WireEncode for VerifiedEntryOwnerProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(self.checked_offset)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedEntryOwnerBranchV1 {
    Library,
    Executable(VerifiedEntryOwnerProjectionV1),
}

impl WireEncode for VerifiedEntryOwnerBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Executable(entry) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                entry.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkIdentityClosureSectionV1 {
    materializations: Vec<LinkObjectMaterializationV1>,
    definition_indexes: Vec<VerifiedObjectDefinitionIndexV1>,
    patch_sites: Vec<super::VerifiedMaterializedPatchSiteV1>,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    verified_link_objects: CodeLinkObjectMemberSetV1,
    image_owner: VerifiedImageOwnerProjectionV1,
    entry_owner: VerifiedEntryOwnerBranchV1,
}

impl LinkIdentityClosureSectionV1 {
    pub fn materializations(&self) -> &[LinkObjectMaterializationV1] {
        &self.materializations
    }

    pub fn definition_indexes(&self) -> &[VerifiedObjectDefinitionIndexV1] {
        &self.definition_indexes
    }

    pub fn patch_sites(&self) -> &[super::VerifiedMaterializedPatchSiteV1] {
        &self.patch_sites
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn verified_link_objects(&self) -> &CodeLinkObjectMemberSetV1 {
        &self.verified_link_objects
    }

    pub const fn image_owner(&self) -> VerifiedImageOwnerProjectionV1 {
        self.image_owner
    }

    pub const fn entry_owner(&self) -> &VerifiedEntryOwnerBranchV1 {
        &self.entry_owner
    }

    pub fn from_verified_code(
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<Self, LinkIdentityClosureBuildError> {
        Self::from_code_parts(
            code.production().link_objects(),
            code.defined_symbols(),
            code.undefined_symbols(),
        )
    }

    /// Builds the unchanged Link identity closure from a Strong V2 Code proof.
    /// The object and symbol projections therefore come from the same layout
    /// profile proof used by the production manifest.
    pub fn from_verified_layout_code(
        code: &VerifiedCodeFingerprintV2,
    ) -> Result<Self, LinkIdentityClosureBuildError> {
        Self::from_code_parts(
            code.production().link_objects(),
            code.defined_symbols(),
            code.undefined_symbols(),
        )
    }

    fn from_code_parts(
        objects: &VerifiedCodeLinkObjectMemberSetV1,
        defined_symbols: &CanonicalDefinedLinkSymbolOwnerSetV1,
        undefined_symbols: &CanonicalUndefinedSymbolRequirementSetV1,
    ) -> Result<Self, LinkIdentityClosureBuildError> {
        let final_objects = objects.final_objects();
        let builtins = final_objects.entry().patch_sites().builtins();
        let image = final_objects.runtime_images().fingerprint().image();
        let primary = image.primary();
        let entry_owner = match (final_objects.entry().branch(), final_objects.entry().plan()) {
            (
                VerifiedEntryProductionBranchV1::Library,
                scoop_lir::EntryProductionPlanV1::Library,
            ) => VerifiedEntryOwnerBranchV1::Library,
            (
                VerifiedEntryProductionBranchV1::Executable(entry),
                scoop_lir::EntryProductionPlanV1::Executable(plan),
            ) => VerifiedEntryOwnerBranchV1::Executable(VerifiedEntryOwnerProjectionV1 {
                member: entry.member(),
                definition: plan.root_descriptor_definition(),
                checked_offset: entry.checked_offset(),
            }),
            _ => return Err(LinkIdentityClosureBuildError::EntryBranchMismatch),
        };
        Ok(Self {
            materializations: materializations(builtins.member_plan()),
            definition_indexes: definition_indexes(builtins),
            patch_sites: final_objects.entry().patch_sites().sites().to_vec(),
            defined_symbols: defined_symbols.clone(),
            undefined_symbols: undefined_symbols.clone(),
            verified_link_objects: objects.projection().clone(),
            image_owner: VerifiedImageOwnerProjectionV1 {
                member: image.member(),
                definition: image.plan().definition_plan(),
                primary_atom: primary.atom(),
                primary_symbol_table_index: image.primary_symbol_table_index(),
                checked_offset: primary.checked_offset(),
                byte_size: primary.byte_size(),
            },
            entry_owner,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkIdentityClosureBuildError {
    EntryBranchMismatch,
}

impl std::fmt::Display for LinkIdentityClosureBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Link identity closure: {self:?}")
    }
}

impl std::error::Error for LinkIdentityClosureBuildError {}

impl WireEncode for LinkIdentityClosureSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        encode_array(encoder, &self.materializations)?;
        encoder.field(2)?;
        encode_array(encoder, &self.definition_indexes)?;
        encoder.field(3)?;
        encode_array(encoder, &self.patch_sites)?;
        encoder.field(4)?;
        self.defined_symbols.encode(encoder)?;
        encoder.field(5)?;
        self.undefined_symbols.encode(encoder)?;
        encoder.field(6)?;
        self.verified_link_objects.encode(encoder)?;
        encoder.field(7)?;
        self.image_owner.encode(encoder)?;
        encoder.field(8)?;
        self.entry_owner.encode(encoder)
    }
}

fn materializations(plan: &PlannedLinkObjectMemberSetV1) -> Vec<LinkObjectMaterializationV1> {
    let mut result = plan
        .scoop_lir_members()
        .iter()
        .map(|member| LinkObjectMaterializationV1::ScoopLir {
            member: member.member_id(),
            units: member.units().clone(),
        })
        .chain(plan.generated_bridge_members().iter().map(|member| {
            LinkObjectMaterializationV1::GeneratedCBridge {
                member: member.member_id(),
                units: member.units().clone(),
            }
        }))
        .collect::<Vec<_>>();
    result.sort_unstable_by_key(LinkObjectMaterializationV1::member);
    result
}

fn definition_indexes(
    builtins: &super::VerifiedBuiltinObjectStrongRelocationSetV1,
) -> Vec<VerifiedObjectDefinitionIndexV1> {
    let mut result = builtins
        .strong_relocations()
        .members()
        .iter()
        .flat_map(|member| {
            let member_id = member.member();
            member
                .definitions()
                .definitions()
                .iter()
                .map(move |definition| {
                    let mut atoms = definition
                        .atoms()
                        .iter()
                        .map(|atom| VerifiedDefinitionAtomRangeProjectionV1 {
                            atom: atom.atom(),
                            atom_role: atom.atom_role(),
                            section_ordinal: atom.section_ordinal().get(),
                            start: atom.start(),
                            end: atom.end(),
                            padding_end: atom.padding_end(),
                        })
                        .collect::<Vec<_>>();
                    atoms.sort_unstable_by_key(|atom| atom.atom);
                    VerifiedObjectDefinitionIndexV1 {
                        member: member_id,
                        definition: definition.definition(),
                        primary_atom: definition.primary_atom(),
                        primary_symbol_table_index: definition.primary_symbol_table_index(),
                        atoms,
                    }
                })
        })
        .collect::<Vec<_>>();
    result.sort_unstable_by_key(|index| (index.member, index.definition));
    result
}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
