use super::*;
use scoop_identity::{ScoopAbiArgument, ScoopAbiReturn};
use scoop_lir_lower::LayoutAbiExportInputV1;

pub(super) fn actual(
    input: LayoutAbiExportInputV1<'_>,
    result: &lir::LayoutAbiExportConstituentsV1,
) {
    assert_eq!(
        result.descriptors().records().len(),
        input.lir.module().meta.type_descriptors.len(),
        "descriptors without MIR exports: {:?}",
        input
            .lir
            .module()
            .meta
            .type_descriptors
            .iter()
            .filter(|(_, descriptor)| input
                .bridge
                .types()
                .get(descriptor.identity.exact_type())
                .is_none())
            .map(|(_, descriptor)| &descriptor.diagnostic_name)
            .collect::<Vec<_>>()
    );
    contents(input, result);
    zero_sized_abi(result);
}

pub(super) fn contents(
    input: LayoutAbiExportInputV1<'_>,
    result: &lir::LayoutAbiExportConstituentsV1,
) {
    assert_eq!(result.provider(), input.mir.module().cone);
    assert_eq!(
        result.callables().records().len(),
        input.bridge.callables().entries().len()
    );
    assert_eq!(
        result.shape_support().records().len(),
        input.lir.shape_support().roots().len()
    );
    assert!(!result.layouts().records().is_empty());
    assert!(!result.dispatch().records().is_empty());
    for source in input.mir.module().meta.source_exact_types.iter() {
        if let mir::SourceExactTypeOwner::Cone(provider) = source.owner()
            && provider != result.provider()
        {
            assert!(
                input
                    .bridge
                    .types()
                    .get(source.identity_record().id())
                    .is_none(),
                "foreign source types remain dependency exports"
            );
            assert!(
                input
                    .mir
                    .materialization()
                    .source_nominal_shape(source.ty())
                    .is_none()
            );
            assert!(
                result
                    .layouts()
                    .records()
                    .iter()
                    .all(|layout| layout.identity().exact() != source.identity_record().id())
            );
        }
    }
}

pub(super) fn zero_sized_abi(result: &lir::LayoutAbiExportConstituentsV1) {
    let mut elided_inputs = 0;
    let mut elided_results = 0;
    for signature in result
        .callables()
        .records()
        .iter()
        .map(|callable| callable.canonical_signature())
        .chain(
            result
                .direct_callables()
                .exports()
                .iter()
                .map(|callable| callable.abi_signature()),
        )
    {
        elided_inputs += signature
            .arguments()
            .iter()
            .filter(|argument| matches!(argument, ScoopAbiArgument::ElidedZst(_)))
            .count();
        elided_results += usize::from(matches!(signature.result(), ScoopAbiReturn::ElidedZst(_)));
    }
    assert!(
        elided_inputs > 0 && elided_results > 0,
        "elided inputs={elided_inputs} results={elided_results}",
    );
}

pub(super) fn bytes(result: &lir::LayoutAbiExportConstituentsV1) -> [Vec<u8>; 5] {
    [
        encode(result.layouts()).unwrap(),
        encode(result.descriptors()).unwrap(),
        encode(result.dispatch()).unwrap(),
        encode(result.callables()).unwrap(),
        encode(result.shape_support()).unwrap(),
    ]
}

pub(super) fn callable_name(
    input: LayoutAbiExportInputV1<'_>,
    target: scoop_identity::StrongCallableDefinitionOwner,
) -> &str {
    let root = input
        .mir
        .materialization()
        .callable_roots()
        .iter()
        .find(|root| {
            root.subject() == scoop_mir::CallableSignatureSubject::Strong(target.callable_owner())
        })
        .unwrap();
    &input.mir.module().functions[root.function()].name
}

pub(super) fn dump(
    input: LayoutAbiExportInputV1<'_>,
    result: &lir::LayoutAbiExportConstituentsV1,
) -> String {
    let mut text = format!(
        "layouts={} descriptors={} dispatch={} callables={} shapes={}\n",
        result.layouts().records().len(),
        result.descriptors().records().len(),
        result.dispatch().records().len(),
        result.callables().records().len(),
        result.shape_support().records().len()
    );
    let mut lines = Vec::new();
    for (_, layout) in input.lir.module().meta.layouts.iter() {
        lines.push(format!(
            "layout {} [{:?}]: size={} align={} fields={:?} scan={:?}\n",
            layout.name,
            layout.identity.layout_record().key().representation(),
            layout.size,
            layout.align,
            layout
                .fields
                .iter()
                .map(|field| (field.offset, field.access_align))
                .collect::<Vec<_>>(),
            layout.kind
        ));
    }
    for descriptor in result.descriptors().records() {
        lines.push(format!(
            "descriptor {}: {:?} scan={:?} vslots={} itables={}\n",
            descriptor.diagnostic_name(),
            descriptor.shape(),
            descriptor.object_scan(),
            result
                .dispatch()
                .get(descriptor.dispatch().vtable())
                .unwrap()
                .entries()
                .len(),
            descriptor.dispatch().itables().len()
        ));
    }
    for table in result.dispatch().records() {
        let owner = result.descriptors().get(table.owner_exact()).unwrap();
        lines.push(format!(
            "dispatch {} [{:?}]: {:?}\n",
            owner.diagnostic_name(),
            table.role(),
            table
                .entries()
                .iter()
                .map(|entry| (
                    entry.position().into_u32(),
                    callable_name(input, entry.implementation().target()),
                    entry.implementation().receiver_adaptation()
                ))
                .collect::<Vec<_>>()
        ));
    }
    for callable in result.callables().records() {
        let signature = callable.canonical_signature();
        let arguments = signature
            .arguments()
            .iter()
            .map(|argument| {
                let kind = match argument {
                    ScoopAbiArgument::ElidedZst(_) => "Zst",
                    ScoopAbiArgument::Direct(_) => "Direct",
                    ScoopAbiArgument::Indirect(_) => "Indirect",
                };
                format!("{kind}({})", argument.storage().byte_size())
            })
            .collect::<Vec<_>>();
        let returns = match signature.result() {
            ScoopAbiReturn::UnitVoid => "UnitVoid".to_string(),
            ScoopAbiReturn::ElidedZst(storage) => format!("Zst({})", storage.byte_size()),
            ScoopAbiReturn::Direct(storage) => format!("Direct({})", storage.byte_size()),
            ScoopAbiReturn::Indirect(storage) => format!("Indirect({})", storage.byte_size()),
        };
        lines.push(format!(
            "callable {}: {:?} receiver={} args={arguments:?} result={returns}\n",
            callable_name(input, callable.target()),
            callable.call_protocol(),
            signature.signature().receiver().is_present()
        ));
    }
    lines.sort();
    text.extend(lines);
    text
}
