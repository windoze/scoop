use super::*;
use scoop_lir::{CAbiCallPlan, CIntegerExtension, CType, DirectCReturn, DirectCType, DirectCValue};

pub(super) fn validate(function: &scoop_lir::ExternFunction) -> Result<(), CodegenError> {
    let ExternFunctionKind::C {
        signature,
        call_plan,
        ..
    } = &function.kind
    else {
        return Ok(());
    };
    let plan = match call_plan {
        CAbiCallPlan::Direct(plan) => plan,
        CAbiCallPlan::StorageBridge { entry, result } => {
            return if matches!(entry.unit_record().key(),
                scoop_lir::GeneratedBridgeUnitKey::OutboundFunction(_, expected) if expected == result)
            {
                Ok(())
            } else {
                Err(CodegenError(format!(
                    "C extern `{}` bridge recipe disagrees with its result adaptation",
                    function.source_name
                )))
            };
        }
    };
    let params_match = signature.params.len() == plan.params.len()
        && signature
            .params
            .iter()
            .zip(&plan.params)
            .all(|(ty, value)| matches_value(ty, value));
    let result_matches = match (&signature.return_type, &plan.result) {
        (scoop_lir::CReturnType::Void, DirectCReturn::Void) => true,
        (scoop_lir::CReturnType::Value(ty), DirectCReturn::Value(value)) => {
            matches_value(ty, value)
        }
        _ => false,
    };
    if !params_match || !result_matches {
        return Err(CodegenError(format!(
            "C extern `{}` DirectC plan disagrees with its canonical C signature",
            function.source_name
        )));
    }
    Ok(())
}

fn matches_value(ty: &CType, value: &DirectCValue) -> bool {
    let extension = match (ty, &value.ty) {
        (CType::Integer(expected), DirectCType::Integer(actual)) if expected == actual => {
            if expected.width().bits() < 32 {
                match expected.signedness() {
                    scoop_lir::IntegerSignedness::Signed => CIntegerExtension::Sign,
                    scoop_lir::IntegerSignedness::Unsigned => CIntegerExtension::Zero,
                }
            } else {
                CIntegerExtension::None
            }
        }
        (CType::Boolean, DirectCType::Boolean) => CIntegerExtension::Zero,
        (CType::Float(expected), DirectCType::Float(actual)) if expected == actual => {
            CIntegerExtension::None
        }
        (CType::DataPointer { storage, .. }, DirectCType::DataPointer(actual))
            if storage == actual =>
        {
            CIntegerExtension::None
        }
        (CType::CodePointer { storage, .. }, DirectCType::CodePointer(actual))
            if storage == actual =>
        {
            CIntegerExtension::None
        }
        _ => return false,
    };
    extension == value.extension
}
