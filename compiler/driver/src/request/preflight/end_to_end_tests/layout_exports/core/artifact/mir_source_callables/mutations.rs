use super::*;

impl Replay<'_> {
    pub(super) fn binding(
        &self,
        original: &mir::ParamFreeMirCallableBindingV1,
        signature: mir::MirBridgeCallableSignatureV1,
        role: mir::MirCallableLoweringRoleV1,
    ) -> mir::ParamFreeMirCallableBindingV1 {
        let foundation = self.changed_foundation(
            original.implementation().strong_owner().unwrap(),
            signature.exact(),
        );
        mir::ParamFreeMirCallableBindingV1::try_new(
            mir::MirCallableBridgeAuthority {
                identities: self.source.metadata().identities,
                foundation: &foundation,
                types: self.section.types(),
            },
            original.origin().clone(),
            original.implementation(),
            signature.clone(),
            signature,
            role,
        )
        .expect("changed machine metadata must pass MIR structural and identity checks")
    }

    pub(super) fn changed_foundation(
        &self,
        implementation: scoop_identity::StrongCallableDefinitionOwner,
        signature: &ExactCallableSignature,
    ) -> mir::OdrFreeMirFoundation {
        let surface = mir::StrongCallableBridgeSurfaceV1::from_foundation(self.foundation);
        let mut foundation = self.foundation.clone();
        foundation
            .set_callable_signatures(
                surface
                    .bridges()
                    .iter()
                    .map(|bridge| {
                        mir::CallableSignatureRecord::new(
                            bridge.subject(),
                            if bridge.implementation() == implementation.callable_owner() {
                                signature.clone()
                            } else {
                                bridge.signature().clone()
                            },
                        )
                    })
                    .collect(),
            )
            .unwrap();
        mir::OdrFreeMirFoundation::try_new(foundation).unwrap()
    }

    pub(super) fn replace(
        &self,
        changed: mir::ParamFreeMirCallableBindingV1,
    ) -> Vec<mir::ParamFreeMirCallableBindingV1> {
        self.section
            .callables()
            .entries()
            .iter()
            .map(|binding| {
                if binding.implementation() == changed.implementation() {
                    changed.clone()
                } else {
                    binding.clone()
                }
            })
            .collect()
    }

    pub(super) fn source_binding(
        &self,
        declaration: Declaration,
        signature: ExactCallableSignature,
    ) -> mir::ParamFreeMirCallableBindingV1 {
        let (origin, role) = match declaration {
            Declaration::Function(id) => (
                mir::MirCallableOriginV1::Function(id),
                mir::MirCallableLoweringRoleV1::Ordinary,
            ),
            Declaration::PropertyAccessor(id) => (
                mir::MirCallableOriginV1::Accessor(id),
                mir::MirCallableLoweringRoleV1::Accessor,
            ),
        };
        let signature = mir::MirBridgeCallableSignatureV1::new(signature, mir::GcEffect::Managed);
        mir::ParamFreeMirCallableBindingV1::try_new(
            mir::MirCallableBridgeAuthority {
                identities: self.source.metadata().identities,
                foundation: self.foundation,
                types: self.section.types(),
            },
            origin,
            declaration.implementation(),
            signature.clone(),
            signature,
            role,
        )
        .unwrap()
    }
}
