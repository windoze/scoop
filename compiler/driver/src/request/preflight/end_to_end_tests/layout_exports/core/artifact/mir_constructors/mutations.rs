use super::*;
use mir::{GcEffect, MirBridgeCallableSignatureV1 as Signature, MirCallableLoweringRoleV1 as Role};

pub(super) fn check(replay: &Replay<'_>) {
    let class = replay
        .fixture_bindings()
        .find(|binding| {
            matches!(binding.lowering_role(), Role::ClassInitializer { .. })
                && binding.semantic_signature().exact().parameters().len() == 1
        })
        .unwrap();
    for (parameters, expected) in [
        (
            vec![class.semantic_signature().exact().result()],
            Component::Parameter { index: 0 },
        ),
        (vec![], Component::ParameterCount),
    ] {
        let semantic = class.semantic_signature().exact();
        let lowered = class.lowered_signature().exact();
        let changed = replay.binding(
            class,
            Signature::new(
                ExactCallableSignature::new(
                    semantic.effect(),
                    None,
                    parameters.clone(),
                    semantic.result(),
                ),
                GcEffect::Managed,
            ),
            Signature::new(
                ExactCallableSignature::new(
                    lowered.effect(),
                    lowered.receiver().into_option(),
                    parameters,
                    lowered.result(),
                ),
                GcEffect::Managed,
            ),
            *class.lowering_role(),
        );
        component(
            replay.reject(replay.replace(changed)),
            expected,
            declaration(class),
        );
    }
    let primary = replay
        .fixture_bindings()
        .find(|binding| {
            matches!(
                binding.lowering_role(),
                Role::PrimaryValueConstructor { .. }
            )
        })
        .unwrap();
    let changed = replay.binding(
        primary,
        primary.semantic_signature().clone(),
        Signature::new(
            primary.lowered_signature().exact().clone(),
            GcEffect::Managed,
        ),
        Role::ValueConstructor {
            owner: primary.semantic_signature().exact().result(),
        },
    );
    component(
        replay.reject(replay.replace(changed)),
        Component::LoweringRole,
        declaration(primary),
    );
    let secondary = replay
        .fixture_bindings()
        .find(|binding| {
            matches!(binding.lowering_role(), Role::ValueConstructor { .. })
                && binding.semantic_signature().gc_effect() == GcEffect::NoGc
        })
        .unwrap();
    let signature = Signature::new(
        secondary.semantic_signature().exact().clone(),
        GcEffect::Managed,
    );
    let changed = replay.binding(
        secondary,
        signature.clone(),
        signature,
        *secondary.lowering_role(),
    );
    component(
        replay.reject(replay.replace(changed)),
        Component::GcEffect,
        declaration(secondary),
    );
}

impl Replay<'_> {
    fn binding(
        &self,
        original: &mir::ParamFreeMirCallableBindingV1,
        semantic: Signature,
        lowered: Signature,
        role: Role,
    ) -> mir::ParamFreeMirCallableBindingV1 {
        let mut foundation = self.foundation.clone();
        foundation
            .set_callable_signatures(
                self.foundation
                    .callable_signatures()
                    .iter()
                    .map(|bridge| {
                        mir::CallableSignatureRecord::new(
                            bridge.subject(),
                            if bridge.subject() == original.implementation().into() {
                                lowered.exact().clone()
                            } else {
                                bridge.signature().clone()
                            },
                        )
                    })
                    .collect(),
            )
            .unwrap();
        mir::ParamFreeMirCallableBindingV1::try_new(
            mir::MirCallableBridgeAuthority {
                identities: self.source.metadata().identities,
                foundation: &foundation,
                types: self.section.types(),
            },
            original.origin().clone(),
            original.implementation(),
            semantic,
            lowered,
            role,
        )
        .expect("changed constructor metadata passes MIR structure and identity validation")
    }
}
