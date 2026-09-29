//! Bindings for already materialized, locally exported derived equality bodies.

use scoop_hir as hir;
use scoop_identity::{
    CallableDefinitionOwner, CallableMaterializationContext, CallableTemplateOwner,
    GeneratedCallableKey, PersistentGeneratedCallableId, ValidatedIdentityGraph,
};
use scoop_mir as mir;
use scoop_wire::WirePath;

mod binding;
mod error;
pub use error::SourceMirEqualityProductionError;
type Error = SourceMirEqualityProductionError;

pub fn lower_derived_equality_bindings(
    output: &hir::DependencyHirOutput,
    input: &mir::ConeMirInput,
    local_types: &mir::CanonicalParamFreeMirTypeExportsV1,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
) -> Result<mir::CanonicalMirCallableBindingsV1, Error> {
    let local = output.output().local.module();
    let mut records = Vec::new();
    for (id, function) in local.functions.iter() {
        let CallableTemplateOwner::Generated(callable) = function.materialization.template() else {
            continue;
        };
        let key = identities.canonical_key::<_, GeneratedCallableKey>(callable)?;
        let GeneratedCallableKey::DerivedEquality { exact_owner } = *key else {
            continue;
        };

        if local_types.get(exact_owner).is_none() {
            continue;
        }
        if function.materialization.context() != CallableMaterializationContext::NoSubstitution {
            return Err(Error::InvalidMaterialization(callable));
        }

        scoop_wire::allocation::try_reserve(&mut records, 1, &WirePath::root())?;
        records.push(binding::project(
            local, id, input, callable, &key, identities, types,
        )?);
    }

    Ok(mir::CanonicalMirCallableBindingsV1::try_new(records)?)
}
