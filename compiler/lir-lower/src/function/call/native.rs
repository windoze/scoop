//! Direct native declarations retain their selected C or Scoop ABI.

use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_native_declaration_call(
        &mut self,
        id: mir::ExternFunctionId,
        arguments: &[mir::Expr],
    ) -> StorageResult<lir::Value> {
        let extern_ = &self.module.extern_functions[id];
        let parameter_types = extern_.params.clone();
        let result = extern_.result.clone();
        assert_eq!(arguments.len(), parameter_types.len(), "extern call arity");
        let args = arguments
            .iter()
            .map(|arg| self.lower_expr(arg))
            .collect::<StorageResult<Vec<_>>>()?;
        match self.extern_function_refs[&id] {
            LoweredExternFunctionRef::C(function) => {
                self.lower_c_call(function, &parameter_types, &result, args)
            }
            LoweredExternFunctionRef::Scoop(function) => {
                let destination = NativeCallDestination::Borrowed(
                    lir::NativeBorrowedCallDestination::extern_function(function),
                );
                let signature = match &self.extern_functions[function.declaration()].kind {
                    lir::ExternFunctionKind::Scoop { signature, .. } => signature.clone(),
                    lir::ExternFunctionKind::C { .. } => {
                        unreachable!("Scoop extern reference names a Scoop declaration")
                    }
                };
                self.emit_native_call_with_signature(destination, &signature, args)
            }
        }
    }
}
