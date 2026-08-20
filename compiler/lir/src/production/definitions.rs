use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    DecodedPersistentId, DefinitionAtomRole, IdentityReferenceError, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, PersistentIdResolver, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::ConeLirFoundation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectDefinitionPlanV1 {
    plan: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    associated_atoms: Vec<ObjectDefinitionAtomId>,
}

impl ObjectDefinitionPlanV1 {
    pub const fn plan(&self) -> ObjectDefinitionPlanId {
        self.plan
    }

    pub const fn primary_atom(&self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub fn associated_atoms(&self) -> &[ObjectDefinitionAtomId] {
        &self.associated_atoms
    }
}

impl WireEncode for ObjectDefinitionPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.plan.encode(encoder)?;
        encoder.field(2)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(3)?;
        encode_array(encoder, &self.associated_atoms)
    }
}

#[derive(Debug)]
struct DecodedObjectDefinitionPlanV1 {
    plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    associated_atoms: Vec<DecodedPersistentId<ObjectDefinitionAtomId>>,
}

impl WireEncode for DecodedObjectDefinitionPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.plan.encode(encoder)?;
        encoder.field(2)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(3)?;
        encode_array(encoder, &self.associated_atoms)
    }
}

impl WireDecode for DecodedObjectDefinitionPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            plan: decoder.field(1, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(2, DecodedPersistentId::decode)?,
            associated_atoms: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectDefinitionPlanSurfaceV1 {
    plans: Vec<ObjectDefinitionPlanV1>,
}

impl ObjectDefinitionPlanSurfaceV1 {
    pub(crate) fn remove_codegen_records(
        &mut self,
        definitions: &BTreeSet<ObjectDefinitionPlanId>,
        atoms: &BTreeSet<ObjectDefinitionAtomId>,
    ) {
        self.plans.retain(|plan| !definitions.contains(&plan.plan));
        for plan in &mut self.plans {
            plan.associated_atoms.retain(|atom| !atoms.contains(atom));
        }
    }

    pub fn from_foundation(
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ObjectDefinitionPlanBuildError> {
        let plan_ids = foundation
            .definition_plans()
            .iter()
            .map(|record| record.id())
            .collect::<BTreeSet<_>>();
        let mut atoms_by_plan = BTreeMap::<ObjectDefinitionPlanId, Vec<_>>::new();
        for atom in foundation.definition_atoms() {
            let plan = atom.key().plan();
            if !plan_ids.contains(&plan) {
                return Err(ObjectDefinitionPlanBuildError::OrphanAtom {
                    atom: atom.id(),
                    plan,
                });
            }
            atoms_by_plan.entry(plan).or_default().push(atom);
        }

        let mut plans = Vec::with_capacity(foundation.definition_plans().len());
        for record in foundation.definition_plans() {
            let plan = record.id();
            let atoms = atoms_by_plan.remove(&plan).unwrap_or_default();
            let mut primary = None;
            let mut associated_atoms = Vec::new();
            for atom in atoms {
                if atom.key().role() == DefinitionAtomRole::Primary {
                    if let Some(first) = primary {
                        return Err(ObjectDefinitionPlanBuildError::MultiplePrimaryAtoms {
                            plan,
                            first,
                            second: atom.id(),
                        });
                    }
                    primary = Some(atom.id());
                } else {
                    associated_atoms.push(atom.id());
                }
            }
            let Some(primary_atom) = primary else {
                return Err(ObjectDefinitionPlanBuildError::MissingPrimaryAtom(plan));
            };
            associated_atoms.sort_unstable();
            plans.push(ObjectDefinitionPlanV1 {
                plan,
                primary_atom,
                associated_atoms,
            });
        }
        plans.sort_unstable_by_key(ObjectDefinitionPlanV1::plan);
        Ok(Self { plans })
    }

    pub fn plans(&self) -> &[ObjectDefinitionPlanV1] {
        &self.plans
    }
}

impl WireEncode for ObjectDefinitionPlanSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
    }
}

#[derive(Debug)]
pub struct DecodedObjectDefinitionPlanSurfaceV1 {
    plans: Vec<DecodedObjectDefinitionPlanV1>,
}

