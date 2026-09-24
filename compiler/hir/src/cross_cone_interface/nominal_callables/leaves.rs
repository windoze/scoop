use scoop_identity::{CoreBuiltinNominal, ExactTypeKey};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::{NominalExactLeafClassifierBuildError as Error, *};
use crate::{NominalInterfaceRecordV1, SourceNominalId};

impl NominalExactLeafClassifierV1 {
    pub fn try_from_nominal_interfaces<'a>(
        nominals: impl IntoIterator<Item = &'a NominalInterfaceRecordV1>,
    ) -> Result<Self, Error> {
        Self::try_from_nominal_interfaces_metered(
            nominals,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
    }

    pub fn try_from_nominal_interfaces_metered<'a>(
        nominals: impl IntoIterator<Item = &'a NominalInterfaceRecordV1>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, Error> {
        let mut leaves = Vec::new();
        append(
            &mut leaves,
            CoreBuiltinNominal::Unit.identity_record().id(),
            meter,
            path,
        )?;
        for nominal in nominals {
            meter.charge_work(1, path).map_err(Error::Resource)?;
            if let SourceNominalId::Concrete(source) = nominal.declaration() {
                append(&mut leaves, source, meter, path)?;
            }
        }
        let count = leaves.len() as u64;
        meter
            .charge_work(
                count.saturating_mul(2 + u64::from(count.max(1).ilog2())),
                path,
            )
            .map_err(Error::Resource)?;
        leaves.sort_unstable_by_key(|(source, _)| *source);
        leaves.dedup_by_key(|(source, _)| *source);
        Ok(Self { leaves })
    }
}

fn append(
    leaves: &mut Vec<(PersistentTypeId, PersistentExactTypeId)>,
    source: PersistentTypeId,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    meter
        .charge_owned_bytes(
            std::mem::size_of::<(PersistentTypeId, PersistentExactTypeId)>() as u64,
            path,
        )
        .map_err(Error::Resource)?;
    meter
        .try_reserve_collection_slots(leaves, 1, path)
        .map_err(Error::Resource)?;
    let exact =
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source)).map_err(Error::Identity)?;
    leaves.push((source, exact));
    Ok(())
}
