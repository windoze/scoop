use super::*;
use scoop_identity::{
    CallableApplicationKey, OdrMemberDiscriminator, OdrMemberKey, SpecializationKey,
};

impl MirCallableBridgeAuthority<'_> {
    pub(super) fn validate_definition_origin(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
    ) -> Result<(), MirCallableBridgeError> {
        match binding.implementation {
            CallableDefinitionOwner::Strong(owner) => {
                if binding.origin.implementation() != Some(owner) {
                    return Err(MirCallableBridgeError::OriginMismatch);
                }
            }
            CallableDefinitionOwner::Odr(member) => {
                let key = self
                    .identities
                    .canonical_key::<_, OdrMemberKey>(member.member())?;
                match (&binding.origin, key.discriminator()) {
                    (
                        MirCallableOriginV1::Application(application),
                        OdrMemberDiscriminator::CallableApplication(actual),
                    ) if application == actual => {
                        let application = self
                            .identities
                            .canonical_key::<_, CallableApplicationKey>(*application)?;
                        let group = self
                            .identities
                            .canonical_key::<_, SpecializationKey>(member.group())?;
                        if !matches!(group.as_ref(), SpecializationKey::Callable { application: expected } if expected == application.as_ref())
                        {
                            return Err(MirCallableBridgeError::OriginMismatch);
                        }
                    }
                    (
                        MirCallableOriginV1::Generated { callable, .. },
                        OdrMemberDiscriminator::GeneratedCallable(actual),
                    ) if callable == actual => {}
                    _ => return Err(MirCallableBridgeError::OriginMismatch),
                }
            }
        }
        self.validate_origin(&binding.origin)
    }
}
