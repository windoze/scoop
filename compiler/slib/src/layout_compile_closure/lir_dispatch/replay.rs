use scoop_identity::{CborIdentityRecord, DispatchTableKey};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::SharedLirDispatchValidationError as Error;

mod entries;
mod lookup;
mod schemas;

/// Borrows checked layouts and ABIs together with the stored dispatch references.
#[derive(Clone, Copy)]
pub struct SharedLirDispatchAbiInputsV1<'a> {
    pub local_dispatch: &'a lir::DecodedCanonicalExactDispatchExportsV1,
    pub local_layouts: &'a lir::CanonicalExactLayoutExportsV1,
    pub local_callables: &'a lir::CanonicalExactCallableAbiExportsV1,
    pub local_direct_callables: &'a lir::CrossConeLirBridgeSectionV1,
    pub dependency_layouts: &'a [&'a lir::CanonicalExactLayoutExportsV1],
    pub dependency_callables: &'a [&'a lir::CanonicalExactCallableAbiExportsV1],
    pub dependency_direct_callables: &'a [&'a lir::CrossConeLirBridgeSectionV1],
}

/// Derives the complete dispatch inventory from checked MIR representation
/// roles and schemas, then joins each target to its actual provider's ABI.
pub fn replay_shared_mir_dispatch(
    target: lir::LirTargetProfile,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    schemas: &mir::CanonicalMirDispatchSchemasV1,
    abis: SharedLirDispatchAbiInputsV1<'_>,
    foundation: &lir::ConeLirFoundation,
) -> Result<lir::CanonicalExactDispatchExportsV1, Error> {
    let abis = lookup::Abis::new(abis, target, foundation.producer())?;
    let path = WirePath::root();

    let mut records = Vec::new();
    for ty in types.records() {
        if matches!(
            ty.representation(),
            mir::MirTypeRepresentationV1::ObjectBacking { .. }
        ) {
            continue;
        }
        let schema = schemas::for_owner(types, schemas, ty)?;
        scoop_wire::allocation::try_reserve(&mut records, 1 + schema.itables().len(), &path)?;
        records.push(replay(
            target,
            DispatchTableKey::vtable(ty.exact()),
            schema.vtable(),
            &abis,
            foundation,
        )?);
        for table in schema.itables() {
            records.push(replay(
                target,
                DispatchTableKey::itable(ty.exact(), table.interface()),
                table.entries(),
                &abis,
                foundation,
            )?);
        }
    }
    Ok(lir::CanonicalExactDispatchExportsV1::try_new(
        target, foundation, records,
    )?)
}

fn replay(
    target: lir::LirTargetProfile,
    key: DispatchTableKey,
    slots: &[mir::MirDispatchEntryV1],
    abis: &lookup::Abis<'_>,
    foundation: &lir::ConeLirFoundation,
) -> Result<lir::ExactDispatchExportV1, Error> {
    let identity = CborIdentityRecord::from_key(key)?;
    let entries = entries::project(identity.id(), slots, abis)?;
    lir::ExactDispatchExportV1::replay_from_schema(target, &identity, &entries, foundation).map_err(
        |source| Error::Replay {
            table: identity.id(),
            source: Box::new(source),
        },
    )
}
