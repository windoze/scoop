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
        args: Vec<lir::Value>,
    ) -> StorageResult<lir::Value> {
        let function = match destination {
            LoweredCallDestination::Managed(lir::ManagedCallDestination::Runtime(function)) => {
                lir::RuntimeFunction::Managed(function)
            }
            LoweredCallDestination::NoGc(lir::NoGcCallDestination::Runtime(function)) => {
                lir::RuntimeFunction::NoGc(function)
            }
            _ => unreachable!("plain helper calls always name a runtime entry"),
        };
        let (call, value) = self.typed_runtime_call(function, args)?;
        let site = self.call_site(destination, call);
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
}