impl DecodedObjectDefinitionPlanSurfaceV1 {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        foundation: &ConeLirFoundation,
    ) -> Result<ObjectDefinitionPlanSurfaceV1, ObjectDefinitionPlanValidationError> {
        let expected = ObjectDefinitionPlanSurfaceV1::from_foundation(foundation)
            .map_err(ObjectDefinitionPlanValidationError::Foundation)?;
        if self.plans.len() != expected.plans.len() {
            return Err(ObjectDefinitionPlanValidationError::PlanCoverage {
                expected: expected.plans.len(),
                actual: self.plans.len(),
            });
        }

        let mut plans: Vec<ObjectDefinitionPlanV1> = Vec::with_capacity(self.plans.len());
        for (index, decoded) in self.plans.into_iter().enumerate() {
            let plan: ObjectDefinitionPlanId = identities
                .resolve(decoded.plan)
                .map_err(ObjectDefinitionPlanValidationError::Identity)?;
            if index > 0 && plans[index - 1].plan >= plan {
                return Err(if plans[index - 1].plan == plan {
                    ObjectDefinitionPlanValidationError::DuplicatePlan(plan)
                } else {
                    ObjectDefinitionPlanValidationError::NonCanonicalPlanOrder { index }
                });
            }
            let primary_atom: ObjectDefinitionAtomId = identities
                .resolve(decoded.primary_atom)
                .map_err(ObjectDefinitionPlanValidationError::Identity)?;
            let mut associated_atoms: Vec<ObjectDefinitionAtomId> =
                Vec::with_capacity(decoded.associated_atoms.len());
            for (atom_index, decoded_atom) in decoded.associated_atoms.into_iter().enumerate() {
                let atom: ObjectDefinitionAtomId = identities
                    .resolve(decoded_atom)
                    .map_err(ObjectDefinitionPlanValidationError::Identity)?;
                if atom_index > 0 && associated_atoms[atom_index - 1] >= atom {
                    return Err(if associated_atoms[atom_index - 1] == atom {
                        ObjectDefinitionPlanValidationError::DuplicateAssociatedAtom { plan, atom }
                    } else {
                        ObjectDefinitionPlanValidationError::NonCanonicalAssociatedAtomOrder {
                            plan,
                            index: atom_index,
                        }
                    });
                }
                associated_atoms.push(atom);
            }
            plans.push(ObjectDefinitionPlanV1 {
                plan,
                primary_atom,
                associated_atoms,
            });
        }
        for (index, (actual, expected)) in plans.iter().zip(&expected.plans).enumerate() {
            if actual != expected {
                return Err(ObjectDefinitionPlanValidationError::PlanMismatch { index });
            }
        }
        Ok(ObjectDefinitionPlanSurfaceV1 { plans })
    }
}

impl WireEncode for DecodedObjectDefinitionPlanSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
    }
}

impl WireDecode for DecodedObjectDefinitionPlanSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedObjectDefinitionPlanV1::decode(decoder))
            .map(|plans| Self { plans })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectDefinitionPlanBuildError {
    OrphanAtom {
        atom: ObjectDefinitionAtomId,
        plan: ObjectDefinitionPlanId,
    },
    MissingPrimaryAtom(ObjectDefinitionPlanId),
    MultiplePrimaryAtoms {
        plan: ObjectDefinitionPlanId,
        first: ObjectDefinitionAtomId,
        second: ObjectDefinitionAtomId,
    },
}

impl fmt::Display for ObjectDefinitionPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong object-definition plan surface: {self:?}"
        )
    }
}

impl std::error::Error for ObjectDefinitionPlanBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectDefinitionPlanValidationError {
    Identity(IdentityReferenceError),
    Foundation(ObjectDefinitionPlanBuildError),
    PlanCoverage {
        expected: usize,
        actual: usize,
    },
    DuplicatePlan(ObjectDefinitionPlanId),
    NonCanonicalPlanOrder {
        index: usize,
    },
    DuplicateAssociatedAtom {
        plan: ObjectDefinitionPlanId,
        atom: ObjectDefinitionAtomId,
    },
    NonCanonicalAssociatedAtomOrder {
        plan: ObjectDefinitionPlanId,
        index: usize,
    },
    PlanMismatch {
        index: usize,
    },
}

impl fmt::Display for ObjectDefinitionPlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong object-definition plan section: {self:?}"
        )
    }
}

impl std::error::Error for ObjectDefinitionPlanValidationError {}

fn encode_array<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
