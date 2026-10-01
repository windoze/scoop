use super::*;
use scoop_identity::{CallableDefinitionOwner, ExactCallableSignature, GeneratedCallableKey};
use scoop_slib::SharedMirCoroutineValidationError as Error;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    core: &hir::CoreBootstrapInterfaceSectionV1,
    foundation: &mir::CanonicalMirFoundation,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let validate = |callables: &mir::CanonicalMirCallableBindingsV1| {
        scoop_slib::validate_shared_mir_coroutines(
            core.compiler_protocols()
                .map(|core| core.coroutine_protocol()),
            source.metadata().identities,
            section.shape_support(),
            callables,
        )
    };
    validate(section.callables()).unwrap_or_else(|error| panic!("{name}: {error}"));
    if name != "base" {
        return;
    }
    let result = section.shape_support().records()[0].exact();
    let start = section
        .callables()
        .entries()
        .iter()
        .find(|binding| {
            matches!(binding.origin(), mir::MirCallableOriginV1::Generated {
            role: GeneratedCallableKey::CoroutineStart { result: actual }, ..
        } if *actual == result)
        })
        .unwrap();
    let remaining = mir::CanonicalMirCallableBindingsV1::try_new(
        section
            .callables()
            .entries()
            .iter()
            .filter(|binding| binding.implementation() != start.implementation())
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(matches!(validate(&remaining), Err(Error::MissingStart(actual)) if actual == result));

    let original = start.semantic_signature().exact();
    let mut parameters = original.parameters().to_vec();
    parameters.swap(0, 1);
    let swapped = ExactCallableSignature::new(
        original.effect(),
        original.receiver().into_option(),
        parameters,
        original.result(),
    );
    let mut changed_foundation = foundation.clone();
    changed_foundation
        .set_callable_signatures(
            foundation
                .callable_signatures()
                .iter()
                .map(|record| {
                    mir::CallableSignatureRecord::new(
                        record.subject(),
                        if CallableDefinitionOwner::try_from(record.subject()).ok()
                            == Some(start.implementation())
                        {
                            swapped.clone()
                        } else {
                            record.signature().clone()
                        },
                    )
                })
                .collect(),
        )
        .unwrap();
    let signature = mir::MirBridgeCallableSignatureV1::new(swapped, mir::GcEffect::Managed);
    let changed = mir::ParamFreeMirCallableBindingV1::try_new(
        mir::MirCallableBridgeAuthority {
            identities: source.metadata().identities,
            foundation: &changed_foundation,
            types: section.types(),
        },
        start.origin().clone(),
        start.implementation(),
        signature.clone(),
        signature,
        mir::MirCallableLoweringRoleV1::CoroutineStart,
    )
    .expect("both swapped parameters remain interfaces of the same result type");
    let callables = mir::CanonicalMirCallableBindingsV1::try_new(
        section
            .callables()
            .entries()
            .iter()
            .map(|binding| {
                if binding.implementation() == start.implementation() {
                    changed.clone()
                } else {
                    binding.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(
        matches!(validate(&callables), Err(Error::StartParameters(actual)) if actual == start.implementation())
    );
}
