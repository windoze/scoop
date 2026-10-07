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
        Self::from_foundation(input, input.foundation(), surface)
    }

    pub(crate) fn from_foundation(
        input: &scoop_lir::ConeLirOutput,
        foundation: &scoop_lir::ConeLirFoundation,
        surface: &ObjectSymbolSurfaceV1,
    ) -> Result<Self, ScoopLirObjectPartitionError> {
        let producer_units = ProducerUnitPartitionV1::from_foundation(foundation)
            .map_err(ScoopLirObjectPartitionError::ProducerUnits)?;
        let mut non_callable = Vec::new();
        let mut independent = Vec::new();
        let mut callables = BTreeMap::<PersistentCallableBodyId, ObjectDefinitionPlanId>::new();
        let site_owners = input
            .module()
            .callable_bodies()
            .flat_map(|function| function.safepoints.iter())
            .map(|site| (site.site_id(), site.owner()))
            .collect::<BTreeMap<_, _>>();
        let mut body_registrations = BTreeMap::<_, Vec<_>>::new();
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
                StrongDefinitionRole::CallableRegistration => {
                    let StrongDefinitionEntityKind::CallableBody(body) = plan.owner().kind() else {
                        return Err(ScoopLirObjectPartitionError::InvalidCallableDefinition(
                            *definition,
                        ));
                    };
                    body_registrations
                        .entry(body)
                        .or_default()
                        .push(*definition);
                }
                StrongDefinitionRole::SafepointRegistration => {
                    let StrongDefinitionEntityKind::SafepointSite(site) = plan.owner().kind()
                    else {
                        return Err(ScoopLirObjectPartitionError::InvalidCallableDefinition(
                            *definition,
                        ));
                    };
                    let body = site_owners.get(&site).ok_or(
                        ScoopLirObjectPartitionError::InvalidCallableDefinition(*definition),
                    )?;
                    body_registrations
                        .entry(*body)
                        .or_default()
                        .push(*definition);
                }
                _ if plan.definition_role() == StrongDefinitionRole::ImageDescriptor
                    || plan.primary_symbol().linkage() == scoop_lir::LinkageClass::OdrWeak =>
                {
                    independent.push(ScoopLirObjectUnitSetV1 {
                        kind: ScoopLirObjectKindV1::NonCallable,
                        definition_plans: vec![*definition],
                    });
                }
                _ => non_callable.push(*definition),
            }
        }
        let mut objects = Vec::with_capacity(1 + independent.len() + callables.len());
        if !non_callable.is_empty() {
            objects.push(ScoopLirObjectUnitSetV1 {
                kind: ScoopLirObjectKindV1::NonCallable,
                definition_plans: non_callable,
            });
        }
        objects.extend(independent);
        objects.extend(callables.into_iter().map(|(body, definition)| {
            let mut definition_plans = vec![definition];
            definition_plans.extend(body_registrations.remove(&body).into_iter().flatten());
            definition_plans.sort_unstable();
            ScoopLirObjectUnitSetV1 {
                kind: ScoopLirObjectKindV1::CallableBody(body),
                definition_plans,
            }
        }));
        if let Some(definition) = body_registrations
            .values()
            .flat_map(|plans| plans.iter())
            .next()
        {
            return Err(ScoopLirObjectPartitionError::InvalidCallableDefinition(
                *definition,
            ));
        }
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
