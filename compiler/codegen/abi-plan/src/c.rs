use std::num::NonZeroU64;

use super::{ProcedureAbi, layout, lir, registers::AbiArgumentRegisters};

pub(super) fn classify(
    input: &lir::AbiPlanningInput,
    request: &lir::CAbiRequest,
    abi: ProcedureAbi,
) -> Result<lir::DirectCSignature, lir::AbiValueError> {
    let result = match &request.result {
        lir::LogicalCAbiResult::Void => lir::DirectCReturn::Void,
        lir::LogicalCAbiResult::Value(value) => {
            match classify_value(input, value, lir::AbiValuePosition::Result, abi)? {
                lir::DirectCArgument::Scalar(value) => lir::DirectCReturn::Value(value),
                lir::DirectCArgument::DirectParts(parts) => lir::DirectCReturn::DirectParts(parts),
                lir::DirectCArgument::Indirect { value, .. } => lir::DirectCReturn::Indirect(value),
            }
        }
    };
    let mut params = request
        .arguments
        .iter()
        .map(|value| classify_value(input, value, lir::AbiValuePosition::Argument, abi))
        .collect::<Result<Vec<_>, _>>()?;
    if let ProcedureAbi::SysvAmd64 = abi {
        let mut registers =
            AbiArgumentRegisters::sysv(matches!(result, lir::DirectCReturn::Indirect(_)));
        for (parameter, value) in params.iter_mut().zip(&request.arguments) {
            match parameter {
                lir::DirectCArgument::Scalar(_) => {
                    let lir::AbiPlanningValue::Scalar { carrier, .. } = input.values[value.value]
                    else {
                        unreachable!("C scalar classification retains its scalar layout input");
                    };
                    registers.scalar(carrier);
                }
                lir::DirectCArgument::DirectParts(parts) => {
                    if !registers.aggregate(parts.coercion()) {
                        *parameter = indirect(abi, parts.value().clone());
                    }
                }
                lir::DirectCArgument::Indirect { .. } => {}
            }
        }
    }
    Ok(lir::DirectCSignature { params, result })
}

fn classify_value(
    input: &lir::AbiPlanningInput,
    value: &lir::CAbiValueInput,
    position: lir::AbiValuePosition,
    abi: ProcedureAbi,
) -> Result<lir::DirectCArgument, lir::AbiValueError> {
    match &input.values[value.value] {
        lir::AbiPlanningValue::Aggregate { value: storage, .. } => {
            match layout::coercion(input, value.value, position, abi) {
                Some(coercion) => Ok(lir::DirectCArgument::DirectParts(lir::AbiDirectParts::new(
                    storage.clone(),
                    coercion,
                )?)),
                None => Ok(indirect(abi, storage.clone())),
            }
        }
        lir::AbiPlanningValue::Scalar { value: storage, .. } => {
            Ok(lir::DirectCArgument::Scalar(lir::DirectCValue {
                storage: storage.clone(),
                extension: c_integer_extension(abi, &value.ty),
            }))
        }
        _ => Err(lir::AbiValueError::InvalidCoercion),
    }
}

fn indirect(abi: ProcedureAbi, value: lir::AbiValue) -> lir::DirectCArgument {
    let passing = c_indirect_passing(abi, &value);
    lir::DirectCArgument::Indirect { value, passing }
}

pub fn c_indirect_passing(abi: ProcedureAbi, value: &lir::AbiValue) -> lir::CIndirectPassing {
    match abi {
        ProcedureAbi::DarwinAarch64 | ProcedureAbi::Win64 => lir::CIndirectPassing::CallerCopy,
        ProcedureAbi::SysvAmd64 => lir::CIndirectPassing::ByValue {
            alignment: NonZeroU64::new(value.layout().alignment().get().max(8))
                .expect("nonzero C byval alignment"),
        },
    }
}

pub fn c_integer_extension(abi: ProcedureAbi, ty: &lir::CType) -> lir::CIntegerExtension {
    use lir::CIntegerExtension as Extension;
    match abi {
        ProcedureAbi::Win64 => match ty {
            lir::CType::Boolean => Extension::Zero,
            _ => Extension::None,
        },
        ProcedureAbi::DarwinAarch64 | ProcedureAbi::SysvAmd64 => match ty {
            lir::CType::Integer(kind) | lir::CType::IntegralEnum { kind, .. }
                if kind.width().bits() < 32 =>
            {
                match kind.signedness() {
                    lir::IntegerSignedness::Signed => Extension::Sign,
                    lir::IntegerSignedness::Unsigned => Extension::Zero,
                }
            }
            lir::CType::Boolean => Extension::Zero,
            _ => Extension::None,
        },
    }
}
