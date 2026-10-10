use super::*;

mod dispatch;
mod emission;
mod native;
mod native_c;
mod protocol;
mod runtime;

use dispatch::{FunctionDispatch, MethodDispatch};
pub(super) use protocol::LoweredCallDestination;
use protocol::{NativeCallDestination, runtime_call_destination};

impl<'a> FunctionLowerer<'a> {
    pub(super) fn lower_call(
        &mut self,
        call: &mir::Call,
        result_ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        match &call.target {
            mir::CallTarget::Direct(callee) => match *callee {
                mir::Callee::User(id) => self.lower_local_call(id, &call.args),
                mir::Callee::Monomorphized(id) => {
                    self.lower_local_call(self.module.meta.instances[id].function, &call.args)
                }
                mir::Callee::External(id) => {
                    self.lower_external_call(self.external_callable_map[&id], &call.args)
                }
                mir::Callee::Extern(id) => self.lower_native_declaration_call(id, &call.args),
                mir::Callee::Runtime(function) => {
                    self.lower_runtime_call(function, &call.args, result_ty)
                }
            },
            mir::CallTarget::Virtual {
                callee,
                slot,
                receiver,
            } => self.lower_method_call(
                MethodDispatch::Virtual(*slot),
                *callee,
                receiver,
                &call.args,
            ),
            mir::CallTarget::Interface {
                callee,
                slot,
                receiver,
                ..
            } => self.lower_method_call(
                MethodDispatch::Interface(*slot),
                *callee,
                receiver,
                &call.args,
            ),
            mir::CallTarget::Closure {
                function_type,
                receiver,
            } => self.lower_function_call(
                FunctionDispatch::Closure,
                *function_type,
                receiver,
                &call.args,
                result_ty,
            ),
            mir::CallTarget::FunctionBridge {
                function_type,
                receiver,
            } => self.lower_function_call(
                FunctionDispatch::Bridge,
                *function_type,
                receiver,
                &call.args,
                result_ty,
            ),
            mir::CallTarget::CoroutineSuspend { .. } => {
                unreachable!("coroutine state-machine lowering removes suspend markers")
            }
        }
    }

    fn lower_local_call(
        &mut self,
        id: mir::FunctionId,
        arguments: &[mir::Expr],
    ) -> StorageResult<lir::Value> {
        let signature = self.function_signatures[&id].clone();
        assert_eq!(
            arguments.len(),
            signature.logical_argument_count(),
            "user call arity"
        );
        let args = arguments
            .iter()
            .map(|arg| self.lower_expr(arg))
            .collect::<StorageResult<Vec<_>>>()?;
        self.finish_call(
            LoweredCallDestination::local(self.local_function_map[&id]),
            args,
            &signature,
        )
    }

    fn lower_external_call(
        &mut self,
        id: lir::ExternalCallableId,
        arguments: &[mir::Expr],
    ) -> StorageResult<lir::Value> {
        let callable = &self.external_callables[id];
        assert_eq!(
            arguments.len(),
            callable.signature().logical_argument_count(),
            "external call arity"
        );
        let effect = callable.gc_effect();
        let signature = callable.signature().clone();
        let args = arguments
            .iter()
            .map(|argument| self.lower_expr(argument))
            .collect::<StorageResult<Vec<_>>>()?;
        self.emit_non_native_call_with_signature(
            LoweredCallDestination::external(id, effect),
            &signature,
            args,
        )
    }

    /// Load a value of `ty` at a fixed byte offset from a raw pointer.
    pub(super) fn load_at_offset(
        &mut self,
        object: lir::Value,
        offset: u64,
        ty: lir::LirType,
    ) -> lir::TempId {
        let out = self.new_temp(ty.clone());
        match ty {
            lir::LirType::MachineScalar(kind) => {
                assert!(
                    kind.is_atomic_state(),
                    "only compiler-owned coroutine state has heap storage"
                );
                self.push(lir::Instruction::MachineHeapLoad {
                    out,
                    kind,
                    object,
                    offset,
                });
            }
            _ => self.push(lir::Instruction::HeapLoad {
                out,
                object,
                offset,
            }),
        }
        out
    }

    /// Store a value of `ty` at a fixed byte offset. Compiler-owned state uses
    /// a closed instruction family so it cannot pass through a source `i64`
    /// heap operation before final LLVM lowering.
    pub(super) fn store_at_offset(
        &mut self,
        object: lir::Value,
        offset: u64,
        value: lir::Value,
        ty: lir::LirType,
    ) {
        match ty {
            lir::LirType::MachineScalar(kind) => {
                assert!(
                    kind.is_atomic_state(),
                    "only compiler-owned coroutine state has heap storage"
                );
                self.push(lir::Instruction::MachineHeapStore {
                    kind,
                    object,
                    offset,
                    value,
                });
            }
            _ => self.push(lir::Instruction::HeapStore {
                object,
                offset,
                value,
            }),
        }
    }

    /// A direct call: Unit-returning callees are void at the LLVM
    /// level; their Unit value is a fresh empty aggregate. Inside a
    /// try body the call may throw, so it is invoked to the innermost
    /// landing pad (M8): the `Invoke` ends the block (the terminator
    /// convention in the module docs) and the result is available in
    /// the normal successor.
    pub(super) fn finish_call(
        &mut self,
        destination: LoweredCallDestination,
        args: Vec<lir::Value>,
        signature: &lir::ScoopAbiSignature,
    ) -> StorageResult<lir::Value> {
        self.emit_non_native_call_with_signature(destination, signature, args)
    }

    /// An indirect call through a function table (vtable / itable
    /// dispatch, impl spec 2.9). Inside a try body it is invoked to
    /// the innermost landing pad, like `finish_call`.
    pub(super) fn finish_indirect(
        &mut self,
        destination: LoweredCallDestination,
        args: Vec<lir::Value>,
        signature: &lir::ScoopAbiSignature,
    ) -> StorageResult<lir::Value> {
        self.finish_call(destination, args, signature)
    }
}
