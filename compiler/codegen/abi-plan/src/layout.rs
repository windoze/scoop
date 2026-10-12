//! Read scalar leaves only when the selected ABI can pass the value in registers.

use super::{ProcedureAbi, aggregate, lir};

pub(super) fn coercion(
    input: &lir::AbiPlanningInput,
    id: lir::AbiPlanningValueId,
    position: lir::AbiValuePosition,
) -> Option<lir::AbiCoercion> {
    let value = match &input.values[id] {
        lir::AbiPlanningValue::Aggregate { value, .. }
        | lir::AbiPlanningValue::IntegerStorage(value) => value,
        _ => return None,
    };
    // AArch64 may use up to four doubles for an HFA. SysV admits two eightbytes.
    let maximum = match ProcedureAbi::for_target(input.target) {
        ProcedureAbi::DarwinAarch64 => 32,
        ProcedureAbi::SysvAmd64 => 16,
    };
    if value.layout().size().get() > maximum {
        return None;
    }
    let mut layout = lir::AbiAggregateLayout {
        size: value.layout().size().get(),
        alignment: value.layout().alignment().get(),
        leaves: Vec::new(),
    };
    collect(input, id, 0, &mut layout.leaves);
    aggregate::coercion(&layout, input.target, position)
}

fn collect(
    input: &lir::AbiPlanningInput,
    id: lir::AbiPlanningValueId,
    offset: u64,
    leaves: &mut Vec<lir::AbiScalarLeaf>,
) {
    match &input.values[id] {
        lir::AbiPlanningValue::ZeroSized(_) => {}
        lir::AbiPlanningValue::Scalar { value, carrier } => leaves.push(lir::AbiScalarLeaf {
            offset,
            carrier: *carrier,
            alignment: value.layout().alignment().get(),
        }),
        lir::AbiPlanningValue::Aggregate { fields, .. } => {
            for field in fields {
                collect(input, field.value, offset + field.offset, leaves);
            }
        }
        lir::AbiPlanningValue::IntegerStorage(value) => {
            let size = value.layout().size().get();
            for chunk in (0..size).step_by(8) {
                leaves.push(lir::AbiScalarLeaf {
                    offset: offset + chunk,
                    carrier: lir::AbiCarrier::Integer(((size - chunk).min(8) * 8) as u8),
                    alignment: value.layout().alignment().get().min(8),
                });
            }
        }
        lir::AbiPlanningValue::Interface(_) | lir::AbiPlanningValue::ScannedAggregate(_) => {
            unreachable!("only reference-free aggregates enter scalar leaf classification")
        }
    }
}
