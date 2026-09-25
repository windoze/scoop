use super::*;
use mir::{MirCallableLoweringRoleV1 as Role, MirTypeBridgeTypeIndexV1};
use std::fmt::Write;

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

pub(super) fn actual(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
    exports: &mir::MirTypeBridgeExportConstituentsV1,
) {
    let roots = input.mir.materialization().callable_roots();
    for record in exports.callables().entries() {
        let root = roots
            .iter()
            .find(|root| root.implementation() == record.implementation().callable_owner())
            .unwrap();
        assert_eq!(
            record.lowered_signature().exact(),
            input
                .mir
                .module()
                .meta
                .callable_signatures
                .get(mir::CallableSignatureSubject::Strong(root.implementation()))
                .unwrap()
                .signature(),
        );
        assert_eq!(
            record.lowered_signature().gc_effect(),
            input.mir.module().functions[root.function()].gc_effect
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
            .entries()
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

pub(super) fn dump(
    input: MirTypeBridgeExportInputV1<'_>,
    exports: &mir::MirTypeBridgeExportConstituentsV1,
) -> String {
    let mut text = format!(
        "types={} callables={} dispatch={} objects={} shapes={} initialization_uses={} ordinary={}\n",
        exports.types().records().len(),
        exports.callables().entries().len(),
        exports.dispatch().records().len(),
        exports.objects().records().len(),
        exports.shapes().records().len(),
        exports.initialization_uses().records().len(),
        input.ordinary.exports().len(),
    );
    for (name, exact) in source_dispatch::owners(input.hir) {
        let record = exports.types().get(exact).unwrap();
        let schema = exports.dispatch().get(exact).unwrap();
        writeln!(
            text,
            "type {name}: {:?}/{:?} vslots={} itables={}",
            record.facts().kind(),
            record.facts().gc(),
            schema.vtable().entries().len(),
            schema.itables().len()
        )
        .unwrap();
    }
    let mut callables = Vec::new();
    for record in exports.callables().entries() {
        let root = input
            .mir
            .materialization()
            .callable_roots()
            .iter()
            .find(|root| root.implementation() == record.implementation().callable_owner())
            .unwrap();
        let name = &input.mir.module().functions[root.function()].name;
        let role = match record.lowering_role() {
            Role::Ordinary => "ordinary",
            Role::Accessor => "accessor",
            Role::PureVirtualTrap { .. } => "trap",
            Role::ClassInitializer { .. } => "class-initializer",
            Role::PrimaryValueConstructor { .. } => "primary-value-constructor",
            Role::ValueConstructor { .. } => "value-constructor",
            Role::DispatchAdjust { .. } => "dispatch-adjust",
            Role::BoxingAdjust { .. } => "boxing-adjust",
            Role::DerivedEquality { .. } => "derived-equality",
            Role::ObjectInitializer { .. } => "object-initializer",
            Role::ObjectEnsure { .. } => "object-ensure",
        };
        callables.push(format!(
            "callable {name}: {role} receiver={} parameters={} {:?}\n",
            record.lowered_signature().exact().receiver().is_present(),
            record.lowered_signature().exact().parameters().len(),
            record.lowered_signature().gc_effect()
        ));
    }
    callables.sort();
    text.extend(callables);
    text
}
