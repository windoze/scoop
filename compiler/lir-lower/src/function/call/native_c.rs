use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_c_call(
        &mut self,
        function: lir::CExternFunctionRef,
        parameter_types: &[mir::Type],
        result: &scoop_identity::ExternResult<mir::Type>,
        args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        let result_ty = result.native_type();
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
                let signature = signature.abi_signature();
                let result = self.emit_native_call_with_signature(destination, &signature, args)?;
                Ok(self.restore_c_value(result_ty, result))
            }
            lir::CAbiCallPlan::StorageBridge { .. } => {
                let mut bridge_args = Vec::with_capacity(args.len());
                for (value, ty) in args.into_iter().zip(parameter_types) {
                    let ty = self.c_storage_type(ty);
                    let local = self.new_hidden_local(ty)?;
                    self.push(lir::Instruction::Store { local, value });
                    bridge_args.push(lir::Value::CArgumentStorage(
                        lir::CArgumentStorage::address_of(local),
                    ));
                }
                let scoop_identity::ExternResult::CaptureErrno { native, scoop } = result else {
                    unreachable!("only errno-adapting externs use a generated storage bridge")
                };
                let signature = self.bridge_signatures[&function.declaration()].clone();
                self.emit_errno_call(destination, native, scoop, &signature, bridge_args)
            }
        }
    }

    fn emit_errno_call(
        &mut self,
        destination: NativeCallDestination,
        native: &mir::Type,
        scoop: &mir::Type,
        signature: &lir::ScoopAbiSignature,
        mut args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        let storage = if *native == mir::Type::Unit {
            None
        } else {
            let storage = self.new_hidden_local(self.c_storage_type(native))?;
            args.insert(
                0,
                lir::Value::CArgumentStorage(lir::CArgumentStorage::address_of(storage)),
            );
            Some(storage)
        };
        let errno = self.emit_native_call_with_signature(destination, signature, args)?;
        let value = match storage {
            Some(storage) => self.restore_c_value(native, lir::Value::Local(storage)),
            None => self.unit_value(),
        };
        Ok(self.make_aggregate(scoop, vec![value, errno]))
    }
}
