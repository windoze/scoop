//! Canonical content of actual ODR data definitions.

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
pub use wire::DecodedCanonicalShapeLirDefinitionsV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShapeIdentity {
    group: OdrGroupId,
    member: OdrMemberId,
    role: OdrMemberRole,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    entity: StrongDefinitionEntity,
}

/// A complete shape leaf bound to its existing physical definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalShapeLirDefinitionV1 {
    identity: ShapeIdentity,
    fingerprint: Digest256,
    abi: Digest256,
}

impl CanonicalShapeLirDefinitionV1 {
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
    pub const fn fingerprint(self) -> Digest256 {
        self.fingerprint
    }
    pub const fn abi(self) -> Digest256 {
        self.abi
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalShapeLirDefinitionsV1 {
    definitions: Vec<CanonicalShapeLirDefinitionV1>,
}

impl CanonicalShapeLirDefinitionsV1 {
    /// Reuses the storage and String semantics already computed for this module.
    pub fn from_module<'a>(
        module: &'a Module,
        foundation: &ConeLirFoundation,
        immortals: impl IntoIterator<Item = crate::StrongImmortalObjectSemanticPlanV1>,
        storages: impl IntoIterator<Item = &'a crate::StrongStaticStorageSemanticPlanV1>,
    ) -> Result<Self, CanonicalShapeLirError> {
        let shapes = projection::ShapeContents::new(module, immortals, storages);
        let definitions = identities(foundation)?
            .into_values()
            .map(|identity| {
                let shape = shapes
                    .get(identity.entity, identity.role)
                    .ok_or(CanonicalShapeLirError::MissingContent(identity.member))?;
                let fingerprint = domain_separated_cbor_hash(
                    "scoop-lir-definition-v1",
                    &encode::ShapeProjection {
                        module,
                        shape,
                        abi: false,
                    },
                )
                .map_err(CanonicalShapeLirError::Hash)?;
                let abi = domain_separated_cbor_hash(
                    "scoop-odr-member-abi-v1",
                    &encode::AbiProjection {
                        module,
                        shape,
                        identity,
                    },
                )
                .map_err(CanonicalShapeLirError::Hash)?;
                Ok(CanonicalShapeLirDefinitionV1 {
                    identity,
                    fingerprint,
                    abi,
                })
            })
            .collect::<Result<_, CanonicalShapeLirError>>()?;
        Ok(Self { definitions })
    }

    /// Construct the exact table from already computed content leaves.
    pub fn new(
        mut records: Vec<(OdrMemberId, Digest256, Digest256)>,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, CanonicalShapeLirError> {
        records.sort_unstable_by_key(|record| record.0);
        Self::from_canonical(records, identities(foundation)?)
    }

    fn from_canonical(
        records: Vec<(OdrMemberId, Digest256, Digest256)>,
        known: BTreeMap<OdrMemberId, ShapeIdentity>,
    ) -> Result<Self, CanonicalShapeLirError> {
        if records
            .iter()
            .map(|record| record.0)
            .ne(known.keys().copied())
        {
            return Err(CanonicalShapeLirError::MemberSet);
        }
        Ok(Self {
            definitions: records
                .into_iter()
                .zip(known.into_values())
                .map(
                    |((_, fingerprint, abi), identity)| CanonicalShapeLirDefinitionV1 {
                        identity,
                        fingerprint,
                        abi,
                    },
                )
                .collect(),
        })
    }

    pub fn definitions(&self) -> &[CanonicalShapeLirDefinitionV1] {
        &self.definitions
    }
    pub fn get(&self, member: OdrMemberId) -> Option<&CanonicalShapeLirDefinitionV1> {
        self.definitions
            .binary_search_by_key(&member, |record| record.member())
            .ok()
            .map(|index| &self.definitions[index])
    }
}

fn identities(
    foundation: &ConeLirFoundation,
) -> Result<BTreeMap<OdrMemberId, ShapeIdentity>, CanonicalShapeLirError> {
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
            .ok_or(CanonicalShapeLirError::Definition(member))?;
        let atom = primary_atoms
            .get(&plan.id())
            .copied()
            .ok_or(CanonicalShapeLirError::Definition(member))?;
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
pub enum CanonicalShapeLirError {
    Hash(HashError),
    MissingContent(OdrMemberId),
    Definition(OdrMemberId),
    UnknownMember([u8; 32]),
    MemberSet,
}

impl fmt::Display for CanonicalShapeLirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid canonical shape definitions: {self:?}")
    }
}
impl std::error::Error for CanonicalShapeLirError {}
