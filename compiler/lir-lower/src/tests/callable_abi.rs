use super::*;
use crate::callable_abi::LocalCallableMaterialization;
use scoop_identity::{
    CallableBodyKey, DependencyCallableDeclarationId, PersistentCallableBodyId,
    StrongCallableDefinitionOwner,
};

#[test]
fn ordinary_initialization_and_layout_publication_share_the_materialized_abi() {
    let (input, output, target, signature) = exact_callable_abi::fixture();
    let ordinary = crate::lower_cross_cone_bridge_section(
        &input,
        &mir_bridge(&input, target, signature.exact()),
        &output,
    )
    .unwrap();
    let materialized = LocalCallableMaterialization::resolve(
        &input,
        &output.module().functions,
        target,
        signature.exact(),
    )
    .unwrap();
    let record = materialized.abi_record(&output.module().enums).unwrap();
    assert_eq!(ordinary.exports()[0].callable_abi(), &record);

    let layout =
        crate::lower_exact_callable_abi_export(&input, &output, target, &signature).unwrap();
    assert_eq!(layout.canonical_signature(), record.abi_signature());
    assert_eq!(layout.definition().symbol(), record.expected_symbol());

    let source = input
        .production()
        .strong_callable_bridges()
        .initialization_cycle()
        .unwrap();
    let scoop_identity::CallableOwner::Function(function) = source.implementation() else {
        panic!("source function");
    };
    let target = StrongCallableDefinitionOwner::Function(function);
    let common = crate::lower_cross_cone_bridge_section(
        &input,
        &mir_bridge(&input, target, source.signature()),
        &output,
    )
    .unwrap();
    let cycle = common.exports()[0].callable_abi();
    let materialized = LocalCallableMaterialization::resolve(
        &input,
        &output.module().functions,
        cycle.target(),
        cycle.abi_signature().signature(),
    )
    .unwrap();
    assert_eq!(
        materialized.abi_record(&output.module().enums).unwrap(),
        *cycle
    );
}

#[test]
fn ordinary_and_layout_publication_reject_a_missing_materialized_body_with_the_same_target() {
    let (input, output, target, signature) = exact_callable_abi::fixture();
    let mut module = output.into_module();
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target)).unwrap();
    module
        .functions
        .retain(|function| function.callable_body.id() != body);
    let output = lir::SingleConeStrongLirOutput::try_new(module, Vec::new()).unwrap();
    assert!(matches!(
        crate::lower_cross_cone_bridge_section(&input, &mir_bridge(&input, target, signature.exact()), &output),
        Err(CrossConeLirBridgeLoweringError::CallableAbi { source: CallableAbiProjectionError::MissingLirBody(actual), .. }) if actual == target
    ));
    assert!(matches!(
        crate::lower_exact_callable_abi_export(&input, &output, target, &signature),
        Err(ExactCallableAbiLoweringError::Materialization(CallableAbiProjectionError::MissingLirBody(actual))) if actual == target
    ));
}

#[test]
fn both_publication_roles_reject_physical_gc_and_argument_drift() {
    for initialization in [false, true] {
        for gc_drift in [false, true] {
            let (input, output, ordinary_target, ordinary_signature) =
                exact_callable_abi::fixture();
            let source = input
                .production()
                .strong_callable_bridges()
                .initialization_cycle()
                .unwrap();
            let scoop_identity::CallableOwner::Function(function) = source.implementation() else {
                panic!("source function");
            };
            let (target, signature) = if initialization {
                (
                    StrongCallableDefinitionOwner::Function(function),
                    source.signature().clone(),
                )
            } else {
                (ordinary_target, ordinary_signature.exact().clone())
            };
            let body =
                PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target)).unwrap();
            let mut module = output.into_module();
            let function = module
                .functions
                .iter_mut()
                .find(|function| function.callable_body.id() == body)
                .unwrap();
            if gc_drift {
                function.gc_effect = lir::GcEffect::NoGc;
            } else {
                function.signature = lir::ScoopAbiSignature::new(
                    vec![lir::AbiArgument::ElidedZst(
                        lir::AbiZst::new(
                            lir::LirType::Aggregate(Vec::new()),
                            lir::AbiZeroSizedLayout::new(1).unwrap(),
                        )
                        .unwrap(),
                    )],
                    lir::AbiReturn::UnitVoid,
                    lir::CallingConvention::Cdecl,
                );
            }
            let error = LocalCallableMaterialization::resolve(
                &input,
                &module.functions,
                target,
                &signature,
            )
            .unwrap()
            .abi_record(&module.enums)
            .unwrap_err();
            if gc_drift {
                assert!(
                    matches!(error, CallableAbiProjectionError::GcEffect(actual) if actual == target)
                );
            } else {
                assert!(
                    matches!(error, CallableAbiProjectionError::ArgumentCount { target: actual, mir: 0, lir: 1 } if actual == target)
                );
            }
        }
    }
}

fn mir_bridge(
    input: &mir::ConeMirInput,
    target: StrongCallableDefinitionOwner,
    signature: &ExactCallableSignature,
) -> mir::CrossConeMirBridgeSectionV1 {
    let StrongCallableDefinitionOwner::Function(function) = target else {
        panic!("fixture exports a source function")
    };
    let export = mir::ParamFreeMirCallableExportV1::try_new(
        DependencyCallableDeclarationId::Function(function),
        target,
        signature.clone(),
        input.module().functions[input
            .materialization()
            .callable_roots()
            .iter()
            .find(|root| {
                root.subject()
                    == scoop_mir::CallableSignatureSubject::Strong(target.callable_owner())
            })
            .unwrap()
            .function()]
        .gc_effect,
    )
    .unwrap();
    mir::CrossConeMirBridgeSectionV1::try_new(
        input.module().cone,
        input.foundation(),
        vec![export],
        Vec::new(),
    )
    .unwrap()
}
