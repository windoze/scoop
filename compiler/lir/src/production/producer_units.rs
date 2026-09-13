//! Complete producer ownership for strong object-definition plans.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    GeneratedBridgeAtomId, GeneratedBridgeUnitId, ObjectDefinitionPlanId,
    ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole, StrongDefinitionEntityKind,
    StrongDefinitionRole,
};

use crate::{GeneratedBridgePlanBuildError, GeneratedBridgePlanSetV1, OdrFreeLirFoundation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedBridgeProducerUnitV1 {
    unit: GeneratedBridgeUnitId,
    definition_plans: Vec<ObjectDefinitionPlanId>,
}

impl GeneratedBridgeProducerUnitV1 {
    pub const fn unit(&self) -> GeneratedBridgeUnitId {
        self.unit
    }

    pub fn definition_plans(&self) -> &[ObjectDefinitionPlanId] {
        &self.definition_plans
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongProducerUnitPartitionV1 {
    scoop_lir_definition_plans: Vec<ObjectDefinitionPlanId>,
    generated_bridge_units: Vec<GeneratedBridgeProducerUnitV1>,
}

impl StrongProducerUnitPartitionV1 {
    pub fn from_odr_free_foundation(
        foundation: &OdrFreeLirFoundation,
    ) -> Result<Self, StrongProducerUnitPartitionError> {
        let bridge_plan = GeneratedBridgePlanSetV1::from_odr_free_foundation(foundation)
            .map_err(StrongProducerUnitPartitionError::GeneratedBridgePlan)?;
        let mut unit_definitions = bridge_plan
            .units()
            .iter()
            .map(|unit| (unit.unit(), Vec::new()))
            .collect::<BTreeMap<_, _>>();
        let mut atom_units = BTreeMap::new();
        for unit in bridge_plan.units() {
            register_atom_unit(&mut atom_units, unit.primary_atom(), unit.unit())?;
            for atom in unit.materialized_associated_atoms() {
                register_atom_unit(&mut atom_units, atom, unit.unit())?;
            }
        }

        let mut scoop_lir_definition_plans = Vec::new();
        for record in foundation.definition_plans() {
            let plan = record.id();
            let key = record.key();
            match (key.owner(), key.definition_role()) {
                (
                    ObjectDefinitionPlanOwner::Strong { entity, .. },
                    ObjectDefinitionPlanRole::Strong(StrongDefinitionRole::GeneratedBridge),
                ) => {
                    let StrongDefinitionEntityKind::GeneratedBridgeAtom(atom) = entity.kind()
                    else {
                        return Err(
                            StrongProducerUnitPartitionError::InvalidGeneratedBridgeOwner(plan),
                        );
                    };
                    let Some(unit) = atom_units.get(&atom) else {
                        return Err(
                            StrongProducerUnitPartitionError::UnplannedGeneratedBridgeAtom {
                                plan,
                                atom,
                            },
                        );
                    };
                    unit_definitions
                        .get_mut(unit)
                        .expect("bridge plan initialized every unit")
                        .push(plan);
                }
                (ObjectDefinitionPlanOwner::Strong { .. }, ObjectDefinitionPlanRole::Strong(_)) => {
                    scoop_lir_definition_plans.push(plan);
                }
                _ => {
                    return Err(StrongProducerUnitPartitionError::NonStrongDefinitionPlan(
                        plan,
                    ));
                }
            }
        }
        scoop_lir_definition_plans.sort_unstable();

        let mut generated_bridge_units = Vec::with_capacity(unit_definitions.len());
        for (unit, mut definition_plans) in unit_definitions {
            if definition_plans.is_empty() {
                return Err(StrongProducerUnitPartitionError::EmptyGeneratedBridgeUnit(
                    unit,
                ));
            }
            definition_plans.sort_unstable();
            generated_bridge_units.push(GeneratedBridgeProducerUnitV1 {
                unit,
                definition_plans,
            });
        }

        Ok(Self {
            scoop_lir_definition_plans,
            generated_bridge_units,
        })
    }

    pub fn scoop_lir_definition_plans(&self) -> &[ObjectDefinitionPlanId] {
        &self.scoop_lir_definition_plans
    }

    pub fn generated_bridge_units(&self) -> &[GeneratedBridgeProducerUnitV1] {
        &self.generated_bridge_units
    }

    pub fn definition_plan_count(&self) -> usize {
        self.scoop_lir_definition_plans.len()
            + self
                .generated_bridge_units
                .iter()
                .map(|unit| unit.definition_plans.len())
                .sum::<usize>()
    }
}

fn register_atom_unit(
    atom_units: &mut BTreeMap<GeneratedBridgeAtomId, GeneratedBridgeUnitId>,
    atom: GeneratedBridgeAtomId,
    unit: GeneratedBridgeUnitId,
) -> Result<(), StrongProducerUnitPartitionError> {
    if let Some(first_unit) = atom_units.insert(atom, unit) {
        return Err(
            StrongProducerUnitPartitionError::DuplicateGeneratedBridgeAtom {
                atom,
                first_unit,
                second_unit: unit,
            },
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongProducerUnitPartitionError {
    GeneratedBridgePlan(GeneratedBridgePlanBuildError),
    DuplicateGeneratedBridgeAtom {
        atom: GeneratedBridgeAtomId,
        first_unit: GeneratedBridgeUnitId,
        second_unit: GeneratedBridgeUnitId,
    },
    InvalidGeneratedBridgeOwner(ObjectDefinitionPlanId),
    UnplannedGeneratedBridgeAtom {
        plan: ObjectDefinitionPlanId,
        atom: GeneratedBridgeAtomId,
    },
    EmptyGeneratedBridgeUnit(GeneratedBridgeUnitId),
    NonStrongDefinitionPlan(ObjectDefinitionPlanId),
}

impl fmt::Display for StrongProducerUnitPartitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong producer-unit partition: {self:?}"
        )
    }
}

impl std::error::Error for StrongProducerUnitPartitionError {}

#[cfg(test)]
mod tests;
