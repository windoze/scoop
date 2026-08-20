use super::protocol::PendingTypedCall;
use super::*;

impl FunctionLowerer<'_> {
    pub(in crate::function) fn emit_non_native_call_with_signature(
        &mut self,
        destination: LoweredCallDestination,
        signature: &lir::ScoopAbiSignature,
        args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        let (call, value) = self.typed_call_with_signature(signature, args)?;
        if let Some(unwind) = self.current_unwind {
            let normal = self.new_block("invoke.normal");
            let site = self.invoke_site(destination, call, normal, unwind);
            self.push(lir::Instruction::Invoke { site });
            self.seal(lir::Terminator::Br(normal));
            self.enter(normal);
        } else {
            let site = self.call_site(destination, call);
            self.push(lir::Instruction::Call { site });
        }
        Ok(value.unwrap_or_else(|| self.unit_value()))
    }

    pub(in crate::function) fn emit_plain_call(
        &mut self,
        destination: LoweredCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        let (call, value) = self.typed_call(parameter_types, result_type, args)?;
        let site = self.call_site(destination, call);
        self.push(lir::Instruction::Call { site });
        Ok(value.unwrap_or_else(|| self.unit_value()))
    }

    pub(in crate::function) fn emit_native_call(
        &mut self,
        destination: NativeCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        let (call, value) = self.typed_call(parameter_types, result_type, args)?;
        let site = self.native_call_site(destination, call)?;
        self.push(lir::Instruction::Call { site });
        Ok(value.unwrap_or_else(|| self.unit_value()))
    }

    pub(in crate::function) fn emit_native_call_with_signature(
        &mut self,
        destination: NativeCallDestination,
        signature: &lir::ScoopAbiSignature,
        args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        let (call, value) = self.typed_call_with_signature(signature, args)?;
        let site = self.native_call_site(destination, call)?;
        self.push(lir::Instruction::Call { site });
        Ok(value.unwrap_or_else(|| self.unit_value()))
    }

    pub(in crate::function) fn emit_native_storage_call(
        &mut self,
        destination: NativeCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        result_scan: lir::RefScan,
        args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        assert_ne!(result_type, lir::LirType::Void);
        assert_eq!(parameter_types.len(), args.len(), "typed call arity");
        let arguments = parameter_types
            .into_iter()
            .map(|ty| abi::classify_argument(self.context, ty, self.structs, self.enums))
            .collect::<StorageResult<Vec<_>>>()?;
        assert!(
            arguments
                .iter()
                .all(|argument| matches!(argument, lir::AbiArgument::Direct(_))),
            "C storage bridges receive only direct raw storage pointers"
        );
        let result = match abi::classify_return(
            self.context,
            Some(result_type.clone()),
            self.structs,
            self.enums,
        )? {
            lir::AbiReturn::Direct(result) => result.value().clone(),
            lir::AbiReturn::Indirect(result) => result,
            lir::AbiReturn::UnitVoid | lir::AbiReturn::ElidedZst(_) => {
                unreachable!("C storage bridge result must have non-zero storage")
            }
        };
        assert_eq!(
            result.scan(),
            &result_scan,
            "C storage bridge result scan must match its exact storage type"
        );
        let signature = self.call_targets.indirect_result_signatures.alloc(
            lir::IndirectResultCallSignature::c_storage_pointer(
                arguments,
                result,
                lir::CallingConvention::Cdecl,
            ),
        );
        let storage = self.new_hidden_local(result_type)?;
        let args = args.into_iter().map(lir::AbiCallArgument::Direct).collect();
        let call = PendingTypedCall::IndirectResult {
            signature,
            storage,
            args,
        };
        let site = self.native_call_site(destination, call)?;
        self.push(lir::Instruction::Call { site });
        Ok(lir::Value::Local(storage))
    }
}
