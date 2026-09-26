use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::SharedLirCallableAbiValidationError as Error;

use super::super::lir_callable_layouts::Layouts;

/// Replays the complete ABI constituent from HIR-joined MIR bindings and
/// already checked layouts. This does not authorize a selected use or import.
pub fn replay_shared_mir_callable_abis(
    target: lir::LirTargetProfile,
    bindings: &mir::CanonicalMirCallableBindingsV1,
    local: &lir::CanonicalExactLayoutExportsV1,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    foundation: &lir::OdrFreeLirFoundation,
    identities: &scoop_identity::ValidatedIdentityGraph,
) -> Result<lir::CanonicalExactCallableAbiExportsV1, Error> {
    let mut layouts = Layouts::new(local, dependencies, target, foundation.producer())?;
    let path = WirePath::root();

    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, bindings.entries().len(), &path)?;
    for binding in bindings.entries() {
        let implementation = binding.implementation();
        let signature = binding.lowered_signature();
        let effect = match signature.gc_effect() {
            mir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
            mir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
        };
        let signature = layouts.signature(signature.exact(), effect, identities)?;
        let record = lir::ExactCallableAbiExportV1::from_signature(
            target,
            implementation,
            signature,
            foundation,
        )
        .map_err(|source| Error::Callable {
            target: implementation,
            source: Box::new(source),
        })?;
        records.push(record);
    }
    Ok(lir::CanonicalExactCallableAbiExportsV1::try_new(
        target, foundation, records,
    )?)
}
