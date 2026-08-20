use super::*;
use scoop_identity::{Effect, GeneratedNominalKey};

impl MirCallableBridgeAuthority<'_> {
    pub(super) fn start_signature(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
        result: PersistentExactTypeId,
    ) -> Result<(), MirCallableBridgeError> {
        let signature = binding.semantic.exact();
        if binding.semantic != binding.lowered
            || signature.effect() != Effect::Ordinary
            || signature.receiver().is_present()
            || signature.parameters().len() != 2
            || signature.parameters()[0] == signature.parameters()[1]
            || !self.is_unit(signature.result())?
            || binding.semantic.gc_effect() != crate::GcEffect::Managed
        {
            return Err(MirCallableBridgeError::SignatureMismatch);
        }
        for parameter in signature.parameters() {
            let key = self
                .identities
                .canonical_key::<_, ExactTypeKey>(*parameter)?;
            if !matches!(key.as_ref(), ExactTypeKey::NominalApplication { arguments, .. }
                if arguments.as_slice() == [result])
                || !matches!(
                    self.type_export(*parameter)?.representation(),
                    MirTypeRepresentationV1::Interface
                )
            {
                return Err(MirCallableBridgeError::SignatureMismatch);
            }
        }
        Ok(())
    }

    pub(super) fn coroutine_signature(
        &self,
        source: &ExactCallableSignature,
        lowered: &ExactCallableSignature,
    ) -> Result<(), MirCallableBridgeError> {
        let mismatch = MirCallableBridgeError::SignatureMismatch;
        if source.effect() != Effect::Suspend
            || lowered.effect() != Effect::Ordinary
            || lowered.receiver() != source.receiver()
            || lowered.parameters().len() != source.parameters().len() + 1
            || !lowered.parameters().starts_with(source.parameters())
        {
            return Err(mismatch);
        }
        let continuation = *lowered
            .parameters()
            .last()
            .expect("the hidden parameter was checked");
        let continuation_key = self
            .identities
            .canonical_key::<_, ExactTypeKey>(continuation)?;
        if !matches!(continuation_key.as_ref(), ExactTypeKey::NominalApplication { arguments, .. } if arguments.as_slice() == [source.result()])
            || !matches!(
                self.type_export(continuation)?.representation(),
                MirTypeRepresentationV1::Interface
            )
        {
            return Err(mismatch);
        }
        let result = self
            .identities
            .canonical_key::<_, ExactTypeKey>(lowered.result())?;
        let ExactTypeKey::Nominal(nominal) = result.as_ref() else {
            return Err(mismatch);
        };
        let generated = self
            .identities
            .canonical_key::<_, GeneratedNominalKey>(*nominal)?;
        if !matches!(generated.as_ref(), GeneratedNominalKey::CoroutineStep { result } if *result == source.result())
        {
            return Err(mismatch);
        }
        Ok(())
    }
}
