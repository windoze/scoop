use super::*;
use scoop_lir::{CAbiCallPlan, CType, DirectCArgument, DirectCReturn, DirectCValue};

pub(super) fn validate(
    target: scoop_lir::LirTargetProfile,
    function: &scoop_lir::ExternFunction,
) -> Result<(), CodegenError> {
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
            .all(|(ty, argument)| match argument {
                DirectCArgument::Scalar(value) => matches_scalar(target, ty, value),
                DirectCArgument::DirectParts(parts) => matches_aggregate(ty, parts.value()),
                DirectCArgument::Indirect { value, passing } => {
                    let passing_matches =
                        *passing == scoop_abi_plan::c_indirect_passing(target, value);
                    passing_matches && matches_aggregate(ty, value)
                }
            });
    let result_matches = match (&signature.return_type, &plan.result) {
        (scoop_lir::CReturnType::Void, DirectCReturn::Void) => true,
        (scoop_lir::CReturnType::Value(ty), DirectCReturn::Value(value)) => {
            matches_scalar(target, ty, value)
        }
        (scoop_lir::CReturnType::Value(ty), DirectCReturn::DirectParts(parts)) => {
            matches_aggregate(ty, parts.value())
        }
        (scoop_lir::CReturnType::Value(ty), DirectCReturn::Indirect(value)) => {
            matches_aggregate(ty, value)
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

fn matches_aggregate(ty: &CType, value: &scoop_lir::AbiValue) -> bool {
    matches!(ty, CType::Struct(_)) && matches_storage(ty, value)
}

fn matches_storage(ty: &CType, value: &scoop_lir::AbiValue) -> bool {
    value.storage_type() == &ty.storage_type() && value.scan() == &RefScan::None
}

fn matches_scalar(target: scoop_lir::LirTargetProfile, ty: &CType, value: &DirectCValue) -> bool {
    !matches!(ty, CType::Struct(_))
        && matches_storage(ty, &value.storage)
        && scoop_abi_plan::c_integer_extension(target, ty) == value.extension
}
