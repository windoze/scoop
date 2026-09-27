//! Ordinary source bodies selected by the public and nominal HIR interfaces.

use scoop_hir as hir;
use scoop_identity::{
    CallableMaterializationContext, CallableTemplateOwner, DependencyCallableDeclarationId,
    ExactCallableSignature, ValidatedIdentityGraph,
};
use scoop_mir as mir;
use scoop_wire::{WireError, WirePath};
mod binding;
mod roles;

/// Produces ordinary source bodies, accessors and actual class trap bodies.
/// Constructors and generated adaptors are separate constituents of the same
/// final callable table; abstract declarations retain fatal trap bindings.
pub fn lower_source_callable_bindings(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
    source: &hir::CrossConeTypeSemanticsSectionV1,
    input: &mir::ConeMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    ordinary: &[mir::ParamFreeMirCallableExportV1],
) -> Result<mir::CanonicalMirCallableBindingsV1, SourceMirCallableProductionError> {
    let mut required =
        hir::select_param_free_source_callables(input.module().cone, public, source, identities)
            .map_err(|error| match error {
                hir::SharedTypeMetadataError::Resource(error) => Error::Resource(error),
                error => Error::SharedSource(Box::new(error)),
            })?;
    for callable in ordinary {
        required.remove(&callable.declaration());
    }
    let mut records = Vec::new();
    reserve(&mut records, required.len())?;
    let local = output.output().local.module();
    for (id, function) in local.functions.iter() {
        let declaration = match function.materialization.template() {
            CallableTemplateOwner::Function(id) => Declaration::Function(id),
            CallableTemplateOwner::Accessor(id) => Declaration::PropertyAccessor(id),
            _ => continue,
        };
        let Some(source) = required.remove(&declaration) else {
            continue;
        };
        let contract = SourceContract::new(source.effects(), source.modality());
        if function.materialization.context() != CallableMaterializationContext::NoSubstitution {
            return Err(Error::OdrRequired(declaration));
        }

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
        )?);
    }
    if let Some((&missing, _)) = required.first_key_value() {
        return Err(Error::MissingSourceMaterialization(missing));
    }

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
    SharedSource(Box<hir::SharedTypeMetadataError>),
    Bridge(mir::MirCallableBridgeError),
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

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), Error> {
    Ok(scoop_wire::allocation::try_reserve(
        values,
        count,
        &WirePath::root(),
    )?)
}
