//! const lowering：`LirConstValue` → LLVM 常量值。

use inkwell::values::BasicValueEnum;

use scoop2_hir::ty::TypeId;
use scoop2_lir::{LirConstValue, LirIntSuffix};

use crate::body::FunctionLowerer;
use crate::error::CodegenResult;

impl<'a, 'ctx> FunctionLowerer<'a, 'ctx> {
    /// 物化一个 `LirConstValue` 为 LLVM 值。
    pub fn lower_const_value(&self, c: &LirConstValue) -> CodegenResult<BasicValueEnum<'ctx>> {
        let ctx = self.cg.context;
        Ok(match c {
            LirConstValue::Bool(b) => ctx.i8_type().const_int(if *b { 1 } else { 0 }, false).into(),
            LirConstValue::Char(ch) => ctx.i32_type().const_int(*ch as u64, false).into(),
            LirConstValue::Unit => ctx.i8_type().const_zero().into(),
            LirConstValue::Int(v, suffix) => {
                // 后缀决定符号性/宽度；当前用 i64（word-sized Int）。
                // suffix: None/U/L/UL —— 暂统一按 i64 处理（完整宽度需查目标布局）。
                let _ = suffix;
                ctx.i64_type().const_int(clamp_to_u64(*v), false).into()
            }
            LirConstValue::Float(v, suffix) => match suffix {
                Some(_) => ctx.f32_type().const_float(*v).into(),
                None => ctx.f64_type().const_float(*v).into(),
            },
            LirConstValue::Null => self.cg.gc_ptr_ty().const_null().into(),
            LirConstValue::String(s) => {
                // String 字面量是 immortal 全局（GC 引用）。
                // 由 globals 层提供；此处委托。
                let ptr = self.cg.get_or_create_string_literal(s)?;
                ptr.into()
            }
        })
    }
}

/// 把 u128 截断为 u64（用于 const_int；溢出按环绕）。
fn clamp_to_u64(v: u128) -> u64 {
    v as u64
}

/// 顶层入口（供 operand 调用）：物化常量，并按 `ty` 校验/转换。
pub fn lower_const<'a, 'ctx>(
    fl: &FunctionLowerer<'a, 'ctx>,
    c: &LirConstValue,
    _ty: TypeId,
) -> CodegenResult<BasicValueEnum<'ctx>> {
    fl.lower_const_value(c)
}

// 避免未使用 import 警告。
#[allow(unused_imports)]
use LirIntSuffix as _LirIntSuffix;
