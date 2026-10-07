use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_c_call(
        &mut self,
        function: lir::CExternFunctionRef,
        parameter_types: &[mir::Type],
        result_ty: &mir::Type,
        args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        let args = args
            .into_iter()
            .zip(parameter_types)
            .map(|(value, ty)| self.project_c_value(ty, value))
            .collect::<Vec<_>>();
        let lir::ExternFunctionKind::C {
            call_mode,
            call_plan,
            ..
        } = &self.extern_functions[function.declaration()].kind
        else {
            unreachable!("C extern reference names a C declaration")
        };
        let destination =
            NativeCallDestination::C(lir::CCallDestination::extern_function(function), *call_mode);
        match call_plan {
            lir::CAbiCallPlan::Direct(signature) => {
                let parameters = signature
                    .params
                    .iter()
                    .map(|value| value.ty.storage_type())
                    .collect();
                let result_type = match &signature.result {
                    lir::DirectCReturn::Void => lir::LirType::Void,
                    lir::DirectCReturn::Value(value) => value.ty.storage_type(),
                };
                let result = self.emit_native_call(destination, parameters, result_type, args)?;
                Ok(self.restore_c_value(result_ty, result))
            }
            lir::CAbiCallPlan::StorageBridge(_) => {
                let mut bridge_args = Vec::with_capacity(args.len());
                for (value, ty) in args.into_iter().zip(parameter_types) {
                    let ty = self.c_storage_type(ty);
                    let local = self.new_hidden_local(ty)?;
                    self.push(lir::Instruction::Store { local, value });
                    bridge_args.push(lir::Value::CArgumentStorage(
                        lir::CArgumentStorage::address_of(local),
                    ));
                }
                let parameters = vec![lir::RAW_PTR; bridge_args.len()];
                if *result_ty == mir::Type::Unit {
                    self.emit_native_call(destination, parameters, lir::LirType::Void, bridge_args)
                } else {
                    let result_type = self.c_storage_type(result_ty);
                    let result = self.emit_native_storage_call(
                        destination,
                        parameters,
                        result_type,
                        lir::RefScan::None,
                        bridge_args,
                    )?;
                    Ok(self.restore_c_value(result_ty, result))
                }
            }
        }
    }
}
