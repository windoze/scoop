use super::*;
use crate::callable_abi::LocalCallableMaterialization;
use scoop_identity::{
    CallableBodyKey, DependencyCallableDeclarationId, PersistentCallableBodyId,
    StrongCallableDefinitionOwner,
};
use scoop_wire::{BudgetMeter, DecodeLimits};

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

    let unit = exact_callable_abi::unit_layout(&output);
    let layout = crate::lower_exact_callable_abi_export(
        &input,
        &output,
        target,
        &signature,
        lir::CallableAbiLayoutInputsV1 {
            receiver: lir::CallableAbiReceiverInputV1::NoReceiver,
            parameters: &[],
            result: &unit,
        },
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
    .unwrap();
    assert_eq!(layout.canonical_signature(), record.abi_signature());
    assert_eq!(layout.definition().symbol(), record.expected_symbol());

    let cycle = output
        .core_lir_bridge()
        .core()
        .unwrap()
        .initialization_cycle_thrower();
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
    let cycle = output.core_lir_bridge().clone();
    let mut module = output.into_module();
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target)).unwrap();
    module
        .functions
        .retain(|function| function.callable_body.id() != body);
    let output = lir::SingleConeStrongLirOutput::try_new(module, Vec::new(), cycle).unwrap();
    assert!(matches!(
        crate::lower_cross_cone_bridge_section(&input, &mir_bridge(&input, target, signature.exact()), &output),
        Err(CrossConeLirBridgeLoweringError::CallableAbi { source: CallableAbiProjectionError::MissingLirBody(actual), .. }) if actual == target
    ));
    let unit = exact_callable_abi::unit_layout(&output);
    assert!(matches!(
        crate::lower_exact_callable_abi_export(&input, &output, target, &signature,
            lir::CallableAbiLayoutInputsV1 { receiver: lir::CallableAbiReceiverInputV1::NoReceiver, parameters: &[], result: &unit },
            &mut BudgetMeter::new(DecodeLimits::default())),
        Err(ExactCallableAbiLoweringError::Materialization(CallableAbiProjectionError::MissingLirBody(actual))) if actual == target
    ));
}

#[test]
fn both_publication_roles_reject_physical_gc_and_argument_drift() {
    for initialization in [false, true] {
        for gc_drift in [false, true] {
            let (input, output, ordinary_target, ordinary_signature) =
                exact_callable_abi::fixture();
            let cycle = output
                .core_lir_bridge()
                .core()
                .unwrap()
                .initialization_cycle_thrower();
            let (target, signature) = if initialization {
                (cycle.target(), cycle.abi_signature().signature().clone())
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
            if initialization {
                assert!(matches!(
                    crate::callable_abi::lower_initialization_abi(
                        &input,
                        &module.functions,
                        &module.enums
                    ),
                    Err(StrongLirLoweringError::CallableAbi(_))
                ));
            }
        }
    }
}

#[test]
fn layout_publication_keeps_the_shared_materialization_search_metered() {
    let (input, output, target, signature) = exact_callable_abi::fixture();
    let unit = exact_callable_abi::unit_layout(&output);
    let mut meter = BudgetMeter::new(DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    });
    let error = crate::lower_exact_callable_abi_export(
        &input,
        &output,
        target,
        &signature,
        lir::CallableAbiLayoutInputsV1 {
            receiver: lir::CallableAbiReceiverInputV1::NoReceiver,
            parameters: &[],
            result: &unit,
        },
        &mut meter,
    )
    .unwrap_err();
    assert!(matches!(error, ExactCallableAbiLoweringError::Resource(_)));
}

fn mir_bridge(
    input: &mir::SingleConeStrongMirInput,
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
