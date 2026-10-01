//! Join validated MIR coroutine signatures with the actual core protocol roles.

use std::collections::BTreeSet;

use scoop_hir::CoreCoroutineProtocolV1;
use scoop_identity::{
    CallableDefinitionOwner, Effect, ExactTypeKey, GeneratedCallableKey, IdentityReferenceError,
    PersistentExactTypeId, PersistentGenericTypeId, ValidatedIdentityGraph,
};
use scoop_mir::{CanonicalMirCallableBindingsV1, CanonicalMirShapeSupportsV1, MirCallableOriginV1};

pub fn validate_shared_mir_coroutines(
    core: Option<&CoreCoroutineProtocolV1>,
    identities: &ValidatedIdentityGraph,
    shapes: &CanonicalMirShapeSupportsV1,
    callables: &CanonicalMirCallableBindingsV1,
) -> Result<(), SharedMirCoroutineValidationError> {
    let mut starts = BTreeSet::new();
    for binding in callables.entries() {
        let source = binding.semantic_signature().exact();
        if source.effect() == Effect::Suspend {
            let core = core.ok_or(Error::MissingCoreProtocol)?;
            let actual = *binding
                .lowered_signature()
                .exact()
                .parameters()
                .last()
                .expect("validated suspend ABI contains a hidden continuation");
            if !application(identities, actual, core.continuation(), source.result())? {
                return Err(Error::Continuation(binding.implementation()));
            }
        }
        if let MirCallableOriginV1::Generated {
            role: GeneratedCallableKey::CoroutineStart { result },
            ..
        } = binding.origin()
        {
            let core = core.ok_or(Error::MissingCoreProtocol)?;
            let parameters = source.parameters();
            if !application(identities, parameters[0], core.suspend_task(), *result)?
                || !application(identities, parameters[1], core.continuation(), *result)?
            {
                return Err(Error::StartParameters(binding.implementation()));
            }
            starts.insert(*result);
        }
    }
    for shape in shapes.records() {
        if !starts.contains(&shape.exact()) {
            return Err(Error::MissingStart(shape.exact()));
        }
    }
    Ok(())
}

fn application(
    identities: &ValidatedIdentityGraph,
    exact: PersistentExactTypeId,
    expected: PersistentGenericTypeId,
    result: PersistentExactTypeId,
) -> Result<bool, Error> {
    let key = identities
        .canonical_key::<_, ExactTypeKey>(exact)
        .map_err(Error::Identity)?;
    Ok(
        matches!(key.as_ref(), ExactTypeKey::NominalApplication { origin, arguments }
        if *origin == expected && arguments.as_slice() == [result]),
    )
}

type Error = SharedMirCoroutineValidationError;

#[derive(Debug)]
pub enum SharedMirCoroutineValidationError {
    Identity(IdentityReferenceError),
    MissingCoreProtocol,
    MissingStart(PersistentExactTypeId),
    StartParameters(CallableDefinitionOwner),
    Continuation(CallableDefinitionOwner),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "shared MIR coroutine protocol agreement: {self:?}"
        )
    }
}
impl std::error::Error for Error {}
