//! Shared layout, scan and dispatch ABIs of actual ODR data definitions.

use std::collections::BTreeMap;
use std::fmt;

use crate::{ConeLirFoundation, Module, StrongDefinitionEntity};
use scoop_identity::{
    DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId, ObjectDefinitionPlanOwner,
    OdrGroupId, OdrMemberId, OdrMemberRole,
};
use scoop_wire::{Digest256, HashError, domain_separated_cbor_hash};

mod encode;
mod projection;
mod wire;
pub use wire::DecodedCanonicalShapeAbisV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShapeIdentity {
    group: OdrGroupId,
    member: OdrMemberId,
    role: OdrMemberRole,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    entity: StrongDefinitionEntity,
}

/// A shared shape ABI bound to its existing physical definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalShapeAbiV1 {
    identity: ShapeIdentity,
    abi: Digest256,
}

impl CanonicalShapeAbiV1 {
    pub const fn group(self) -> OdrGroupId {
        self.identity.group
    }
    pub const fn member(self) -> OdrMemberId {
        self.identity.member
    }
    pub const fn role(self) -> OdrMemberRole {
        self.identity.role
    }
    pub const fn definition(self) -> ObjectDefinitionPlanId {
        self.identity.definition
    }
    pub const fn primary_atom(self) -> ObjectDefinitionAtomId {
        self.identity.atom
    }
    pub const fn entity(self) -> StrongDefinitionEntity {
        self.identity.entity
    }
    pub const fn abi(self) -> Digest256 {
        self.abi
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalShapeAbisV1 {
    definitions: Vec<CanonicalShapeAbiV1>,
}

impl CanonicalShapeAbisV1 {
    /// Reuses the storage and String semantics already computed for this module.
    pub fn from_module<'a>(
        module: &'a Module,
        foundation: &ConeLirFoundation,
        immortals: impl IntoIterator<Item = crate::StrongImmortalObjectSemanticPlanV1>,
        storages: impl IntoIterator<Item = &'a crate::StrongStaticStorageSemanticPlanV1>,
    ) -> Result<Self, CanonicalShapeAbiError> {
        let shapes = projection::ShapeContents::new(module, immortals, storages);
        let definitions = identities(foundation)?
            .into_values()
            .map(|identity| {
                let shape = shapes
                    .get(identity.entity, identity.role)
                    .ok_or(CanonicalShapeAbiError::MissingContent(identity.member))?;
                let abi = domain_separated_cbor_hash(
                    "scoop-odr-member-abi-v1",
                    &encode::AbiProjection {
                        module,
                        shape,
                        identity,
                    },
                )
                .map_err(CanonicalShapeAbiError::Hash)?;
                Ok(CanonicalShapeAbiV1 { identity, abi })
            })
            .collect::<Result<_, CanonicalShapeAbiError>>()?;
        Ok(Self { definitions })
    }

    /// Construct the exact table from already computed shared ABIs.
    pub fn new(
        mut records: Vec<(OdrMemberId, Digest256)>,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, CanonicalShapeAbiError> {
        records.sort_unstable_by_key(|record| record.0);
        Self::from_canonical(records, identities(foundation)?)
    }

    fn from_canonical(
        records: Vec<(OdrMemberId, Digest256)>,
        known: BTreeMap<OdrMemberId, ShapeIdentity>,
    ) -> Result<Self, CanonicalShapeAbiError> {
        if records
            .iter()
            .map(|record| record.0)
            .ne(known.keys().copied())
        {
            return Err(CanonicalShapeAbiError::MemberSet);
        }
        Ok(Self {
            definitions: records
                .into_iter()
                .zip(known.into_values())
                .map(|((_, abi), identity)| CanonicalShapeAbiV1 { identity, abi })
                .collect(),
        })
    }

    pub fn definitions(&self) -> &[CanonicalShapeAbiV1] {
        &self.definitions
    }
    pub fn get(&self, member: OdrMemberId) -> Option<&CanonicalShapeAbiV1> {
        self.definitions
            .binary_search_by_key(&member, |record| record.member())
            .ok()
            .map(|index| &self.definitions[index])
    }
}

fn identities(
    foundation: &ConeLirFoundation,
) -> Result<BTreeMap<OdrMemberId, ShapeIdentity>, CanonicalShapeAbiError> {
    let primary_atoms: BTreeMap<_, _> = foundation
        .definition_atoms()
        .iter()
        .filter(|record| record.key().role() == DefinitionAtomRole::Primary)
        .map(|record| (record.key().plan(), record.id()))
        .collect();
    let mut output = BTreeMap::new();
    for plan in foundation.definition_plans() {
        let ObjectDefinitionPlanOwner::Odr { member } = plan.key().owner() else {
            continue;
        };
        let Some(record) = foundation.odr_member(member) else {
            continue;
        };
        let key = record.key();
        if !matches!(
            key.role(),
            OdrMemberRole::Layout
                | OdrMemberRole::ScanProgram
                | OdrMemberRole::TypeDescriptor
                | OdrMemberRole::DispatchTable
                | OdrMemberRole::ImmortalObject
                | OdrMemberRole::StaticStorage
                | OdrMemberRole::InitializationCell
        ) {
            continue;
        }
        let (entity, _) = foundation
            .definition_subject(plan)
            .ok_or(CanonicalShapeAbiError::Definition(member))?;
        let atom = primary_atoms
            .get(&plan.id())
            .copied()
            .ok_or(CanonicalShapeAbiError::Definition(member))?;
        output.insert(
            member,
            ShapeIdentity {
                group: key.group(),
                member,
                role: key.role(),
                definition: plan.id(),
                atom,
                entity,
            },
        );
    }
    Ok(output)
}

#[derive(Debug)]
pub enum CanonicalShapeAbiError {
    Hash(HashError),
    MissingContent(OdrMemberId),
    Definition(OdrMemberId),
    UnknownMember([u8; 32]),
    MemberSet,
}

impl fmt::Display for CanonicalShapeAbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid canonical shape ABIs: {self:?}")
    }
}
impl std::error::Error for CanonicalShapeAbiError {}
