//! operand lowering：`LirOperand` → LLVM `BasicValueEnum`。

use inkwell::values::BasicValueEnum;

use scoop2_hir::ty::TypeId;
use scoop2_lir::LirOperand;

use crate::body::FunctionLowerer;
use crate::error::CodegenResult;

impl<'a, 'ctx> FunctionLowerer<'a, 'ctx> {
    /// 把 `LirOperand` 解析为 LLVM 值。
    /// - `Local(id)`：按 local 的声明类型 load（`ty` 参数仅用于 Const 物化）。
    /// - `Const(c)`：物化常量。
    pub fn lower_operand(
        &self,
        operand: &LirOperand,
        ty: TypeId,
    ) -> CodegenResult<BasicValueEnum<'ctx>> {
        match operand {
            LirOperand::Local(id) => self.load_local(*id),
            LirOperand::Const(c) => super::consts::lower_const(self, c, ty),
        }
    }
}
