use super::{ProcedureAbi, layout, lir, registers::AbiArgumentRegisters};

fn value(
    input: &lir::AbiPlanningInput,
    id: lir::AbiPlanningValueId,
    position: lir::AbiValuePosition,
    abi: ProcedureAbi,
) -> Result<lir::AbiArgument, lir::AbiValueError> {
    use lir::{AbiArgument as A, AbiDirectParts, AbiDirectValue, AbiPlanningValue as V};
    Ok(match &input.values[id] {
        V::ZeroSized(value) => A::ElidedZst(value.clone()),
        V::Scalar { value, .. } => A::Direct(value.clone().into()),
        V::Interface(value)
            if matches!(
                (abi, position),
                (ProcedureAbi::Win64, lir::AbiValuePosition::Result)
            ) =>
        {
            A::Indirect(value.clone())
        }
        V::Interface(value) => A::Direct(AbiDirectValue::DirectParts(AbiDirectParts::interface(
            value.clone(),
        )?)),
        V::Aggregate { value, .. } | V::IntegerStorage(value) => {
            if value.layout().size().get() <= 16
                && value.layout().alignment().get() <= 8
                && let Some(coercion) = layout::coercion(input, id, position, abi)
            {
                A::Direct(AbiDirectValue::DirectParts(AbiDirectParts::new(
                    value.clone(),
                    coercion,
                )?))
            } else {
                A::Indirect(value.clone())
            }
        }
        V::ScannedAggregate(value) => A::Indirect(value.clone()),
    })
}

pub(super) fn classify(
    input: &lir::AbiPlanningInput,
    request: &lir::ScoopAbiRequest,
    abi: ProcedureAbi,
) -> Result<lir::ScoopAbiSignature, lir::AbiValueError> {
    let result = match request.result {
        lir::LogicalAbiResult::Void => lir::AbiReturn::UnitVoid,
        lir::LogicalAbiResult::Value(id) => {
            match value(input, id, lir::AbiValuePosition::Result, abi)? {
                lir::AbiArgument::ElidedZst(value) => lir::AbiReturn::ElidedZst(value),
                lir::AbiArgument::Direct(value) => lir::AbiReturn::Direct(value),
                lir::AbiArgument::Indirect(value) => lir::AbiReturn::Indirect(value),
            }
        }
    };
    let mut arguments = request
        .arguments
        .iter()
        .map(|&id| value(input, id, lir::AbiValuePosition::Argument, abi))
        .collect::<Result<Vec<_>, _>>()?;
    if let ProcedureAbi::SysvAmd64 = abi {
        let mut registers = AbiArgumentRegisters::sysv(result.is_indirect());
        for (argument, &id) in arguments.iter_mut().zip(&request.arguments) {
            match argument {
                lir::AbiArgument::Direct(lir::AbiDirectValue::Scalar(_)) => {
                    let lir::AbiPlanningValue::Scalar { carrier, .. } = input.values[id] else {
                        unreachable!("scalar classification retains its scalar layout input");
                    };
                    registers.scalar(carrier);
                }
                lir::AbiArgument::Direct(lir::AbiDirectValue::DirectParts(parts)) => {
                    if matches!(input.values[id], lir::AbiPlanningValue::Interface(_)) {
                        for part in parts.parts() {
                            registers.scalar(part.carrier());
                        }
                    } else if !registers.aggregate(parts.coercion()) {
                        *argument = lir::AbiArgument::Indirect(parts.value().clone());
                    }
                }
                lir::AbiArgument::ElidedZst(_) | lir::AbiArgument::Indirect(_) => {}
            }
        }
    }
    Ok(lir::ScoopAbiSignature::new(
        arguments,
        result,
        lir::CallingConvention::Cdecl,
    ))
}
