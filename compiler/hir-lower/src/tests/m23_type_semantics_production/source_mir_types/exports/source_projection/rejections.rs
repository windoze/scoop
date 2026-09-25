use super::*;
use mir::MirTypeBridgeSourceInventoryV1 as Inventory;
use mir::MirTypeBridgeSourceJoinError as JoinError;

pub(super) fn inventories(
    input: MirTypeBridgeExportInputV1<'_>,
    expected: &mir::MirTypeBridgeExportConstituentsV1,
    source: &MirTypeBridgeSourceProjectionV1,
) {
    for missing in [
        Inventory::Types,
        Inventory::Callables,
        Inventory::Dispatch,
        Inventory::Objects,
    ] {
        let types = if missing == Inventory::Types {
            mir::CanonicalParamFreeMirTypeExportsV1::try_new(vec![]).unwrap()
        } else {
            expected.types().clone()
        };
        let callables = if missing == Inventory::Callables {
            mir::CanonicalMirCallableBindingsV1::try_new(vec![]).unwrap()
        } else {
            expected.callables().clone()
        };
        let dispatch = if missing == Inventory::Dispatch {
            mir::CanonicalMirDispatchSchemasV1::try_new(
                mir::MirDispatchSchemaAuthority {
                    identities: input.identities,
                    types: expected.types(),
                    callables: expected.callables(),
                },
                vec![],
            )
            .unwrap()
        } else {
            expected.dispatch().clone()
        };
        let objects = if missing == Inventory::Objects {
            mir::CanonicalMirObjectValuesV1::try_new(vec![]).unwrap()
        } else {
            expected.objects().clone()
        };
        let candidate = mir::MirTypeBridgeExportConstituentsV1::new(
            types,
            callables,
            dispatch,
            objects,
            expected.shapes().clone(),
            expected.initialization_uses().clone(),
        );
        assert!(
            matches!(
                candidate.validate_sources(input.mir.module().cone, input.identities, source),
                Err(JoinError::Inventory(actual)) if actual == missing
            ),
            "missing {missing:?} must be rejected from independent source inventory"
        );
    }
    let shapes = mir::CanonicalMirShapeSupportsV1::try_new(
        input.mir.module().cone,
        mir::MirShapeSupportAuthority {
            identities: input.identities,
            types: expected.types(),
        },
        vec![],
    )
    .unwrap();
    let candidate = mir::MirTypeBridgeExportConstituentsV1::new(
        expected.types().clone(),
        expected.callables().clone(),
        expected.dispatch().clone(),
        expected.objects().clone(),
        shapes,
        expected.initialization_uses().clone(),
    );
    assert!(matches!(
        candidate.validate_sources(input.mir.module().cone, input.identities, source),
        Err(JoinError::Shape(
            mir::MirShapeSupportError::MissingSource { .. }
        ))
    ));
}
