use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, GeneratedBridgeAtomId, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitId, IdentityReferenceError, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    ObjectDefinitionPlanOwner, PersistentId, PersistentIdResolver, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{
    OdrFreeLirFoundation, StrongObjectDefinitionPlanBuildError, StrongObjectDefinitionPlanSurfaceV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedBridgeUnitPlanV1 {
    unit: GeneratedBridgeUnitId,
    primary_atom: GeneratedBridgeAtomId,
    materialized_associated_atoms: Vec<GeneratedBridgeAtomId>,
    static_assert_atoms: Vec<GeneratedBridgeAtomId>,
}

impl GeneratedBridgeUnitPlanV1 {
    pub const fn unit(&self) -> GeneratedBridgeUnitId {
        self.unit
    }

    pub const fn primary_atom(&self) -> GeneratedBridgeAtomId {
        self.primary_atom
    }

    pub fn materialized_associated_atoms(&self) -> &[GeneratedBridgeAtomId] {
        &self.materialized_associated_atoms
    }

    pub fn static_assert_atoms(&self) -> &[GeneratedBridgeAtomId] {
        &self.static_assert_atoms
    }
}

impl WireEncode for GeneratedBridgeUnitPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.unit.encode(encoder)?;
        encoder.field(2)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(3)?;
        encode_array(encoder, &self.materialized_associated_atoms)?;
        encoder.field(4)?;
        encode_array(encoder, &self.static_assert_atoms)
    }
}

#[derive(Debug)]
struct DecodedGeneratedBridgeUnitPlanV1 {
    unit: DecodedPersistentId<GeneratedBridgeUnitId>,
    primary_atom: DecodedPersistentId<GeneratedBridgeAtomId>,
    materialized_associated_atoms: Vec<DecodedPersistentId<GeneratedBridgeAtomId>>,
    static_assert_atoms: Vec<DecodedPersistentId<GeneratedBridgeAtomId>>,
}

impl WireEncode for DecodedGeneratedBridgeUnitPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.unit.encode(encoder)?;
        encoder.field(2)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(3)?;
        encode_array(encoder, &self.materialized_associated_atoms)?;
        encoder.field(4)?;
        encode_array(encoder, &self.static_assert_atoms)
    }
}

