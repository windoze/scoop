//! Bindings for already materialized, locally exported derived equality bodies.

use scoop_hir as hir;
use scoop_identity::{
    CallableMaterializationContext, CallableOwner, CallableTemplateOwner, GeneratedCallableKey,
    PersistentGeneratedCallableId, StrongCallableDefinitionOwner, ValidatedIdentityGraph,
};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

mod binding;
mod error;
pub use error::SourceMirEqualityProductionError;
type Error = SourceMirEqualityProductionError;

pub fn lower_derived_equality_bindings(
    output: &hir::DependencyHirOutput,
    input: &mir::SingleConeStrongMirInput,
    local_types: &mir::CanonicalParamFreeMirTypeExportsV1,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<mir::CanonicalMirCallableBindingsV1, Error> {
    let local = output.output().local.module();
    let mut records = Vec::new();
    for (id, function) in local.functions.iter() {
        meter.charge_work(1, &WirePath::root())?;
        let CallableTemplateOwner::Generated(callable) = function.materialization.template() else {
            continue;
        };
        let key = identities.canonical_key::<_, GeneratedCallableKey>(callable)?;
        let GeneratedCallableKey::DerivedEquality { exact_owner } = *key else {
            continue;
        };
        meter.charge_work(search(local_types.records().len()), &WirePath::root())?;
        if local_types.get(exact_owner).is_none() {
            continue;
        }
        if function.materialization.context() != CallableMaterializationContext::NoSubstitution {
            return Err(Error::OdrRequired(callable));
        }
        meter.charge_nodes(1, &WirePath::root())?;
        meter.charge_owned_bytes(
            std::mem::size_of::<mir::ParamFreeMirCallableBindingV1>() as u64,
            &WirePath::root(),
        )?;
        meter.try_reserve_collection_slots(&mut records, 1, &WirePath::root())?;
        records.push(binding::project(
            local, id, input, callable, &key, identities, types, meter,
        )?);
    }
    meter.charge_work(
        records.len() as u64 * search(records.len()),
        &WirePath::root(),
    )?;
    Ok(mir::CanonicalMirCallableBindingsV1::try_new(records)?)
}

fn search(count: usize) -> u64 {
    u64::from(count.checked_ilog2().unwrap_or(0)) + 1
}
