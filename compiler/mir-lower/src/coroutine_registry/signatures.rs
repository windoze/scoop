//! The physical signature shared by suspend bodies and dispatch slots.

use super::*;
use scoop_identity::{CborIdentityRecord, Effect, ExactCallableSignature, ExactTypeKey};

pub(crate) fn lowered_signature(
    module: &hir::Module,
    source: &ExactCallableSignature,
) -> ExactCallableSignature {
    if source.effect() == Effect::Ordinary {
        return source.clone();
    }
    let protocol = module
        .coroutine_protocols
        .iter()
        .find(|protocol| module.exact_type_identities[protocol.result_type].id() == source.result())
        .expect("every concrete suspend result has its complete protocol");
    let continuation =
        module.exact_type_identities[module.interfaces[protocol.continuation].canonical_type].id();
    let step = CborIdentityRecord::from_key(scoop_identity::GeneratedNominalKey::CoroutineStep {
        result: source.result(),
    })
    .expect("the result defines one coroutine step");
    let step_exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(step.id()))
        .expect("a coroutine step has one exact nominal identity");
    let mut parameters = source.parameters().to_vec();
    parameters.push(continuation);
    ExactCallableSignature::new(
        Effect::Ordinary,
        source.receiver().into_option(),
        parameters,
        step_exact.id(),
    )
}
