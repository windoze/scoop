//! Constructor bindings from the same sealed HIR and actual MIR production.

use scoop_hir as hir;
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableOwner, CallableTemplateOwner,
    ExactCallableSignature, PersistentConstructorId, StrongCallableDefinitionOwner,
    ValidatedIdentityGraph,
};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WireError, WirePath};

mod binding;
use binding::Producer;

/// Produces all public/protected constructor bindings required by the local
/// type interface. Imported types stay borrowed through the shared lookup.
pub fn lower_constructor_bindings(
    output: &hir::DependencyHirOutput,
    source: &hir::CrossConeTypeSemanticsProductionV1,
    input: &mir::SingleConeStrongMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<mir::CanonicalMirCallableBindingsV1, SourceMirConstructorProductionError> {
    let local = output.output().local.module();
    let mut producer = Producer::new(source, input, identities, types, meter)?;
    for (id, constructor) in local.class_constructors.iter() {
        let Some(source) = producer.source(constructor.materialization)? else {
            continue;
        };
        producer.signature_cost(local.types.len(), constructor.parameters.len())?;
        let lowered = crate::source_callables::exact_class_initializer_signature(local, id);
        let owner = lowered.receiver().into_option().ok_or(
            SourceMirConstructorProductionError::InvalidClassSignature(source.declaration()),
        )?;
        let semantic = ExactCallableSignature::new(
            lowered.effect(),
            None,
            lowered.parameters().to_vec(),
            owner,
        );
        producer.record(
            source,
            semantic,
            lowered,
            mir::MirCallableLoweringRoleV1::ClassInitializer { owner },
        )?;
    }
    for (id, constructor) in local.struct_constructors.iter() {
        let Some(source) = producer.source(constructor.materialization)? else {
            continue;
        };
        producer.signature_cost(local.types.len(), constructor.parameters.len())?;
        let lowered = crate::source_callables::exact_struct_constructor_signature(local, id);
        let owner = lowered.result();
        producer.record(
            source,
            lowered.clone(),
            lowered,
            match constructor.kind {
                hir::concrete::StructConstructorKind::Primary => {
                    mir::MirCallableLoweringRoleV1::PrimaryValueConstructor { owner }
                }
                hir::concrete::StructConstructorKind::Secondary { .. } => {
                    mir::MirCallableLoweringRoleV1::ValueConstructor { owner }
                }
            },
        )?;
    }
    producer.finish()
}

#[derive(Debug)]
pub enum SourceMirConstructorProductionError {
    Resource(WireError),
    Bridge(mir::MirCallableBridgeError),
    IncompleteConstructors { expected: usize, actual: usize },
    MissingMaterialization(PersistentConstructorId),
    MissingSignature(PersistentConstructorId),
    SourceSignatureMismatch(PersistentConstructorId),
    InvalidClassSignature(PersistentConstructorId),
    OdrRequired(PersistentConstructorId),
}
impl From<WireError> for SourceMirConstructorProductionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<mir::MirCallableBridgeError> for SourceMirConstructorProductionError {
    fn from(error: mir::MirCallableBridgeError) -> Self {
        Self::Bridge(error)
    }
}
impl std::fmt::Display for SourceMirConstructorProductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot produce MIR constructor bindings: {self:?}")
    }
}
impl std::error::Error for SourceMirConstructorProductionError {}

type Error = SourceMirConstructorProductionError;
