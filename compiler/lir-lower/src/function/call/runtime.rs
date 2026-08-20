//! Calls to the compiler's fixed runtime operations.

use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_runtime_call(
        &mut self,
        function: mir::RuntimeFn,
        arguments: &[mir::Expr],
        result_ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let expected_arg_count = match function {
            mir::RuntimeFn::StringConcat | mir::RuntimeFn::StringCompare => 2,
            // The GC intrinsics (M9, runtime spec 3.4): the
            // pin / handle operations speak raw machine words
            // — the object reference in, the word out (or the
            // reverse); the hooks take nothing.
            mir::RuntimeFn::Pin
            | mir::RuntimeFn::GetHandle
            | mir::RuntimeFn::Unpin
            | mir::RuntimeFn::ReleaseHandle
            | mir::RuntimeFn::MaterializeException => 1,
            mir::RuntimeFn::InitializationEnter
            | mir::RuntimeFn::InitializationSucceed
            | mir::RuntimeFn::InitializationFailure
            | mir::RuntimeFn::InitializationCycleMessage => 1,
            mir::RuntimeFn::InitializationFail => 2,
            mir::RuntimeFn::GcCollect | mir::RuntimeFn::GcStats => 0,
            // The M6 runtime functions are emitted by dedicated
            // IsInstance / dispatch lowerings, never as plain
            // MIR calls.
            mir::RuntimeFn::IsInstance | mir::RuntimeFn::ITableLookup => {
                unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
            }
        };
        assert_eq!(arguments.len(), expected_arg_count, "runtime call arity");
        let args: Vec<lir::Value> = arguments
            .iter()
            .map(|arg| {
                let value = self.lower_expr(arg)?;
                Ok::<_, StorageLoweringError>(self.reference_object(value, &arg.ty))
            })
            .collect::<StorageResult<Vec<_>>>()?;
        let (parameter_types, result_type) = match function {
            mir::RuntimeFn::StringConcat => {
                (vec![lir::MANAGED_PTR, lir::MANAGED_PTR], lir::MANAGED_PTR)
            }
            mir::RuntimeFn::StringCompare => {
                assert_eq!(
                    *result_ty,
                    mir::Type::Integer(mir::IntegerKind::SIGNED_64),
                    "the closed string-compare runtime ABI returns signed 64-bit",
                );
                (vec![lir::MANAGED_PTR, lir::MANAGED_PTR], lir::LirType::I64)
            }
            // The pin / handle intrinsics exchange a word with
            // the runtime: `pin` / `getGcHandle` yield the raw
            // word (i64), `unpin` / `releaseGcHandle` yield the
            // reference (ptr), `gcStats` yields the count.
            mir::RuntimeFn::Pin | mir::RuntimeFn::GetHandle | mir::RuntimeFn::GcStats => {
                let params = if function == mir::RuntimeFn::GcStats {
                    Vec::new()
                } else {
                    vec![lir::MANAGED_PTR]
                };
                (params, lir::LirType::I64)
            }
            mir::RuntimeFn::Unpin | mir::RuntimeFn::ReleaseHandle => {
                (vec![lir::LirType::I64], lir::MANAGED_PTR)
            }
            // `BeginCatch` publishes the registered stable external
            // exception object as a managed reference. The runtime
            // materializer consumes that same typed reference even
            // though its base is outside the moving heap.
            mir::RuntimeFn::MaterializeException => (vec![lir::MANAGED_PTR], lir::MANAGED_PTR),
            mir::RuntimeFn::InitializationEnter => (
                vec![lir::METADATA_PTR],
                lir::LirType::MachineScalar(lir::MachineScalarKind::InitializationOutcome),
            ),
            mir::RuntimeFn::InitializationSucceed => (vec![lir::METADATA_PTR], lir::LirType::Void),
            mir::RuntimeFn::InitializationFail => (
                vec![lir::METADATA_PTR, lir::MANAGED_PTR],
                lir::LirType::Void,
            ),
            mir::RuntimeFn::InitializationFailure => (vec![lir::METADATA_PTR], lir::MANAGED_PTR),
            mir::RuntimeFn::InitializationCycleMessage => {
                (vec![lir::METADATA_PTR], lir::MANAGED_PTR)
            }
            mir::RuntimeFn::GcCollect => (Vec::new(), lir::LirType::Void),
            mir::RuntimeFn::IsInstance | mir::RuntimeFn::ITableLookup => {
                unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
            }
        };
        let function = lower_runtime_function(function);
        let value = self.emit_plain_call(
            runtime_call_destination(function),
            parameter_types,
            result_type,
            args,
        )?;
        self.restore_reference_value(value, result_ty)
    }
}
