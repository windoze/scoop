use super::*;
use scoop_identity::StrongCallableDefinitionOwner;
use scoop_wire::{BudgetMeter, DecodeLimits};

pub(super) fn fixture() -> (
    mir::SingleConeStrongMirInput,
    lir::SingleConeStrongLirOutput,
    StrongCallableDefinitionOwner,
    mir::MirBridgeCallableSignatureV1,
) {
    let mut builder = Builder::new();
    let function = builder.user_fn("exportedAbi", Arena::new(), Vec::new());
    let cycle = builder.user_fn("__scoopThrowInitializationCycle", Arena::new(), Vec::new());
    let mut module = builder.finish_with_types(function, ConeIdentity::CORE, vec![]);
    module.output = mir::MirOutput::Library;
    let target = match module.meta.callable_signature_subject(function).unwrap() {
        mir::CallableSignatureSubject::Strong(owner) => {
            StrongCallableDefinitionOwner::Function(match owner {
                scoop_identity::CallableOwner::Function(id) => id,
                _ => panic!("source function"),
            })
        }
        _ => panic!("strong function"),
    };
    let cycle_owner = match module.meta.callable_signature_subject(cycle).unwrap() {
        mir::CallableSignatureSubject::Strong(scoop_identity::CallableOwner::Function(id)) => id,
        _ => panic!("source cycle function"),
    };
    let foundation = mir::OdrFreeMirFoundation::from_module(&module).unwrap();
    let strong = mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation);
    let signature = mir::MirBridgeCallableSignatureV1::new(
        strong
            .bridges()
            .iter()
            .find(|entry| entry.implementation() == target.callable_owner())
            .unwrap()
            .signature()
            .clone(),
        module.functions[function].gc_effect,
    );
    let production = mir::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        mir::EntryMirBridgeBranchV1::Library,
        strong.with_initialization_cycle(cycle_owner).unwrap(),
    )
    .unwrap();
    let input = mir::SingleConeStrongMirInput::try_new(
        module,
        foundation,
        production,
        Vec::new(),
        mir::StrongExternalCallableInput::Unused,
    )
    .unwrap();
    let output = crate::lower(
        &input,
        RuntimeStringDescriptor::Local,
        &lir::SelectedExternalLirSet::empty(input.module().cone),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    (input, output, target, signature)
}

pub(super) fn unit_layout(output: &lir::SingleConeStrongLirOutput) -> lir::ExactLayoutExportV1 {
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let identity = lir::ExactLayoutIdentityV1::from_foundation(
        output.module().meta.target_profile,
        exact,
        scoop_identity::RepresentationRole::ManagedValue,
        output.foundation(),
        &mut meter,
    )
    .unwrap();
    lir::ExactValueLayoutV1::unit(identity, output.foundation(), &mut meter)
        .unwrap()
        .into()
}

#[test]
fn callable_abi_producer_binds_real_materialization_and_emitted_unit_signature() {
    let (input, output, target, signature) = fixture();
    let unit = unit_layout(&output);
    let result = lower_exact_callable_abi_export(
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
    assert_eq!(result.target(), target);
    assert_eq!(result.canonical_signature().signature(), signature.exact());
    assert_eq!(
        result.canonical_signature().result(),
        scoop_identity::ScoopAbiReturn::UnitVoid
    );
    assert_eq!(
        result.layout_dependencies().result().identity().layout(),
        unit.identity().layout()
    );
    assert!(
        output
            .module()
            .functions
            .iter()
            .any(|body| body.callable_body.id() == result.definition().semantic_id())
    );
}

#[test]
fn callable_abi_producer_rejects_signature_and_effect_before_export() {
    let (input, output, target, signature) = fixture();
    let unit = unit_layout(&output);
    let wrong_effect = match signature.gc_effect() {
        mir::GcEffect::Managed => mir::GcEffect::NoGc,
        mir::GcEffect::NoGc => mir::GcEffect::Managed,
    };
    let wrong = mir::MirBridgeCallableSignatureV1::new(signature.exact().clone(), wrong_effect);
    assert!(matches!(
        lower_exact_callable_abi_export(
            &input,
            &output,
            target,
            &wrong,
            lir::CallableAbiLayoutInputsV1 {
                receiver: lir::CallableAbiReceiverInputV1::NoReceiver,
                parameters: &[],
                result: &unit
            },
            &mut BudgetMeter::new(DecodeLimits::default())
        ),
        Err(ExactCallableAbiLoweringError::GcEffect)
    ));
    let wrong = mir::MirBridgeCallableSignatureV1::new(
        ExactCallableSignature::new(
            Effect::Ordinary,
            Some(unit.identity().exact()),
            vec![],
            unit.identity().exact(),
        ),
        signature.gc_effect(),
    );
    assert!(matches!(
        lower_exact_callable_abi_export(
            &input,
            &output,
            target,
            &wrong,
            lir::CallableAbiLayoutInputsV1 {
                receiver: lir::CallableAbiReceiverInputV1::NoReceiver,
                parameters: &[],
                result: &unit
            },
            &mut BudgetMeter::new(DecodeLimits::default())
        ),
        Err(ExactCallableAbiLoweringError::Materialization(CallableAbiProjectionError::MirSignature(actual))) if actual == target
    ));
}
