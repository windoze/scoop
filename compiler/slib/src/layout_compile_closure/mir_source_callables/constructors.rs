use scoop_hir as hir;
use scoop_identity::{PersistentConstructorId, StrongCallableDefinitionOwner};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

mod errors;
mod signatures;
use SharedMirConstructorValidationError as Error;
pub use errors::{SharedMirConstructorComponent, SharedMirConstructorValidationError};

/// Replays constructor exports using shared declarations, without local HIR or
/// a caller-provided inventory of expected machine bindings.
pub fn validate_shared_mir_constructors(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    callables: &mir::CanonicalMirCallableBindingsV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let metadata = source.metadata();
    let required = hir::select_param_free_source_constructors(
        source.provider(),
        metadata.public,
        metadata.identities,
        meter,
    )?;
    for (&declaration, &source) in &required {
        lookup(callables.entries().len(), meter)?;
        let binding = callables
            .get(StrongCallableDefinitionOwner::Constructor(declaration))
            .ok_or(Error::Missing(declaration))?;
        signatures::validate(declaration, metadata, source, binding, meter)?;
    }
    for binding in callables.entries() {
        meter.charge_work(1, &WirePath::root())?;
        if let mir::MirCallableOriginV1::Constructor(declaration) = binding.origin() {
            lookup(required.len(), meter)?;
            if !required.contains_key(declaration) {
                return Err(Error::Unexpected(*declaration));
            }
        }
    }
    Ok(())
}

fn lookup(length: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())?)
}
