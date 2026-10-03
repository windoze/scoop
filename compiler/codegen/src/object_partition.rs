//! Deterministic physical partition for strong Scoop LIR object emission.

use std::collections::BTreeMap;
use std::fmt;

use scoop_lir::{
    ObjectDefinitionPlanId, ObjectSymbolSurfaceV1, PersistentCallableBodyId,
    ProducerUnitPartitionError, ProducerUnitPartitionV1, StrongDefinitionEntityKind,
    StrongDefinitionRole,
};

/// The codegen-only selector for one physical Scoop LIR object.
///
/// This is deliberately not an archive member role. Stable member identity is
/// derived later from [`ScoopLirObjectUnitSetV1::definition_plans`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoopLirObjectKindV1 {
    NonCallable,
    CallableBody(PersistentCallableBodyId),
}

/// One non-empty set of strong definition plans emitted into one LLVM object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopLirObjectUnitSetV1 {
    kind: ScoopLirObjectKindV1,
    definition_plans: Vec<ObjectDefinitionPlanId>,
}

impl ScoopLirObjectUnitSetV1 {
    pub const fn kind(&self) -> ScoopLirObjectKindV1 {
        self.kind
    }

    pub fn definition_plans(&self) -> &[ObjectDefinitionPlanId] {
        &self.definition_plans
    }
}

/// Complete, non-overlapping Scoop LIR object partition selected by codegen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopLirObjectPartitionV1 {
    producer_units: ProducerUnitPartitionV1,
    objects: Vec<ScoopLirObjectUnitSetV1>,
}

impl ScoopLirObjectPartitionV1 {
    pub fn from_input(
        input: &scoop_lir::ConeLirOutput,
        surface: &ObjectSymbolSurfaceV1,
    ) -> Result<Self, ScoopLirObjectPartitionError> {
        let producer_units = ProducerUnitPartitionV1::from_foundation(input.foundation())
            .map_err(ScoopLirObjectPartitionError::ProducerUnits)?;
        let mut non_callable = Vec::new();
        let mut callables = BTreeMap::<PersistentCallableBodyId, ObjectDefinitionPlanId>::new();
        for definition in producer_units.scoop_lir_definition_plans() {
            let plan = surface
                .plan(*definition)
                .ok_or(ScoopLirObjectPartitionError::MissingDefinition(*definition))?;
            match plan.definition_role() {
                StrongDefinitionRole::CallableBody => {
                    let StrongDefinitionEntityKind::CallableBody(body) = plan.owner().kind() else {
                        return Err(ScoopLirObjectPartitionError::InvalidCallableDefinition(
                            *definition,
                        ));
                    };
                    if let Some(first) = callables.insert(body, *definition) {
                        return Err(ScoopLirObjectPartitionError::DuplicateCallableBody {
                            body,
                            first,
                            second: *definition,
                        });
                    }
                }
                _ => non_callable.push(*definition),
            }
        }
        if non_callable.is_empty() {
            return Err(ScoopLirObjectPartitionError::EmptyNonCallableObject);
        }

        let mut objects = Vec::with_capacity(1 + callables.len());
        objects.push(ScoopLirObjectUnitSetV1 {
            kind: ScoopLirObjectKindV1::NonCallable,
            definition_plans: non_callable,
        });
        objects.extend(
            callables
                .into_iter()
                .map(|(body, definition)| ScoopLirObjectUnitSetV1 {
                    kind: ScoopLirObjectKindV1::CallableBody(body),
                    definition_plans: vec![definition],
                }),
        );
        Ok(Self {
            producer_units,
            objects,
        })
    }

    pub const fn producer_units(&self) -> &ProducerUnitPartitionV1 {
        &self.producer_units
    }

    pub fn objects(&self) -> &[ScoopLirObjectUnitSetV1] {
        &self.objects
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopLirObjectPartitionError {
    ProducerUnits(ProducerUnitPartitionError),
    MissingDefinition(ObjectDefinitionPlanId),
    InvalidCallableDefinition(ObjectDefinitionPlanId),
    DuplicateCallableBody {
        body: PersistentCallableBodyId,
        first: ObjectDefinitionPlanId,
        second: ObjectDefinitionPlanId,
    },
    EmptyNonCallableObject,
}

impl fmt::Display for ScoopLirObjectPartitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong Scoop LIR object partition: {self:?}"
        )
    }
}

impl std::error::Error for ScoopLirObjectPartitionError {}
