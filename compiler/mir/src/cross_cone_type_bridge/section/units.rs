use super::*;
use scoop_identity::{
    GeneratedCallableKey, InitializationCallableRole, PersistentGeneratedCallableId,
};

mod shared;
mod validation;
pub use shared::replay_source_initialization_units;

/// Reader replay is a semantic proof, never evidence of an emitted body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirInitializationUnitProofKindV1 {
    ProducerEmitted(crate::StrongInitializationUnitMaterializationRoot),
    ReaderSemanticReplay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTypeBridgeInitializationUnitV1 {
    unit: PersistentInitializationUnitId,
    initializer: StrongCallableDefinitionOwner,
    ensure: StrongCallableDefinitionOwner,
    signature: MirBridgeCallableSignatureV1,
    proof: MirInitializationUnitProofKindV1,
}
impl MirTypeBridgeInitializationUnitV1 {
    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.unit
    }
    pub const fn initializer(&self) -> StrongCallableDefinitionOwner {
        self.initializer
    }
    pub const fn ensure(&self) -> StrongCallableDefinitionOwner {
        self.ensure
    }
    pub const fn signature(&self) -> &MirBridgeCallableSignatureV1 {
        &self.signature
    }
    pub const fn proof_kind(&self) -> MirInitializationUnitProofKindV1 {
        self.proof
    }
}

pub(super) fn build<E>(
    authority: MirTypeBridgeLocalAuthorityV1<'_>,
    source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
    graph: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<Vec<MirTypeBridgeInitializationUnitV1>, MirTypeBridgeSectionError<E>> {
    let required = source
        .local_initialization_units()
        .map_err(MirTypeBridgeSectionError::Source)?;
    let path = WirePath::root();
    meter.check_table_entries(required.len() as u64, &path)?;
    meter.charge_work(required.len() as u64, &path)?;
    if required.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MirTypeBridgeSectionError::NonCanonicalUnitInventory);
    }
    let mut units = Vec::new();
    meter.try_reserve_collection_slots(&mut units, required.len(), &path)?;
    for unit in required {
        units.push(validation::unit(
            authority, source, *unit, graph, types, meter,
        )?);
    }
    if let MirTypeBridgeLocalAuthorityV1::Producer { input, .. } = authority {
        let roots = input.materialization().initialization_roots();
        meter.charge_work(roots.len() as u64, &path)?;
        if roots.len() != units.len() {
            return Err(MirTypeBridgeSectionError::ProviderContext);
        }
        for root in roots {
            if units
                .binary_search_by_key(&root.identity(), MirTypeBridgeInitializationUnitV1::unit)
                .is_err()
            {
                return Err(MirTypeBridgeSectionError::Unit {
                    unit: root.identity(),
                    problem: MirTypeBridgeUnitProblemV1::ProducerInventory,
                });
            }
        }
    }
    Ok(units)
}
