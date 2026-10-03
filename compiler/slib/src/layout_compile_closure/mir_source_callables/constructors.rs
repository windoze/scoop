use scoop_hir as hir;
use scoop_identity::{PersistentConstructorId, StrongCallableDefinitionOwner};
use scoop_mir as mir;

mod errors;
mod signatures;
use SharedMirConstructorValidationError as Error;
pub use errors::{SharedMirConstructorComponent, SharedMirConstructorValidationError};

/// Replays constructor exports using shared declarations, without local HIR or
/// a caller-provided inventory of expected machine bindings.
pub fn validate_shared_mir_constructors(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    callables: &mir::CanonicalMirCallableBindingsV1,
) -> Result<(), Error> {
    let metadata = source.metadata();
    let required = hir::select_param_free_source_constructors(
        source.provider(),
        metadata.public,
        metadata.identities,
    )?;
    for (&declaration, &source) in &required {
        let binding = callables
            .get(StrongCallableDefinitionOwner::Constructor(declaration))
            .ok_or(Error::Missing(declaration))?;
        signatures::validate(declaration, metadata, source, binding)?;
    }
    for binding in callables.entries() {
        if let mir::MirCallableOriginV1::Constructor(declaration) = binding.origin() {
            if !required.contains_key(declaration) {
                return Err(Error::Unexpected(*declaration));
            }
        }
    }
    Ok(())
}
