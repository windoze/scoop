use super::*;
use scoop_identity::{
    CanonicalCAbiReturn, CanonicalCStorageType, CanonicalScoopStorage, RepresentationRole,
    ScoopAbiArgument, ScoopAbiReturn,
};
use scoop_lir::{CompilerNativeContractV1 as Known, CompilerNativeValueV1 as Value};

pub(super) fn difference(left: &NativeExternalContract, right: &NativeExternalContract) -> String {
    if left.library() != right.library() {
        return "library binding differs".into();
    }
    match (left, right) {
        (
            NativeExternalContract::Function {
                abi: a,
                calling_convention: ac,
                ..
            },
            NativeExternalContract::Function {
                abi: b,
                calling_convention: bc,
                ..
            },
        ) => {
            if ac != bc {
                return "calling convention differs".into();
            }
            match (a, b) {
                (NativeExternAbi::C(a), NativeExternAbi::C(b)) => {
                    if a.parameters() != b.parameters() {
                        "C parameter ABI differs".into()
                    } else {
                        "C result ABI differs".into()
                    }
                }
                (NativeExternAbi::Scoop(a), NativeExternAbi::Scoop(b)) => {
                    if a.gc_effect() != b.gc_effect() {
                        "GC effect differs".into()
                    } else if a.signature() != b.signature() {
                        "Scoop receiver or exact parameter/result type differs".into()
                    } else if a.arguments() != b.arguments() {
                        "Scoop argument storage or passing differs".into()
                    } else {
                        "Scoop result storage or passing differs".into()
                    }
                }
                _ => "C/Scoop ABI differs".into(),
            }
        }
        _ => "function/data/TLS/mutability or storage differs".into(),
    }
}

pub(super) fn check_compiler_contract(
    symbol: &str,
    source: &NativeExternalContract,
    profile: &ValidatedFinalLinkProfile,
    closure: &ProgramLinkClosure,
) -> Result<(), LinkError> {
    let normalize = |name| {
        profile
            .target()
            .contract()
            .native_symbol_normalization()
            .compiler_generated_object_symbol(name)
    };
    let known = scoop_lir::RuntimeAbiSymbolV1::ALL
        .into_iter()
        .find(|entry| normalize(entry.logical_symbol()) == symbol)
        .map(|entry| entry.machine_contract())
        .or_else(|| {
            scoop_lir::TargetEhSupportV1::ALL
                .into_iter()
                .find(|entry| normalize(entry.logical_symbol()) == symbol)
                .map(|entry| entry.machine_contract())
        })
        .or_else(|| {
            scoop_lir::CBridgeTargetSupportV1::for_target(profile.target())
                .iter()
                .copied()
                .find(|entry| normalize(entry.logical_symbol()) == symbol)
                .map(|entry| entry.machine_contract())
        });
    let Some(known) = known else {
        return Ok(());
    };
    match (known, source) {
        (
            Known::Function {
                parameters,
                result,
                effect,
            },
            NativeExternalContract::Function { abi, .. },
        ) => {
            let (actual_parameters, actual_result) = match abi {
                NativeExternAbi::C(signature) => {
                    let args = signature
                        .parameters()
                        .iter()
                        .map(|param| c_value(param.storage()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let result = match signature.result() {
                        CanonicalCAbiReturn::Void => Value::Void,
                        CanonicalCAbiReturn::Value { storage, .. } => c_value(storage)?,
                    };
                    (args, result)
                }
                NativeExternAbi::Scoop(signature) => {
                    if signature.gc_effect() != effect {
                        return Err(error(format!(
                            "native compiler ABI conflict for {symbol}: GC effect differs"
                        )));
                    }
                    let args = signature.arguments().iter().map(|argument| match argument {
                        ScoopAbiArgument::Direct(storage) => scoop_value(*storage, closure),
                        _ => Err(error(format!("native compiler ABI conflict for {symbol}: argument passing differs"))),
                    }).collect::<Result<Vec<_>, _>>()?;
                    let result = match signature.result() {
                        ScoopAbiReturn::UnitVoid => Value::Void,
                        ScoopAbiReturn::Direct(storage) => scoop_value(storage, closure)?,
                        _ => {
                            return Err(error(format!(
                                "native compiler ABI conflict for {symbol}: result passing differs"
                            )));
                        }
                    };
                    (args, result)
                }
            };
            if actual_parameters != parameters || actual_result != result {
                return Err(error(format!(
                    "native compiler ABI conflict for {symbol}: expected {parameters:?} -> {result:?}, found {actual_parameters:?} -> {actual_result:?}"
                )));
            }
        }
        (
            Known::Data {
                byte_size,
                alignment,
                thread_local,
                mutable,
            },
            source,
        ) => {
            let (storage, actual_tls, actual_mutable) = match source {
                NativeExternalContract::ReadOnlyData { storage, .. } => (*storage, false, false),
                NativeExternalContract::MutableData { storage, .. } => (*storage, false, true),
                NativeExternalContract::ReadOnlyTls { storage, .. } => (*storage, true, false),
                NativeExternalContract::MutableTls { storage, .. } => (*storage, true, true),
                _ => {
                    return Err(error(format!(
                        "native compiler ABI conflict for {symbol}: expected data"
                    )));
                }
            };
            let actual_size = match c_value(storage)? {
                Value::Pointer => 8,
                Value::Integer(bits) => u64::from(bits / 8),
                Value::Boolean => 1,
                Value::Void => 0,
            };
            if actual_size != byte_size
                || actual_size != alignment
                || thread_local != actual_tls
                || mutable != actual_mutable
            {
                return Err(error(format!(
                    "native compiler ABI conflict for {symbol}: data layout/TLS/mutability differs"
                )));
            }
        }
        _ => {
            return Err(error(format!(
                "native compiler ABI conflict for {symbol}: expected function"
            )));
        }
    }
    Ok(())
}

fn c_value(storage: CanonicalCStorageType) -> Result<Value, LinkError> {
    match storage {
        CanonicalCStorageType::Integer { bit_width, .. } => Ok(Value::Integer(bit_width.get())),
        CanonicalCStorageType::Boolean { .. } => Ok(Value::Boolean),
        CanonicalCStorageType::DataPointer { .. } | CanonicalCStorageType::CodePointer { .. } => {
            Ok(Value::Pointer)
        }
        CanonicalCStorageType::Struct { .. } => Err(error(
            "compiler ABI has no aggregate C value at this position",
        )),
    }
}

fn scoop_value(
    storage: CanonicalScoopStorage,
    closure: &ProgramLinkClosure,
) -> Result<Value, LinkError> {
    let layout = closure
        .artifacts()
        .find_map(|(artifact, _)| {
            artifact
                .layout()
                .exports()
                .layouts()
                .find_exact_role(storage.exact_type(), RepresentationRole::ManagedValue)
        })
        .and_then(|layout| layout.value_handle())
        .ok_or_else(|| {
            error(format!(
                "missing native ABI layout {}",
                storage.exact_type()
            ))
        })?;
    match layout.representation().kind() {
        scoop_lir::ExactRepresentationKindV1::Scalar(
            scoop_lir::ScalarRepresentationKindV1::Boolean,
        ) => Ok(Value::Boolean),
        scoop_lir::ExactRepresentationKindV1::Scalar(
            scoop_lir::ScalarRepresentationKindV1::Integer(kind),
        ) => Ok(Value::Integer(kind.width().bits() as u8)),
        scoop_lir::ExactRepresentationKindV1::QualifiedPointer(_) => Ok(Value::Pointer),
        _ => Err(error("compiler native ABI requires a scalar or pointer")),
    }
}
