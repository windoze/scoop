//! Applied operation signatures from the declaration owner's bound source tables.
use super::nominal_binding::Applied;
use crate::*;
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOriginSubject, OptionalSignatureType,
    PersistentConstructorId, SignatureTypeKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

mod constructors;
mod errors;
mod members;
pub use errors::DefaultSourceCallableOperationError;
type Error = DefaultSourceCallableOperationError;

fn query(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), Error> {
    meter.check_semantic_depth(1, path)?;
    meter.charge_nodes(1, path)?;
    meter.charge_edges(1, path)?;
    meter.charge_work(
        (u64::from(count.max(1).ilog2()) + 1).saturating_mul(32),
        path,
    )?;
    Ok(())
}
fn owner_matches(applied: &Applied<'_, '_>, expected: SourceNominalId) -> Result<(), Error> {
    let actual = applied.source.owner();
    if actual != expected {
        return Err(Error::Owner { expected, actual });
    }
    Ok(())
}
