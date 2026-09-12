//! Function-level GC strategy and machine-policy attributes.

use super::llvm::llvm_value_name;
use super::*;

pub(super) fn verify_function_policies(
    module: &LlvmModule<'_>,
    expected: &ExpectedSafepoints,
    profile: ValidatedBackendProfile,
) -> Result<(), CodegenError> {
    for (symbol, effect) in &expected.functions {
        let function = module.get_function(symbol).ok_or_else(|| {
            CodegenError(format!(
                "LIR function `{symbol}` is absent from the LLVM module"
            ))
        })?;
        let gc = gc_strategy(function)?;
        match effect {
            GcEffect::Managed => {
                if gc != GC_STRATEGY {
                    return Err(CodegenError(format!(
                        "managed function `{symbol}` has GC strategy `{gc}`, expected `{GC_STRATEGY}`"
                    )));
                }
                verify_string_attribute(
                    function,
                    symbol,
                    "frame-pointer",
                    profile.frame_pointer_attribute(),
                )?;
                verify_string_attribute(
                    function,
                    symbol,
                    "disable-tail-calls",
                    profile.disable_tail_calls_attribute(),
                )?;
            }
            GcEffect::NoGc if !gc.is_empty() => {
                return Err(CodegenError(format!(
                    "NoGc function `{symbol}` unexpectedly has GC strategy `{gc}`"
                )));
            }
            GcEffect::NoGc => {}
        }
    }
    for function in module.get_functions() {
        let gc = gc_strategy(function)?;
        if gc.is_empty() {
            continue;
        }
        let symbol = llvm_value_name(function.as_value_ref())?;
        if !expected.functions.contains_key(&symbol) {
            return Err(CodegenError(format!(
                "LLVM function `{symbol}` has an unmanifested GC strategy `{gc}`"
            )));
        }
    }
    Ok(())
}

fn gc_strategy(function: FunctionValue<'_>) -> Result<String, CodegenError> {
    // LLVMGetGC returns null when no strategy is attached. Inkwell's
    // FunctionValue::get_gc assumes a non-null pointer, so preserve the
    // absent case explicitly instead of constructing a CStr from null.
    let pointer = unsafe { LLVMGetGC(function.as_value_ref()) };
    if pointer.is_null() {
        return Ok(String::new());
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .map(str::to_owned)
        .map_err(|error| CodegenError(format!("LLVM GC strategy is not UTF-8: {error}")))
}

fn verify_string_attribute(
    function: FunctionValue<'_>,
    symbol: &str,
    key: &str,
    expected: &str,
) -> Result<(), CodegenError> {
    let attribute = function
        .get_string_attribute(AttributeLoc::Function, key)
        .ok_or_else(|| {
            CodegenError(format!(
                "managed function `{symbol}` lacks required `{key}` attribute"
            ))
        })?;
    let actual = attribute.get_string_value().to_str().map_err(|error| {
        CodegenError(format!(
            "attribute `{key}` on `{symbol}` is not UTF-8: {error}"
        ))
    })?;
    if actual != expected {
        return Err(CodegenError(format!(
            "managed function `{symbol}` has `{key}`=`{actual}`, expected `{expected}`"
        )));
    }
    Ok(())
}
