use super::*;
use mir::{MirCallableLoweringRoleV1 as Role, MirTypeBridgeTypeIndexV1};

pub(super) fn bytes(exports: &mir::MirTypeBridgeExportConstituentsV1) -> [Vec<u8>; 6] {
    [
        encode(exports.types()).unwrap(),
        encode(exports.callables()).unwrap(),
        encode(exports.dispatch()).unwrap(),
        encode(exports.objects()).unwrap(),
        encode(exports.shapes()).unwrap(),
        encode(exports.initialization_uses()).unwrap(),
    ]
}

fn callable(
    input: &mir::ConeMirInput,
    binding: &mir::ParamFreeMirCallableBindingV1,
) -> (mir::CallableSignatureSubject, mir::FunctionId) {
    input
        .materialization()
        .callable_roots()
        .iter()
        .map(|root| (root.subject(), root.function()))
        .chain(input.module().meta.coroutine_starts.iter().map(|start| {
            (
                start.identity().signature_record().subject(),
                start.function(),
            )
        }))
        .find(|(subject, _)| *subject == binding.implementation().into())
        .unwrap()
}

pub(super) fn actual(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
    exports: &mir::MirTypeBridgeExportConstituentsV1,
) {
    for record in exports.callables().entries() {
        let (subject, function) = callable(input.mir, record);
        assert_eq!(
            record.lowered_signature().exact(),
            input
                .mir
                .module()
                .meta
                .callable_signatures
                .get(subject)
                .unwrap()
                .signature()
        );
        assert_eq!(
            record.lowered_signature().gc_effect(),
            input.mir.module().functions[function].gc_effect
        );
    }
    assert_eq!(input.ordinary.exports().len(), 1);
    assert!(
        input
            .ordinary
            .exports()
            .iter()
            .all(|record| { exports.callables().get(record.implementation()).is_none() })
    );
    assert!(
        dependencies
            .records()
            .iter()
            .all(|record| exports.types().get(record.exact()).is_none())
    );
    assert_eq!(
        exports.shapes().records().len(),
        input.mir.materialization().shape_support().len()
    );
    for schema in exports.dispatch().records() {
        assert!(exports.types().get(schema.owner()).is_some());
        for entry in schema
            .vtable()
            .iter()
            .chain(schema.itables().iter().flat_map(|table| table.entries()))
        {
            assert!(
                exports
                    .callables()
                    .get(entry.implementation().target())
                    .is_some()
            );
        }
    }
    for object in exports.objects().records() {
        let ensure = exports.callables().get(object.ensure()).unwrap();
        assert!(
            matches!(ensure.lowering_role(), Role::ObjectEnsure { unit } if *unit == object.unit())
        );
    }
    assert!(exports.initialization_uses().records().is_empty());
}

pub(super) fn roundtrip(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
    exports: &mir::MirTypeBridgeExportConstituentsV1,
) {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(input.identities)
        .unwrap();
    let mut graph = pending.finish().unwrap();
    let types: mir::DecodedCanonicalParamFreeMirTypeExportsV1 = decoded(exports.types());
    let types = types.validate(&mut graph, input.mir.foundation()).unwrap();
    assert_eq!(&types, exports.types());
    let index = MirTypeBridgeTypeIndexV1::try_new(&[&types, dependencies]).unwrap();
    let callables: mir::DecodedCanonicalMirCallableBindingsV1 = decoded(exports.callables());
    let callables = callables
        .validate(&mut graph, input.mir.foundation(), &index)
        .unwrap();
    assert_eq!(&callables, exports.callables());
    let dispatch: mir::DecodedCanonicalMirDispatchSchemasV1 = decoded(exports.dispatch());
    assert_eq!(
        dispatch.validate(&mut graph, &index, &callables).unwrap(),
        *exports.dispatch()
    );
    let objects: mir::DecodedCanonicalMirObjectValuesV1 = decoded(exports.objects());
    assert_eq!(
        objects.validate(&mut graph, &index, &callables).unwrap(),
        *exports.objects()
    );
    let shapes: mir::DecodedCanonicalMirShapeSupportsV1 = decoded(exports.shapes());
    assert_eq!(
        shapes
            .validate(input.mir.module().cone, &mut graph, &types)
            .unwrap(),
        *exports.shapes()
    );
    let uses: mir::DecodedCanonicalMirExternalInitializationUsesV1 =
        decoded(exports.initialization_uses());
    assert_eq!(
        uses.validate(input.mir.module().cone, &mut graph).unwrap(),
        *exports.initialization_uses()
    );
}