impl WireDecode for DecodedGeneratedBridgeUnitPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            unit: decoder.field(1, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(2, DecodedPersistentId::decode)?,
            materialized_associated_atoms: decoder.field(3, decode_id_array)?,
            static_assert_atoms: decoder.field(4, decode_id_array)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedBridgePlanSetV1 {
    producer: ConeIdentity,
    units: Vec<GeneratedBridgeUnitPlanV1>,
}

impl GeneratedBridgePlanSetV1 {
    pub fn from_odr_free_foundation(
        foundation: &OdrFreeLirFoundation,
    ) -> Result<Self, GeneratedBridgePlanBuildError> {
        let definition_plans =
            StrongObjectDefinitionPlanSurfaceV1::from_odr_free_foundation(foundation)
                .map_err(GeneratedBridgePlanBuildError::DefinitionSurface)?;
        let bridge_units = foundation
            .bridge_units()
            .iter()
            .map(|record| record.id())
            .collect::<BTreeSet<_>>();
        let bridge_atoms = foundation
            .bridge_atoms()
            .iter()
            .map(|record| (record.id(), record.key()))
            .collect::<BTreeMap<_, _>>();
        let mut atoms_by_unit = BTreeMap::<GeneratedBridgeUnitId, Vec<_>>::new();
        for record in foundation.bridge_atoms() {
            let unit = record.key().atom().unit();
            if !bridge_units.contains(&unit) {
                return Err(GeneratedBridgePlanBuildError::UnknownAtomUnit {
                    atom: record.id(),
                    unit,
                });
            }
            atoms_by_unit.entry(unit).or_default().push(record);
        }

        let mut materializable_atoms = BTreeSet::new();
        let mut units = Vec::with_capacity(foundation.bridge_units().len());
        for unit_record in foundation.bridge_units() {
            let unit = unit_record.id();
            let atoms = atoms_by_unit.remove(&unit).unwrap_or_default();
            let mut primary_atom = None;
            let mut materialized_associated_atoms = Vec::new();
            let mut static_assert_atoms = Vec::new();
            for atom in atoms {
                match atom.key().atom() {
                    GeneratedBridgeAtomRoleKey::PrimaryEntry { .. } => {
                        primary_atom = Some(atom.id());
                        materializable_atoms.insert(atom.id());
                    }
                    GeneratedBridgeAtomRoleKey::SignatureDescriptor { .. }
                    | GeneratedBridgeAtomRoleKey::ContextDescriptor { .. } => {
                        materialized_associated_atoms.push(atom.id());
                        materializable_atoms.insert(atom.id());
                    }
                    GeneratedBridgeAtomRoleKey::StaticAssertSupport { .. } => {
                        static_assert_atoms.push(atom.id());
                    }
                }
            }
            let Some(primary_atom) = primary_atom else {
                return Err(GeneratedBridgePlanBuildError::MissingPrimaryAtom(unit));
            };
            materialized_associated_atoms.sort_unstable();
            static_assert_atoms.sort_unstable();
            units.push(GeneratedBridgeUnitPlanV1 {
                unit,
                primary_atom,
                materialized_associated_atoms,
                static_assert_atoms,
            });
        }
        units.sort_unstable_by_key(GeneratedBridgeUnitPlanV1::unit);

        let planned_definitions = foundation
            .definition_plans()
            .iter()
            .filter_map(|record| match record.key().owner() {
                ObjectDefinitionPlanOwner::Strong {
                    entity,
                    producer: _,
                } => match entity.kind() {
                    StrongDefinitionEntityKind::GeneratedBridgeAtom(atom) => {
                        Some((atom, record.id()))
                    }
                    _ => None,
                },
                ObjectDefinitionPlanOwner::Odr { .. } => None,
            })
            .collect::<BTreeMap<_, _>>();
        for atom in &materializable_atoms {
            let key = bridge_atoms[atom];
            let entity = StrongDefinitionEntity::generated_bridge_atom(key)
                .map_err(|_| GeneratedBridgePlanBuildError::NonMaterializableDefinition(*atom))?;
            let expected_plan = ObjectDefinitionPlanId::from_key(
                &ObjectDefinitionPlanKey::strong(
                    foundation.producer(),
                    entity,
                    StrongDefinitionRole::GeneratedBridge,
                )
                .map_err(|_| GeneratedBridgePlanBuildError::InvalidDefinitionPlan(*atom))?,
            )
            .map_err(|_| GeneratedBridgePlanBuildError::InvalidDefinitionPlan(*atom))?;
            if planned_definitions.get(atom) != Some(&expected_plan)
                || definition_plans
                    .plans()
                    .binary_search_by_key(&expected_plan, |plan| plan.plan())
                    .is_err()
            {
                return Err(GeneratedBridgePlanBuildError::MissingDefinitionPlan {
                    atom: *atom,
                    expected: expected_plan,
                });
            }
        }
        if let Some((&atom, &plan)) = planned_definitions
            .iter()
            .find(|(atom, _)| !materializable_atoms.contains(atom))
        {
            return Err(GeneratedBridgePlanBuildError::ExtraneousDefinitionPlan { atom, plan });
        }
        Ok(Self {
            producer: foundation.producer(),
            units,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn units(&self) -> &[GeneratedBridgeUnitPlanV1] {
        &self.units
    }
}

impl WireEncode for GeneratedBridgePlanSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.units)
    }
}

#[derive(Debug)]
pub struct DecodedGeneratedBridgePlanSetV1 {
    units: Vec<DecodedGeneratedBridgeUnitPlanV1>,
}

impl DecodedGeneratedBridgePlanSetV1 {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        foundation: &OdrFreeLirFoundation,
    ) -> Result<GeneratedBridgePlanSetV1, GeneratedBridgePlanValidationError> {
        let expected = GeneratedBridgePlanSetV1::from_odr_free_foundation(foundation)
            .map_err(GeneratedBridgePlanValidationError::Foundation)?;
        if self.units.len() != expected.units.len() {
            return Err(GeneratedBridgePlanValidationError::UnitCoverage {
                expected: expected.units.len(),
                actual: self.units.len(),
            });
        }
        let mut units: Vec<GeneratedBridgeUnitPlanV1> = Vec::with_capacity(self.units.len());
        for (index, decoded) in self.units.into_iter().enumerate() {
            let unit: GeneratedBridgeUnitId = identities
                .resolve(decoded.unit)
                .map_err(GeneratedBridgePlanValidationError::Identity)?;
            if index > 0 && units[index - 1].unit >= unit {
                return Err(if units[index - 1].unit == unit {
                    GeneratedBridgePlanValidationError::DuplicateUnit(unit)
                } else {
                    GeneratedBridgePlanValidationError::NonCanonicalUnitOrder { index }
                });
            }
            let primary_atom = identities
                .resolve(decoded.primary_atom)
                .map_err(GeneratedBridgePlanValidationError::Identity)?;
            let materialized_associated_atoms = resolve_atom_array(
                decoded.materialized_associated_atoms,
                unit,
                GeneratedBridgeAtomSet::MaterializedAssociated,
                identities,
            )?;
            let static_assert_atoms = resolve_atom_array(
                decoded.static_assert_atoms,
                unit,
                GeneratedBridgeAtomSet::StaticAssert,
                identities,
            )?;
            units.push(GeneratedBridgeUnitPlanV1 {
                unit,
                primary_atom,
                materialized_associated_atoms,
                static_assert_atoms,
            });
        }
        for (index, (actual, expected)) in units.iter().zip(&expected.units).enumerate() {
            if actual != expected {
                return Err(GeneratedBridgePlanValidationError::UnitMismatch { index });
            }
        }
        Ok(GeneratedBridgePlanSetV1 {
            producer: foundation.producer(),
            units,
        })
    }
}

impl WireEncode for DecodedGeneratedBridgePlanSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.units)
    }
}

impl WireDecode for DecodedGeneratedBridgePlanSetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedGeneratedBridgeUnitPlanV1::decode(decoder))
            .map(|units| Self { units })
    }
}

fn resolve_atom_array(
    decoded: Vec<DecodedPersistentId<GeneratedBridgeAtomId>>,
    unit: GeneratedBridgeUnitId,
    set: GeneratedBridgeAtomSet,
    identities: &mut ValidatedIdentityGraph,
) -> Result<Vec<GeneratedBridgeAtomId>, GeneratedBridgePlanValidationError> {
    let mut atoms: Vec<GeneratedBridgeAtomId> = Vec::with_capacity(decoded.len());
    for (index, decoded_atom) in decoded.into_iter().enumerate() {
        let atom = identities
            .resolve(decoded_atom)
            .map_err(GeneratedBridgePlanValidationError::Identity)?;
        if index > 0 && atoms[index - 1] >= atom {
            return Err(if atoms[index - 1] == atom {
                GeneratedBridgePlanValidationError::DuplicateAtom { unit, set, atom }
            } else {
                GeneratedBridgePlanValidationError::NonCanonicalAtomOrder { unit, set, index }
            });
        }
        atoms.push(atom);
    }
    Ok(atoms)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedBridgeAtomSet {
    MaterializedAssociated,
    StaticAssert,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedBridgePlanBuildError {
    DefinitionSurface(StrongObjectDefinitionPlanBuildError),
    UnknownAtomUnit {
        atom: GeneratedBridgeAtomId,
        unit: GeneratedBridgeUnitId,
    },
    MissingPrimaryAtom(GeneratedBridgeUnitId),
    NonMaterializableDefinition(GeneratedBridgeAtomId),
    InvalidDefinitionPlan(GeneratedBridgeAtomId),
    MissingDefinitionPlan {
        atom: GeneratedBridgeAtomId,
        expected: ObjectDefinitionPlanId,
    },
    ExtraneousDefinitionPlan {
        atom: GeneratedBridgeAtomId,
        plan: ObjectDefinitionPlanId,
    },
}

impl fmt::Display for GeneratedBridgePlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid generated bridge plan: {self:?}")
    }
}

impl std::error::Error for GeneratedBridgePlanBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedBridgePlanValidationError {
    Identity(IdentityReferenceError),
    Foundation(GeneratedBridgePlanBuildError),
    UnitCoverage {
        expected: usize,
        actual: usize,
    },
    DuplicateUnit(GeneratedBridgeUnitId),
    NonCanonicalUnitOrder {
        index: usize,
    },
    DuplicateAtom {
        unit: GeneratedBridgeUnitId,
        set: GeneratedBridgeAtomSet,
        atom: GeneratedBridgeAtomId,
    },
    NonCanonicalAtomOrder {
        unit: GeneratedBridgeUnitId,
        set: GeneratedBridgeAtomSet,
        index: usize,
    },
    UnitMismatch {
        index: usize,
    },
}

impl fmt::Display for GeneratedBridgePlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid generated bridge plan section: {self:?}")
    }
}

impl std::error::Error for GeneratedBridgePlanValidationError {}

fn decode_id_array<I: PersistentId>(
    decoder: &mut Decoder<'_, '_>,
) -> Result<Vec<DecodedPersistentId<I>>, WireError> {
    decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
}

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
