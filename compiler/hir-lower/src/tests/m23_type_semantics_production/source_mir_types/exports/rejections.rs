use super::*;

pub(super) fn check(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
) {
    missing_ordinary_source(input, dependencies);
    assert!(produce(input, &[], &mut meter()).is_err());
    assert!(matches!(
        produce(input, &[dependencies, dependencies], &mut meter()),
        Err(Error::Lookup(
            mir::MirTypeBridgeLookupError::DuplicateType { .. }
        ))
    ));
    let wrong = mir::CrossConeMirBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        input.mir.foundation(),
        vec![],
        vec![],
    )
    .unwrap();
    assert!(matches!(
        produce(
            MirTypeBridgeExportInputV1 {
                ordinary: &wrong,
                ..input
            },
            &[dependencies],
            &mut meter()
        ),
        Err(Error::ProviderMismatch)
    ));
    let mut measured = meter();
    produce(input, &[dependencies], &mut measured).unwrap();
    let usage = measured.usage();
    assert!(usage.validation_work_units > 0 && usage.owned_bytes > 0);
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    produce(input, &[dependencies], &mut shared).unwrap();
    assert!(produce(input, &[dependencies], &mut shared).is_err());
    for limits in [
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(produce(input, &[dependencies], &mut BudgetMeter::new(limits)).is_err());
    }
}

fn missing_ordinary_source(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
) {
    let root = input
        .mir
        .materialization()
        .callable_roots()
        .iter()
        .find(|root| input.mir.module().functions[root.function()].name == "hiddenReady")
        .unwrap();
    let scoop_identity::CallableOwner::Function(function) = root.implementation() else {
        panic!("source function")
    };
    let signature = input
        .mir
        .module()
        .meta
        .callable_signatures
        .get(mir::CallableSignatureSubject::Strong(root.implementation()))
        .unwrap()
        .signature()
        .clone();
    let mut ordinary = input.ordinary.exports().to_vec();
    ordinary.push(
        mir::ParamFreeMirCallableExportV1::try_new(
            scoop_identity::DependencyCallableDeclarationId::Function(function),
            scoop_identity::StrongCallableDefinitionOwner::Function(function),
            signature,
        )
        .unwrap(),
    );
    let ordinary = mir::CrossConeMirBridgeSectionV1::try_new(
        input.mir.module().cone,
        input.mir.foundation(),
        ordinary,
        vec![],
    )
    .unwrap();
    assert!(matches!(
        produce(
            MirTypeBridgeExportInputV1 {
                ordinary: &ordinary,
                ..input
            },
            &[dependencies],
            &mut meter()
        ),
        Err(Error::IncompleteOrdinaryCallables {
            expected: 1,
            actual: 2
        })
    ));
}
