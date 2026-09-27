//! Rebuild object membership from the same artifact's typed producer units.

use super::resources::allocate;
use super::*;
use scoop_wire::{WirePath, encode_canonical_temporary};

type Error = LinkObjectMaterializationValidationError;

impl DecodedLinkIdentityClosureSectionV1 {
    /// Borrows the original wire so layout replay retains every later field.
    /// This proves member assignment only, not object contents or Code identity.
    pub fn replay_materializations(
        &self,
        partition: &ProducerUnitPartitionV1,
    ) -> Result<PlannedLinkObjectMemberSetV1, Error> {
        let path = WirePath::root().field(1);
        let definitions = partition.scoop_lir_definition_plans();
        let bridges = partition.generated_bridge_units();

        let scoop_ids = definitions.iter().map(|id| (*id.as_array(), *id)).collect();
        let bridge_ids = bridges
            .iter()
            .map(|unit| (*unit.unit().as_array(), unit.unit()))
            .collect();
        let count = self.materializations.len();
        let mut scoop_sets = allocate(count, &path)?;
        let mut bridge_sets = allocate(count, &path)?;
        for (index, materialization) in self.materializations.iter().enumerate() {
            let path = path.clone().index(index as u64);

            match materialization {
                DecodedLinkObjectMaterializationV1::ScoopLir { units, .. } => {
                    scoop_sets.push(
                        CanonicalScoopLirObjectUnitSetV1::new(resolve(
                            units,
                            &scoop_ids,
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

        let actual = encode_canonical_temporary(&WireArray(&self.materializations), &path)?;

        let plan = PlannedLinkObjectMemberSetV1::new(partition, scoop_sets, bridge_sets)
            .map_err(Error::MemberPlan)?;
        let expected = super::super::materializations(&plan);
        let expected = encode_canonical_temporary(&WireArray(&expected), &path)?;

        if actual != expected {
            return Err(Error::ProjectionMismatch);
        }
        Ok(plan)
    }
}

fn resolve<I: PersistentId + Copy>(
    units: &[DecodedPersistentId<I>],
    known: &BTreeMap<[u8; 32], I>,

    path: &WirePath,
    missing: impl Fn([u8; 32]) -> Error,
) -> Result<Vec<I>, Error> {
    let mut resolved = allocate(units.len(), path)?;
    for unit in units {
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
