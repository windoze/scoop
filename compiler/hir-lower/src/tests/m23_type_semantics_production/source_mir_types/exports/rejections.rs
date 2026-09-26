use super::*;

pub(super) fn check(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
) {
    missing_ordinary_source(input, dependencies);
    assert!(produce(input, &[]).is_err());
    assert!(matches!(
        produce(input, &[dependencies, dependencies]),
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
            &[dependencies]
        ),
        Err(Error::ProviderMismatch)
    ));

    produce(input, &[dependencies]).unwrap();
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
            scoop_mir::GcEffect::Managed,
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
            &[dependencies]
        ),
        Err(Error::IncompleteOrdinaryCallables {
            expected: 1,
            actual: 2
        })
    ));
}
