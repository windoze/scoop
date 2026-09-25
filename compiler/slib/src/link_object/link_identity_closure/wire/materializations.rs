//! Rebuild object membership from the same artifact's typed producer units.

use super::resources::{allocate, table};
use super::*;
use scoop_wire::{WirePath, encode_canonical_temporary_with_meter};

type Error = LinkObjectMaterializationValidationError;

impl DecodedLinkIdentityClosureSectionV1 {
    /// Borrows the original wire so layout replay retains every later field.
    /// This proves member assignment only, not object contents or Code identity.
    pub fn replay_materializations(
        &self,
        partition: &StrongProducerUnitPartitionV1,
        meter: &mut BudgetMeter,
    ) -> Result<PlannedLinkObjectMemberSetV1, Error> {
        let path = WirePath::root().field(1);
        let definitions = partition.scoop_lir_definition_plans();
        let bridges = partition.generated_bridge_units();
        table::<([u8; 32], ObjectDefinitionPlanId)>(definitions.len(), meter, &path)?;
        table::<([u8; 32], GeneratedBridgeUnitId)>(bridges.len(), meter, &path)?;
        let scoop_ids = definitions.iter().map(|id| (*id.as_array(), *id)).collect();
        let bridge_ids = bridges
            .iter()
            .map(|unit| (*unit.unit().as_array(), unit.unit()))
            .collect();
        let count = self.materializations.len();
        let mut scoop_sets = allocate(count, meter, &path)?;
        let mut bridge_sets = allocate(count, meter, &path)?;
        for (index, materialization) in self.materializations.iter().enumerate() {
            let path = path.clone().index(index as u64);
            meter.charge_nodes(1, &path)?;
            match materialization {
                DecodedLinkObjectMaterializationV1::ScoopLir { units, .. } => {
                    scoop_sets.push(
                        CanonicalScoopLirObjectUnitSetV1::new(resolve(
                            units,
                            &scoop_ids,
                            meter,
                            &path,
                            Error::UnknownScoopLirDefinition,
                        )?)
                        .map_err(Error::ScoopUnitSet)?,
                    );
                }
                DecodedLinkObjectMaterializationV1::GeneratedCBridge { units, .. } => {
                    bridge_sets.push(
                        CanonicalGeneratedBridgeObjectUnitSetV1::new(resolve(
                            units,
                            &bridge_ids,
                            meter,
                            &path,
                            Error::UnknownGeneratedBridgeUnit,
                        )?)
                        .map_err(Error::BridgeUnitSet)?,
                    );
                }
            }
        }
        // The shared constructor allocates sorted member and assignment tables,
        // checks exact coverage and hashes every complete unit grouping.
        table::<crate::PlannedScoopLirObjectMemberV1>(count, meter, &path)?;
        table::<crate::PlannedGeneratedBridgeObjectMemberV1>(count, meter, &path)?;
        table::<(ObjectDefinitionPlanId, SlibMemberId)>(
            partition.definition_plan_count().saturating_mul(3),
            meter,
            &path,
        )?;
        table::<(GeneratedBridgeUnitId, SlibMemberId)>(
            bridges.len().saturating_mul(3),
            meter,
            &path,
        )?;
        let actual = encode_canonical_temporary_with_meter(
            &WireArray(&self.materializations),
            meter,
            &path,
        )?;
        meter.charge_owned_bytes(actual.len() as u64, &path)?;
        meter.charge_work((actual.len() as u64).saturating_mul(4), &path)?;
        let plan = PlannedLinkObjectMemberSetV1::new(partition, scoop_sets, bridge_sets)
            .map_err(Error::MemberPlan)?;
        let expected = super::super::materializations(&plan);
        let expected = encode_canonical_temporary_with_meter(&WireArray(&expected), meter, &path)?;
        meter.charge_work(actual.len().min(expected.len()) as u64, &path)?;
        if actual != expected {
            return Err(Error::ProjectionMismatch);
        }
        Ok(plan)
    }
}

fn resolve<I: PersistentId + Copy>(
    units: &[DecodedPersistentId<I>],
    known: &BTreeMap<[u8; 32], I>,
    meter: &mut BudgetMeter,
    path: &WirePath,
    missing: impl Fn([u8; 32]) -> Error,
) -> Result<Vec<I>, Error> {
    meter.charge_edges(units.len() as u64, path)?;
    let mut resolved = allocate(units.len(), meter, path)?;
    for unit in units {
        meter.charge_work(1 + u64::from(known.len().max(1).ilog2()), path)?;
        resolved.push(
            known
                .get(unit.as_array())
                .copied()
                .ok_or_else(|| missing(*unit.as_array()))?,
        );
    }
    Ok(resolved)
}

impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
