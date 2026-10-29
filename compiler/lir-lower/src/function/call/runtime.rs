//! Calls to the compiler's fixed runtime operations.

use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_runtime_call(
        &mut self,
        function: mir::RuntimeFn,
        arguments: &[mir::Expr],
        result_ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let mut args: Vec<lir::Value> = arguments
            .iter()
            .map(|arg| {
                let value = self.lower_expr(arg)?;
                Ok::<_, StorageLoweringError>(self.reference_object(value, &arg.ty))
            })
            .collect::<StorageResult<Vec<_>>>()?;
        if let mir::RuntimeFn::PrepareThrowable { trace_bytes } = function {
            args.push(self.td_ref(&mir::Type::Class(trace_bytes)));
        }
        let function = lower_runtime_function(function);
        let value = self.emit_plain_call(runtime_call_destination(function), args)?;
        self.restore_reference_value(value, result_ty)
    }
}
