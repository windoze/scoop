//! Exact IEEE constants at the final LLVM boundary.

use super::*;
use inkwell::types::{AsTypeRef, FloatType};
use inkwell::values::{AsValueRef, FloatValue};
use scoop_lir::{FloatKind, LirFloatConstant};

pub(crate) fn float_type(context: &Context, kind: FloatKind) -> FloatType<'_> {
    match kind {
        FloatKind::F32 => context.f32_type(),
        FloatKind::F64 => context.f64_type(),
    }
}

pub(crate) fn float_constant(context: &Context, value: LirFloatConstant) -> FloatValue<'_> {
    let integer = match value {
        LirFloatConstant::F32(bits) => context.i32_type().const_int(u64::from(bits), false),
        LirFloatConstant::F64(bits) => context.i64_type().const_int(bits, false),
    };
    // Both LLVM types have the same bit width. Constant bitcast preserves all
    // IEEE representations, including negative zero and signaling NaNs.
    unsafe {
        FloatValue::new(llvm_sys::core::LLVMConstBitCast(
            integer.as_value_ref(),
            float_type(context, value.kind()).as_type_ref(),
        ))
    }
}
