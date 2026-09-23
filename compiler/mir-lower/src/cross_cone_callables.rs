//! Ordinary source bodies selected by the public and nominal HIR interfaces.

use scoop_hir as hir;
use scoop_identity::{
    CallableMaterializationContext, CallableTemplateOrigin, CallableTemplateOwner,
    DependencyCallableDeclarationId, ExactCallableSignature, ValidatedIdentityGraph,
};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WireError, WirePath};
use std::collections::BTreeMap;

mod accessors;
mod binding;
mod inventory;
mod roles;

/// Produces ordinary source bodies, accessors and actual class trap bodies.
/// Constructors and generated adaptors are separate constituents of the same
/// final callable table; abstract declarations retain fatal trap bindings.
pub fn lower_source_callable_bindings(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
    source: &hir::CrossConeTypeSemanticsProductionV1,
    input: &mir::SingleConeStrongMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<mir::CanonicalMirCallableBindingsV1, SourceMirCallableProductionError> {
    let mut required = inventory::collect(public, source, meter)?;
    accessors::project(
        output.output().export.module(),
        source,
        &mut required,
        meter,
    )?;
    let mut records = Vec::new();
    reserve(&mut records, required.len(), meter)?;
    let local = output.output().local.module();
    for (id, function) in local.functions.iter() {
        meter.charge_nodes(1, &WirePath::root())?;
        work(
            u64::from(required.len().checked_ilog2().unwrap_or(0)) + 1,
            meter,
        )?;
        let declaration = match function.materialization.template() {
            CallableTemplateOwner::Function(id) => Declaration::Function(id),
            CallableTemplateOwner::Accessor(id) => Declaration::PropertyAccessor(id),
            _ => continue,
        };
        let Some(contract) = required.remove(&declaration) else {
            continue;
        };
        if function.materialization.context() != CallableMaterializationContext::NoSubstitution {
            return Err(Error::OdrRequired(declaration));
        }
        work(function.params.len() as u64 * 3 + 1, meter)?;
        meter.charge_collection_slots(function.params.len() as u64 * 2, &WirePath::root())?;
        meter.charge_owned_bytes(
            (function.params.len() as u64).saturating_mul(
                2 * std::mem::size_of::<scoop_identity::PersistentExactTypeId>() as u64,
            ),
            &WirePath::root(),
        )?;
        let expected = crate::source_callables::exact_function_signature(local, id);
        let role = roles::project(local, function, declaration, contract.modality)?;
        records.push(binding::project(
            input,
            identities,
            types,
            declaration,
            contract,
            expected,
            role,
            meter,
        )?);
    }
    if let Some((&missing, _)) = required.first_key_value() {
        return Err(Error::MissingSourceMaterialization(missing));
    }
    work(
        (records.len() as u64)
            .saturating_mul(u64::from(records.len().checked_ilog2().unwrap_or(0)) + 1),
        meter,
    )?;
    Ok(mir::CanonicalMirCallableBindingsV1::try_new(records)?)
}

type Declaration = DependencyCallableDeclarationId;
type Error = SourceMirCallableProductionError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceContract {
    execution: scoop_identity::Effect,
    gc: mir::GcEffect,
    modality: hir::CallableModalityV1,
}

impl SourceContract {
    fn new(effects: hir::CallableSourceEffectsV1, modality: hir::CallableModalityV1) -> Self {
        Self {
            execution: effects.execution(),
            gc: match effects.gc_effect() {
                scoop_identity::GcEffect::Managed => mir::GcEffect::Managed,
                scoop_identity::GcEffect::NoGc => mir::GcEffect::NoGc,
            },
            modality,
        }
    }
}

#[derive(Debug)]
pub enum SourceMirCallableProductionError {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Materialization(hir::NominalMaterializationClosureError),
    Bridge(mir::MirCallableBridgeError),
    ConflictingSource(DependencyCallableDeclarationId),
    MissingSourceContract(DependencyCallableDeclarationId),
    MissingSourceMaterialization(DependencyCallableDeclarationId),
    MissingMirMaterialization(DependencyCallableDeclarationId),
    MissingSignature(DependencyCallableDeclarationId),
    SourceSignatureMismatch(DependencyCallableDeclarationId),
    InvalidSourceRole(DependencyCallableDeclarationId),
    OdrRequired(DependencyCallableDeclarationId),
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<mir::MirCallableBridgeError> for Error {
    fn from(error: mir::MirCallableBridgeError) -> Self {
        Self::Bridge(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot produce source MIR callable bindings: {self:?}")
    }
}
impl std::error::Error for Error {}

fn work(count: u64, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(count, &WirePath::root())?)
}
fn reserve<T>(values: &mut Vec<T>, count: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    meter.charge_owned_bytes(
        (count as u64).saturating_mul(std::mem::size_of::<T>() as u64),
        &WirePath::root(),
    )?;
    Ok(meter.try_reserve_collection_slots(values, count, &WirePath::root())?)
}
