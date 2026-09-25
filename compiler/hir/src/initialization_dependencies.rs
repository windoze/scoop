//! Direct property initialization uses from executable HIR and shared metadata.

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, ConeIdentity, GeneratedCallableKey,
    InitializationCallableRole, PersistentInitializationUnitId, PersistentPropertyAccessorId,
    ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WirePath};

mod committed;
mod errors;
mod properties;
mod shared;
use HirInitializationUseError as Error;
pub use errors::HirInitializationUseError;
pub(crate) use properties::accessor_initialization_unit;

/// A direct accessor use in one materialized initializer. Repeated occurrences
/// with the same four typed identities have the same initialization edge.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct HirPropertyInitializationUseV1 {
    local_unit: PersistentInitializationUnitId,
    provider: ConeIdentity,
    dependency_unit: PersistentInitializationUnitId,
    accessor: PersistentPropertyAccessorId,
}

impl HirPropertyInitializationUseV1 {
    pub const fn local_unit(self) -> PersistentInitializationUnitId {
        self.local_unit
    }
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }
    pub const fn dependency_unit(self) -> PersistentInitializationUnitId {
        self.dependency_unit
    }
    pub const fn accessor(self) -> PersistentPropertyAccessorId {
        self.accessor
    }
}

fn initializer_root(
    root: CallableMaterialization,
    identities: &ValidatedIdentityGraph,
    local_units: &[PersistentInitializationUnitId],
    meter: &mut BudgetMeter,
) -> Result<Option<PersistentInitializationUnitId>, Error> {
    meter.charge_work(1, &WirePath::root())?;
    let Some(generated) = root.generated_template() else {
        return Ok(None);
    };
    meter.charge_work(
        u64::from(identities.identity_count().max(1).ilog2()) + 1,
        &WirePath::root(),
    )?;
    let key = identities.canonical_key::<_, GeneratedCallableKey>(generated)?;
    let GeneratedCallableKey::Initialization {
        unit,
        role: InitializationCallableRole::Initializer,
    } = key.as_ref()
    else {
        // Lexical bodies retain their own root, even inside an initializer.
        return Ok(None);
    };
    if root.context() != CallableMaterializationContext::NoSubstitution {
        return Err(Error::InitializationRootContext(root));
    }
    meter.charge_work(local_units.len() as u64, &WirePath::root())?;
    if !local_units.contains(unit) {
        return Err(Error::MissingLocalUnit(*unit));
    }
    Ok(Some(*unit))
}

fn push(
    uses: &mut Vec<HirPropertyInitializationUseV1>,
    record: HirPropertyInitializationUseV1,
    consumer: ConeIdentity,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    if record.provider == consumer {
        return Err(Error::LocalProvider(consumer));
    }
    meter.check_table_entries(uses.len() as u64 + 1, &WirePath::root())?;
    meter.charge_owned_bytes(
        std::mem::size_of::<HirPropertyInitializationUseV1>() as u64,
        &WirePath::root(),
    )?;
    meter.try_reserve_collection_slots(uses, 1, &WirePath::root())?;
    uses.push(record);
    Ok(())
}

fn canonicalize(
    mut uses: Vec<HirPropertyInitializationUseV1>,
    meter: &mut BudgetMeter,
) -> Result<Vec<HirPropertyInitializationUseV1>, Error> {
    meter.charge_work(
        (uses.len() as u64).saturating_mul(u64::from(uses.len().max(1).ilog2()) + 1),
        &WirePath::root(),
    )?;
    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}

fn local_unit_ids(
    units: impl ExactSizeIterator<Item = PersistentInitializationUnitId>,
    meter: &mut BudgetMeter,
) -> Result<Vec<PersistentInitializationUnitId>, Error> {
    let mut ids = Vec::new();
    meter.check_table_entries(units.len() as u64, &WirePath::root())?;
    meter.charge_work(units.len() as u64, &WirePath::root())?;
    meter.charge_owned_bytes(
        (units.len() as u64)
            .saturating_mul(std::mem::size_of::<PersistentInitializationUnitId>() as u64),
        &WirePath::root(),
    )?;
    meter.try_reserve_collection_slots(&mut ids, units.len(), &WirePath::root())?;
    ids.extend(units);
    Ok(ids)
}
