use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    DecodedPersistentId, DefinitionAtomRole, IdentityReferenceError, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, PersistentIdResolver, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::OdrFreeLirFoundation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongObjectDefinitionPlanV1 {
    plan: ObjectDefinitionPlanId,
    primary_atom: ObjectDefinitionAtomId,
    associated_atoms: Vec<ObjectDefinitionAtomId>,
}

impl StrongObjectDefinitionPlanV1 {
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

impl WireEncode for StrongObjectDefinitionPlanV1 {
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
struct DecodedStrongObjectDefinitionPlanV1 {
    plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    associated_atoms: Vec<DecodedPersistentId<ObjectDefinitionAtomId>>,
}

impl WireEncode for DecodedStrongObjectDefinitionPlanV1 {
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

impl WireDecode for DecodedStrongObjectDefinitionPlanV1 {
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
pub struct StrongObjectDefinitionPlanSurfaceV1 {
    plans: Vec<StrongObjectDefinitionPlanV1>,
}

impl StrongObjectDefinitionPlanSurfaceV1 {
    pub fn from_odr_free_foundation(
        foundation: &OdrFreeLirFoundation,
    ) -> Result<Self, StrongObjectDefinitionPlanBuildError> {
        let plan_ids = foundation
            .definition_plans()
            .iter()
            .map(|record| record.id())
            .collect::<BTreeSet<_>>();
        let mut atoms_by_plan = BTreeMap::<ObjectDefinitionPlanId, Vec<_>>::new();
        for atom in foundation.definition_atoms() {
            let plan = atom.key().plan();
            if !plan_ids.contains(&plan) {
                return Err(StrongObjectDefinitionPlanBuildError::OrphanAtom {
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
                        return Err(StrongObjectDefinitionPlanBuildError::MultiplePrimaryAtoms {
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
                return Err(StrongObjectDefinitionPlanBuildError::MissingPrimaryAtom(
                    plan,
                ));
            };
            associated_atoms.sort_unstable();
            plans.push(StrongObjectDefinitionPlanV1 {
                plan,
                primary_atom,
                associated_atoms,
            });
        }
        plans.sort_unstable_by_key(StrongObjectDefinitionPlanV1::plan);
        Ok(Self { plans })
    }

    pub fn plans(&self) -> &[StrongObjectDefinitionPlanV1] {
        &self.plans
    }
}

impl WireEncode for StrongObjectDefinitionPlanSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
    }
}

#[derive(Debug)]
pub struct DecodedStrongObjectDefinitionPlanSurfaceV1 {
    plans: Vec<DecodedStrongObjectDefinitionPlanV1>,
}

impl DecodedStrongObjectDefinitionPlanSurfaceV1 {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        foundation: &OdrFreeLirFoundation,
    ) -> Result<StrongObjectDefinitionPlanSurfaceV1, StrongObjectDefinitionPlanValidationError>
    {
        let expected = StrongObjectDefinitionPlanSurfaceV1::from_odr_free_foundation(foundation)
            .map_err(StrongObjectDefinitionPlanValidationError::Foundation)?;
        if self.plans.len() != expected.plans.len() {
            return Err(StrongObjectDefinitionPlanValidationError::PlanCoverage {
                expected: expected.plans.len(),
                actual: self.plans.len(),
            });
        }

        let mut plans: Vec<StrongObjectDefinitionPlanV1> = Vec::with_capacity(self.plans.len());
        for (index, decoded) in self.plans.into_iter().enumerate() {
            let plan: ObjectDefinitionPlanId = identities
                .resolve(decoded.plan)
                .map_err(StrongObjectDefinitionPlanValidationError::Identity)?;
            if index > 0 && plans[index - 1].plan >= plan {
                return Err(if plans[index - 1].plan == plan {
                    StrongObjectDefinitionPlanValidationError::DuplicatePlan(plan)
                } else {
                    StrongObjectDefinitionPlanValidationError::NonCanonicalPlanOrder { index }
                });
            }
            let primary_atom: ObjectDefinitionAtomId = identities
                .resolve(decoded.primary_atom)
                .map_err(StrongObjectDefinitionPlanValidationError::Identity)?;
            let mut associated_atoms: Vec<ObjectDefinitionAtomId> =
                Vec::with_capacity(decoded.associated_atoms.len());
            for (atom_index, decoded_atom) in decoded.associated_atoms.into_iter().enumerate() {
                let atom: ObjectDefinitionAtomId = identities
                    .resolve(decoded_atom)
                    .map_err(StrongObjectDefinitionPlanValidationError::Identity)?;
                if atom_index > 0 && associated_atoms[atom_index - 1] >= atom {
                    return Err(if associated_atoms[atom_index - 1] == atom {
                        StrongObjectDefinitionPlanValidationError::DuplicateAssociatedAtom {
                            plan,
                            atom,
                        }
                    } else {
                        StrongObjectDefinitionPlanValidationError::NonCanonicalAssociatedAtomOrder {
                            plan,
                            index: atom_index,
                        }
                    });
                }
                associated_atoms.push(atom);
            }
            plans.push(StrongObjectDefinitionPlanV1 {
                plan,
                primary_atom,
                associated_atoms,
            });
        }
        for (index, (actual, expected)) in plans.iter().zip(&expected.plans).enumerate() {
            if actual != expected {
                return Err(StrongObjectDefinitionPlanValidationError::PlanMismatch { index });
            }
        }
        Ok(StrongObjectDefinitionPlanSurfaceV1 { plans })
    }
}

impl WireEncode for DecodedStrongObjectDefinitionPlanSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
    }
}

impl WireDecode for DecodedStrongObjectDefinitionPlanSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedStrongObjectDefinitionPlanV1::decode(decoder))
            .map(|plans| Self { plans })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongObjectDefinitionPlanBuildError {
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

impl fmt::Display for StrongObjectDefinitionPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong object-definition plan surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongObjectDefinitionPlanBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongObjectDefinitionPlanValidationError {
    Identity(IdentityReferenceError),
    Foundation(StrongObjectDefinitionPlanBuildError),
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

impl fmt::Display for StrongObjectDefinitionPlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong object-definition plan section: {self:?}"
        )
    }
}

impl std::error::Error for StrongObjectDefinitionPlanValidationError {}

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
