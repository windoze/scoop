use scoop_identity::{CborIdentityRecord, DispatchTableKey};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

use super::SharedLirDispatchValidationError as Error;

mod entries;
mod lookup;
mod schemas;

/// Borrowed checked layout/ABI constituents; this carries no selected-use authority.
#[derive(Clone, Copy)]
pub struct SharedLirDispatchAbiInputsV1<'a> {
    pub local_layouts: &'a lir::CanonicalExactLayoutExportsV1,
    pub local_callables: &'a lir::CanonicalExactCallableAbiExportsV1,
    pub dependency_layouts: &'a [&'a lir::CanonicalExactLayoutExportsV1],
    pub dependency_callables: &'a [&'a lir::CanonicalExactCallableAbiExportsV1],
}

/// Derives the complete dispatch inventory from checked MIR representation
/// roles and schemas, then joins each target to its actual provider's ABI.
pub fn replay_shared_mir_dispatch(
    target: lir::LirTargetProfile,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    schemas: &mir::CanonicalMirDispatchSchemasV1,
    abis: SharedLirDispatchAbiInputsV1<'_>,
    foundation: &lir::OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalExactDispatchExportsV1, Error> {
    let abis = lookup::Abis::new(abis, target, foundation.producer(), meter)?;
    let path = WirePath::root();
    meter.check_table_entries(types.records().len() as u64, &path)?;
    let mut records = Vec::new();
    for ty in types.records() {
        meter.charge_work(1, &path)?;
        if matches!(
            ty.representation(),
            mir::MirTypeRepresentationV1::ObjectBacking { .. }
        ) {
            continue;
        }
        let schema = schemas::for_owner(types, schemas, ty, meter)?;
        meter.try_reserve_collection_slots(&mut records, 1 + schema.itables().len(), &path)?;
        records.push(replay(
            target,
            DispatchTableKey::vtable(ty.exact()),
            schema.vtable(),
            &abis,
            foundation,
            meter,
        )?);
        for table in schema.itables() {
            records.push(replay(
                target,
                DispatchTableKey::itable(ty.exact(), table.interface()),
                table.entries(),
                &abis,
                foundation,
                meter,
            )?);
        }
    }
    Ok(lir::CanonicalExactDispatchExportsV1::try_new(
        target, foundation, records, meter,
    )?)
}

fn replay(
    target: lir::LirTargetProfile,
    key: DispatchTableKey,
    slots: &[mir::MirDispatchEntryV1],
    abis: &lookup::Abis<'_>,
    foundation: &lir::OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<lir::ExactDispatchExportV1, Error> {
    meter.charge_work(1, &WirePath::root())?;
    let identity = CborIdentityRecord::from_key(key)?;
    let entries = entries::project(slots, abis, meter)?;
    lir::ExactDispatchExportV1::replay_from_schema(target, &identity, &entries, foundation, meter)
        .map_err(|source| Error::Replay {
            table: identity.id(),
            source: Box::new(source),
        })
}
