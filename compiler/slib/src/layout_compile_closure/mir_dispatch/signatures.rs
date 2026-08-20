//! Check physical slot signatures against the actual core coroutine protocol.

use super::*;
use scoop_identity::{
    Effect, ExactCallableSignature, ExactTypeKey, GeneratedNominalKey, NonEmptyVec,
    PersistentTypeId,
};

impl Replay<'_> {
    pub(super) fn lowered_signature(
        &self,
        source: &mir::MirBridgeCallableSignatureV1,
    ) -> Result<mir::MirBridgeCallableSignatureV1, Error> {
        let signature = source.exact();
        if signature.effect() == Effect::Ordinary {
            return Ok(source.clone());
        }
        let core = self.core.ok_or(Error::MissingCoroutineProtocol)?;
        let continuation = PersistentExactTypeId::from_key(&ExactTypeKey::NominalApplication {
            origin: core.continuation(),
            arguments: NonEmptyVec::from_first(signature.result(), []),
        })
        .map_err(Error::Hash)?;
        let step = PersistentTypeId::from_generated_key(&GeneratedNominalKey::CoroutineStep {
            result: signature.result(),
        })
        .map_err(Error::Nominal)?;
        let step =
            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(step)).map_err(Error::Hash)?;
        let mut parameters = signature.parameters().to_vec();
        parameters.push(continuation);
        Ok(mir::MirBridgeCallableSignatureV1::new(
            ExactCallableSignature::new(
                Effect::Ordinary,
                signature.receiver().into_option(),
                parameters,
                step,
            ),
            source.gc_effect(),
        ))
    }
}
