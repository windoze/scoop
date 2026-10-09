use scoop_lir as lir;
use scoop_mir as mir;

use super::SharedLirCallableAbiValidationError as Error;

use super::super::lir_callable_layouts::Layouts;

/// Joins saved physical ABI plans to the checked MIR signatures and layouts.
/// Register classification belongs to LIR lowering and is not repeated here.
pub fn validate_shared_mir_callable_abis(
    target: lir::LirTargetProfile,
    bindings: &mir::CanonicalMirCallableBindingsV1,
    local: &lir::CanonicalExactLayoutExportsV1,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    foundation: &lir::ConeLirFoundation,
    identities: &scoop_identity::ValidatedIdentityGraph,
    actual: &lir::CanonicalExactCallableAbiExportsV1,
) -> Result<(), Error> {
    let mut layouts = Layouts::new(local, dependencies, target, foundation.producer())?;
    if actual.provider() != foundation.producer() {
        return Err(Error::LocalProvider);
    }
    if actual.target() != target {
        return Err(Error::LocalTarget);
    }
    if actual.records().len() != bindings.entries().len() {
        return Err(lir::ExactCallableAbiTableError::Count.into());
    }
    for binding in bindings.entries() {
        let implementation = binding.implementation();
        let signature = binding.lowered_signature();
        let effect = match signature.gc_effect() {
            mir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
            mir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
        };
        let actual = actual
            .get(implementation)
            .ok_or(Error::SignatureMismatch(implementation))?
            .canonical_signature();
        if actual.signature() != signature.exact() || actual.gc_effect() != effect {
            return Err(Error::SignatureMismatch(implementation));
        }
        layouts.check_signature(actual, identities)?;
    }
    Ok(())
}
